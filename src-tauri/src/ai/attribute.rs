//! Attribution: run a vision inference with the configured provider and map the strict JSON onto the editor's metadata.
//! Single mode returns the parsed result (frontend applies it); batch mode stores each result in the metadata store as app-only (feature 008 — the file is not modified), sequentially (local servers serialize inference and cloud rate limits stay predictable), streaming progress.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::events::{BatchProgress, ItemStatus};
use crate::store::{DbState, StoredMetadata};

use super::image::{self, ImagePayload};
use super::types::{AttributionConfig, AttributionResult, Provider};
use super::{anthropic, chatgpt, http, lmstudio, ollama, openai, openai_compat, schema, secrets};

/// Best-effort extraction of the JSON payload from a model response. Lenient / cloud models often wrap
/// the JSON in a markdown code fence (```json … ```) or surround it with prose; strip the fence and
/// narrow to the outermost object/array span so the parser sees clean JSON.
fn extract_json(raw: &str) -> &str {
    let mut s = raw.trim();

    // Unwrap a fenced code block: ```json … ``` (or a plain ``` … ```).
    if let Some(rest) = s.strip_prefix("```") {
        // Skip the remainder of the opening-fence line (an optional language tag like "json").
        let body = rest.split_once('\n').map_or(rest, |(_, b)| b);
        s = body.trim().strip_suffix("```").unwrap_or(body).trim();
    }

    // Narrow to the outermost JSON object/array span, ignoring any surrounding prose. Brace chars are
    // ASCII, so the byte indices are valid char boundaries.
    let start = s.find(['{', '[']);
    let end = s.rfind(['}', ']']);
    match (start, end) {
        (Some(a), Some(b)) if b >= a => &s[a..=b],
        _ => s,
    }
}

/// Parse and validate the model's strict-JSON `response` string into the applied fields. The
/// editorial/mature_content/illustration flags are lenient (default false if missing or non-bool).
/// `clamp_to` truncates array fields to the schema's `maxItems`, for providers whose structured output cannot enforce it.
fn parse_result(
    raw: &str,
    clamp_to: Option<&serde_json::Value>,
) -> Result<AttributionResult, String> {
    let mut v: serde_json::Value = serde_json::from_str(extract_json(raw))
        .map_err(|e| format!("invalid JSON from model: {e}"))?;
    if let Some(schema) = clamp_to {
        schema::clamp_arrays(&mut v, schema);
    }
    let str_field = |key: &str| -> Result<String, String> {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("response missing string field '{key}'"))
    };
    let arr_field = |key: &str| -> Result<Vec<String>, String> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .ok_or_else(|| format!("response missing array field '{key}'"))
    };
    let bool_field = |key: &str| -> bool { v.get(key).and_then(|x| x.as_bool()).unwrap_or(false) };
    Ok(AttributionResult {
        title: str_field("title")?,
        description: str_field("description")?,
        keywords: arr_field("keywords")?,
        categories: arr_field("categories")?,
        editorial: bool_field("editorial"),
        mature_content: bool_field("mature_content"),
        illustration: bool_field("illustration"),
    })
}

/// Resolve once the shared cancel flag is set (polled). Raced against the inference so a single attribution can be cancelled from the UI — dropping the inference future aborts the HTTP request.
async fn cancelled(cancel: &Arc<AtomicBool>) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The key a cloud provider cannot work without, after checking that `base_url` is a safe destination for it.
async fn required_key(provider: Provider, base_url: &str) -> Result<String, String> {
    let key = secrets::load_api_key(provider).await?.ok_or_else(|| {
        format!(
            "{} API key is not set (Settings → AI attribution)",
            provider.label()
        )
    })?;
    http::check_key_destination(base_url)?;
    Ok(key)
}

/// The downscaled JPEG every provider except Ollama receives (decoding runs off the async runtime).
async fn prepared_image(path: &str) -> Result<ImagePayload, String> {
    let path = path.to_string();
    tokio::task::spawn_blocking(move || image::prepare_jpeg(&path, image::MAX_EDGE))
        .await
        .map_err(|e| e.to_string())?
}

