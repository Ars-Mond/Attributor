//! Attribution: run a vision inference and map the strict JSON onto the editor's metadata.
//! Single mode returns the parsed result (frontend applies it); batch mode merges into each file's
//! existing metadata and saves it, sequentially (Ollama serializes inference), streaming progress.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::events::{BatchProgress, ItemStatus};
use crate::types::SaveRequest;

use super::client;
use super::types::{AttributionConfig, AttributionResult};

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
    let start = s.find(|c| c == '{' || c == '[');
    let end = s.rfind(|c| c == '}' || c == ']');
    match (start, end) {
        (Some(a), Some(b)) if b >= a => &s[a..=b],
        _ => s,
    }
}

/// Parse and validate the model's strict-JSON `response` string into the applied fields. The
/// editorial/mature_content/illustration flags are lenient (default false if missing or non-bool).
fn parse_result(raw: &str) -> Result<AttributionResult, String> {
    let v: serde_json::Value =
        serde_json::from_str(extract_json(raw)).map_err(|e| format!("invalid JSON from model: {e}"))?;
    let str_field = |key: &str| -> Result<String, String> {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("response missing string field '{key}'"))
    };
    let arr_field = |key: &str| -> Result<Vec<String>, String> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
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

/// Resolve once the shared cancel flag is set (polled). Raced against the inference so a single
/// attribution can be cancelled from the UI — dropping the `generate` future aborts the HTTP request.
async fn cancelled(cancel: &Arc<AtomicBool>) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Single-image attribution → parsed result (the frontend applies it to the form). Auto-starts the
/// daemon and races the inference against the cancel flag so it can be aborted mid-request.
pub async fn attribute_one(
    path: &str,
    cfg: &AttributionConfig,
    cancel: &Arc<AtomicBool>,
) -> Result<AttributionResult, String> {
    log::info!("attribute start: {path} (model {})", cfg.model);
    client::ensure_running(&cfg.base_url).await?;
    let image = client::image_to_base64(path)?;
    let raw = tokio::select! {
        _ = cancelled(cancel) => return Err("cancelled".to_string()),
        result = client::generate(cfg, image) => result?,
    };
    let result = parse_result(&raw)?;
    log::info!(
        "attribute done: {path} → {} keywords, {} categories",
        result.keywords.len(),
        result.categories.len()
    );
    Ok(result)
}

/// Attribute one file and persist it: read existing metadata, overwrite text/categories, append+dedupe
/// keywords, then save via the shared per-file writer. Returns the final path.
async fn attribute_and_save(
    path: &str,
    cfg: &AttributionConfig,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    let result = attribute_one(path, cfg, cancel).await?;
    let path = path.to_string();
    tokio::task::spawn_blocking(move || {
        let existing = crate::photo::read_metadata(path.clone()).unwrap_or_default();
        let mut keywords = existing.keywords;
        for kw in result.keywords {
            let k = kw.trim().to_lowercase();
            if !k.is_empty() && !keywords.iter().any(|e| e.eq_ignore_ascii_case(&k)) {
                keywords.push(k);
            }
        }
        let stem = Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        crate::batch::save_one(SaveRequest {
            filepath: path,
            filename: stem,
            title: result.title,
            description: result.description,
            keywords,
            categories: result.categories.join(", "),
            release_filename: String::new(),
            editorial: result.editorial,
            mature_content: result.mature_content,
            illustration: result.illustration,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sequentially attribute and save every path, streaming one `BatchProgress` per file. A failed item is
/// recorded and the loop continues; cancellation stops before the next item (already-saved files remain).
pub async fn attribute_batch(
    paths: &[String],
    cfg: &AttributionConfig,
    cancel: &Arc<AtomicBool>,
    progress: impl Fn(BatchProgress),
) -> Vec<ItemStatus> {
    let mut out = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let status = if cancel.load(Ordering::Relaxed) {
            ItemStatus::Cancelled
        } else {
            match attribute_and_save(path, cfg, cancel).await {
                Ok(p) => ItemStatus::Ok { path: p },
                // Cancelled mid-inference (the generate future was dropped) — record it as cancelled.
                Err(_) if cancel.load(Ordering::Relaxed) => ItemStatus::Cancelled,
                Err(error) => {
                    log::warn!("attribute batch item {index} failed: {error}");
                    ItemStatus::Failed { error }
                }
            }
        };
        progress(BatchProgress { index, status: status.clone() });
        out.push(status);
    }
    out
}