/// Runs one inference with the configured provider and returns the model's raw answer text.
/// Ollama and LM Studio are started on demand when installed but not running.
async fn infer(
    path: &str,
    cfg: &AttributionConfig,
    session: &chatgpt::Session,
) -> Result<String, String> {
    match cfg.provider {
        Provider::Ollama => {
            ollama::ensure_running(&cfg.base_url).await?;
            let path = path.to_string();
            let image = tokio::task::spawn_blocking(move || image::original_base64(&path))
                .await
                .map_err(|e| e.to_string())??;
            ollama::generate(cfg, image).await
        }
        Provider::Openai => {
            let key = required_key(cfg.provider, &cfg.base_url).await?;
            openai::generate(cfg, &key, &prepared_image(path).await?).await
        }
        Provider::Anthropic => {
            let key = required_key(cfg.provider, &cfg.base_url).await?;
            anthropic::generate(cfg, &key, &prepared_image(path).await?).await
        }
        Provider::Chatgpt => chatgpt::generate(cfg, session, &prepared_image(path).await?).await,
        Provider::Llamacpp
        | Provider::Lmstudio
        | Provider::Jan
        | Provider::Vllm
        | Provider::Unsloth => {
            if cfg.provider == Provider::Lmstudio {
                lmstudio::ensure_running(&cfg.base_url).await?;
            }
            let key = secrets::optional_api_key(cfg.provider).await;
            openai_compat::generate(cfg, key.as_deref(), &prepared_image(path).await?).await
        }
    }
}

/// Single-image attribution → parsed result (the frontend applies it to the form). Races the whole inference (including daemon start-up) against the cancel flag so it can be aborted mid-request.
pub async fn attribute_one(
    path: &str,
    cfg: &AttributionConfig,
    cancel: &Arc<AtomicBool>,
    session: &chatgpt::Session,
) -> Result<AttributionResult, String> {
    log::info!(
        "attribute start: {path} (provider {}, model {})",
        cfg.provider.label(),
        cfg.model
    );
    let raw = tokio::select! {
        _ = cancelled(cancel) => return Err("cancelled".to_string()),
        result = infer(path, cfg, session) => result?,
    };
    // Ollama keeps its historical behavior (no clamping); cloud schemas drop maxItems, so their answers are clamped here.
    let clamp_to = (cfg.provider != Provider::Ollama).then_some(&cfg.format);
    let result = parse_result(&raw, clamp_to)?;
    log::info!(
        "attribute done: {path} → {} keywords, {} categories",
        result.keywords.len(),
        result.categories.len()
    );
    Ok(result)
}

/// Attribute one file and store the result in the metadata store as app-only (feature 008 — the file
/// is NOT modified). Keyword merge + release_filename preservation happen inside `store_attribution`.
/// Returns the (unchanged) path.
async fn attribute_and_store(
    path: &str,
    cfg: &AttributionConfig,
    cancel: &Arc<AtomicBool>,
    db: &DbState,
    session: &chatgpt::Session,
) -> Result<String, String> {
    let result = attribute_one(path, cfg, cancel, session).await?;
    let model = StoredMetadata {
        title: result.title,
        description: result.description,
        keywords: result.keywords,
        categories: result.categories.join(", "),
        release_filename: String::new(),
        editorial: result.editorial,
        mature_content: result.mature_content,
        illustration: result.illustration,
    };
    // Run the SQLite write off the async runtime (no mutex/SQLite work on a tokio worker thread).
    let dbh = db.share();
    let p = path.to_string();
    tokio::task::spawn_blocking(move || dbh.store_attribution(&p, &model))
        .await
        .map_err(|e| e.to_string())?;
    Ok(path.to_string())
}

/// Sequentially attribute every path and store each result (app-only), streaming one `BatchProgress`
/// per file. A failed item is recorded and the loop continues; cancellation stops before the next item.
pub async fn attribute_batch(
    paths: &[String],
    cfg: &AttributionConfig,
    cancel: &Arc<AtomicBool>,
    db: &DbState,
    session: &chatgpt::Session,
    progress: impl Fn(BatchProgress),
) -> Vec<ItemStatus> {
    let mut out = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let status = if cancel.load(Ordering::Relaxed) {
            ItemStatus::Cancelled
        } else {
            match attribute_and_store(path, cfg, cancel, db, session).await {
                Ok(p) => ItemStatus::Ok { path: p },
                // Cancelled mid-inference (the generate future was dropped) — record it as cancelled.
                Err(_) if cancel.load(Ordering::Relaxed) => ItemStatus::Cancelled,
                Err(error) => {
                    log::warn!("attribute batch item {index} failed: {error}");
                    ItemStatus::Failed { error }
                }
            }
        };
        progress(BatchProgress {
            index,
            status: status.clone(),
        });
        out.push(status);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ANSWER: &str = r#"{"title":"T","description":"D","keywords":["a","b","c"],"categories":["Nature"],"editorial":true}"#;

    #[test]
    fn extract_json_unwraps_fences_and_prose() {
        assert_eq!(extract_json(&format!("```json\n{ANSWER}\n```")), ANSWER);
        assert_eq!(
            extract_json(&format!("Here you go: {ANSWER} Enjoy!")),
            ANSWER
        );
        assert_eq!(extract_json(ANSWER), ANSWER);
    }

    #[test]
    fn parse_result_reads_fields_and_defaults_flags() {
        let r = parse_result(ANSWER, None).unwrap();
        assert_eq!((r.title.as_str(), r.keywords.len()), ("T", 3));
        assert!(r.editorial && !r.mature_content && !r.illustration);
        assert!(parse_result(r#"{"title":"T"}"#, None).is_err());
    }

    #[test]
    fn parse_result_clamps_only_when_asked() {
        let schema = json!({"properties": {"keywords": {"type": "array", "maxItems": 2}}});
        assert_eq!(
            parse_result(ANSWER, Some(&schema)).unwrap().keywords,
            ["a", "b"]
        );
        assert_eq!(parse_result(ANSWER, None).unwrap().keywords.len(), 3);
    }
}

#[cfg(test)]
mod end_to_end {
    use super::*;
    use crate::ai::http::mock;
    use serde_json::json;

    /// The whole single-photo path for a local OpenAI-compatible server: image preparation, request, answer parsing and clamping.
    /// (Reads the provider's optional key from the OS credential store; none is expected in a test environment.)
    #[tokio::test]
    async fn attribute_one_against_a_local_server() {
        let answer = json!({"title": "T", "description": "D", "keywords": ["a", "b", "c"], "categories": ["Nature"], "illustration": true});
        let reply = json!({"choices": [{"finish_reason": "stop", "message": {"content": answer.to_string()}}]});
        let (base, server) = mock::serve_once(200, reply).await;
        let cfg: AttributionConfig = serde_json::from_value(json!({
            "provider": "vllm",
            "baseUrl": format!("{base}/v1"),
            "model": "Qwen/Qwen3-VL-8B-Instruct",
            "prompt": "Describe",
            "think": false,
            "options": {},
            "format": {"type": "object", "properties": {"keywords": {"type": "array", "items": {"type": "string"}, "maxItems": 2}}}
        }))
        .unwrap();
        let photo = format!("{}/../test_images/0T2A6727.jpg", env!("CARGO_MANIFEST_DIR"));
        let cancel = Arc::new(AtomicBool::new(false));
        let result = attribute_one(&photo, &cfg, &cancel, &chatgpt::Session::default())
            .await
            .unwrap();
        assert_eq!(
            result.keywords,
            ["a", "b"],
            "clamped to the schema's maxItems"
        );
        assert!(result.illustration && !result.editorial);

        let req = server.await.unwrap();
        let url = req.body["messages"][0]["content"][0]["image_url"]["url"]
            .as_str()
            .unwrap();
        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert_eq!(
            req.body["chat_template_kwargs"],
            json!({"enable_thinking": false})
        );
    }

    #[tokio::test]
    async fn attribute_one_is_cancelable() {
        let cfg: AttributionConfig = serde_json::from_value(json!({
            "provider": "llamacpp", "baseUrl": "http://127.0.0.1:9/v1", "model": "m", "prompt": "p", "format": {}
        }))
        .unwrap();
        let cancel = Arc::new(AtomicBool::new(true));
        let err = attribute_one("missing.jpg", &cfg, &cancel, &chatgpt::Session::default())
            .await
            .unwrap_err();
        assert_eq!(err, "cancelled");
    }
}
