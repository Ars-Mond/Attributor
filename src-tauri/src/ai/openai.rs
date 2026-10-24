//! OpenAI Responses API: the request builder and answer parser shared by the OpenAI Platform (API key) and ChatGPT (subscription) providers, plus the non-streaming Platform client.

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::http;
use super::image::ImagePayload;
use super::schema;
use super::types::{AttributionConfig, ModelInfo, Think};

/// Name of the structured-output format; must match `^[A-Za-z0-9_-]{1,64}$` and stay stable so the compiled schema is reused.
const FORMAT_NAME: &str = "photo_metadata";

/// Model id fragments that never accept image + text chat input (audio, image generation, embeddings, …), used to filter `GET /v1/models`.
const NON_CHAT_FRAGMENTS: &[&str] = &[
    "audio",
    "realtime",
    "transcribe",
    "tts",
    "whisper",
    "speech",
    "translate",
    "image",
    "dall-e",
    "embedding",
    "moderation",
    "search",
    "deep-research",
    "computer-use",
    "codex",
    "instruct",
    "babbage",
    "davinci",
    "gpt-3.5",
];

/// Model id prefixes of chat models with vision input.
const CHAT_PREFIXES: &[&str] = &["gpt-6", "gpt-5", "gpt-4.1", "gpt-4o", "o3"];

/// Model id prefixes that match [`CHAT_PREFIXES`] but take no image input.
const TEXT_ONLY_PREFIXES: &[&str] = &["o3-mini"];

/// Vision chat models visible to the key (`GET /v1/models`). The endpoint carries no capability data, so models are filtered by id.
///
/// # Errors
///
/// Returns the API's error message for an invalid key or an unreachable host.
pub async fn list_models(base_url: &str, key: &str) -> Result<Vec<ModelInfo>, String> {
    #[derive(Deserialize)]
    struct List {
        data: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        id: String,
    }
    let resp = http::client()
        .get(http::url(base_url, "models"))
        .bearer_auth(key)
        .timeout(http::QUICK_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("OpenAI API not reachable: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!(
            "OpenAI API returned {status}: {}",
            http::error_detail(resp).await
        ));
    }
    let list: List = resp.json().await.map_err(|e| e.to_string())?;
    let mut models: Vec<ModelInfo> = list
        .data
        .into_iter()
        .filter(|m| is_vision_chat_model(&m.id))
        .map(|m| ModelInfo {
            name: m.id,
            size: 0,
        })
        .collect();
    models.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(models)
}

/// Whether a model id from `GET /v1/models` names a chat model with image input (judged by id, as the list has no capabilities).
pub fn is_vision_chat_model(id: &str) -> bool {
    CHAT_PREFIXES.iter().any(|p| id.starts_with(p))
        && !TEXT_ONLY_PREFIXES.iter().any(|p| id.starts_with(p))
        && !NON_CHAT_FRAGMENTS.iter().any(|f| id.contains(f))
}

/// Builds a Responses request: the prompt first (a prefix shared by every photo, so it is cached), then the image; `store: false` keeps photos off OpenAI's servers.
pub fn request_body(cfg: &AttributionConfig, image: &ImagePayload) -> Map<String, Value> {
    let mut body = Map::new();
    body.insert("model".into(), json!(cfg.model));
    body.insert("store".into(), json!(false));
    body.insert(
        "input".into(),
        json!([{
            "type": "message",
            "role": "user",
            "content": [
                {"type": "input_text", "text": cfg.prompt},
                {"type": "input_image", "image_url": image.data_url(), "detail": "high"}
            ]
        }]),
    );
    if schema::is_present(&cfg.format) {
        let (schema, strict) = schema::for_openai(&cfg.format);
        body.insert("text".into(), json!({"format": {"type": "json_schema", "name": FORMAT_NAME, "schema": schema, "strict": strict}}));
    }
    // Only an explicit effort level is sent: supported levels differ per model family, and the model default is the safe choice otherwise.
    if let Some(Think::Level(level)) = &cfg.think {
        body.insert("reasoning".into(), json!({"effort": level}));
    }
    http::merge_options(&mut body, &cfg.options);
    body
}

/// Extracts the answer text from a Responses object (`output[].content[]` parts of type `output_text`), turning refusals and incomplete or failed responses into errors.
pub fn answer_text(response: &Value, provider: &str) -> Result<String, String> {
    match response.get("status").and_then(Value::as_str) {
        Some("incomplete") => {
            let reason = response
                .pointer("/incomplete_details/reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            return Err(format!("{provider} stopped before finishing the answer ({reason}); lower the reasoning effort or raise max_output_tokens"));
        }
        Some("failed") => {
            let message = response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(format!("{provider} failed: {message}"));
        }
        _ => {}
    }
    let mut text = String::new();
    let parts = response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten();
    for part in parts {
        match part.get("type").and_then(Value::as_str) {
            Some("output_text") => {
                text.push_str(part.get("text").and_then(Value::as_str).unwrap_or_default())
            }
            Some("refusal") => {
                let reason = part
                    .get("refusal")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                return Err(format!(
                    "{provider} declined to describe this image: {reason}"
                ));
            }
            _ => {}
        }
    }
    if text.trim().is_empty() {
        return Err(format!("{provider} returned no text"));
    }
    Ok(text)
}

/// One non-streaming vision inference against the OpenAI Platform. Returns the JSON answer text.
///
/// # Errors
///
/// Returns an error for transport failures, non-success statuses (with the API's message), refusals and incomplete answers.
pub async fn generate(
    cfg: &AttributionConfig,
    key: &str,
    image: &ImagePayload,
) -> Result<String, String> {
    let body = request_body(cfg, image);
    log::info!(
        "openai generate → model={} options={} think={:?} promptChars={}",
        cfg.model,
        cfg.options,
        cfg.think,
        cfg.prompt.chars().count()
    );
    let resp = http::client()
        .post(http::url(&cfg.base_url, "responses"))
        .bearer_auth(key)
        .json(&body)
        .timeout(http::GENERATE_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("OpenAI API not reachable: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!(
            "OpenAI API returned {status}: {}",
            http::error_detail(resp).await
        ));
    }
    let response: Value = resp.json().await.map_err(|e| e.to_string())?;
    let text = answer_text(&response, "OpenAI")?;
    log::info!(
        "openai generate ← model={} responseChars={}",
        cfg.model,
        text.chars().count()
    );
    log::debug!("openai generate raw response: {text}");
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> AttributionConfig {
        serde_json::from_value(json!({
            "provider": "openai",
            "baseUrl": "https://api.openai.com/v1",
            "model": "gpt-6-luna",
            "prompt": "Describe",
            "think": "low",
            "options": {},
            "format": {"type": "object", "properties": {"title": {"type": "string"}}, "required": ["title"]}
        }))
        .unwrap()
    }

    #[test]
    fn body_puts_prompt_before_image_with_strict_schema() {
        let body = Value::Object(request_body(
            &cfg(),
            &ImagePayload {
                mime: "image/jpeg",
                base64: "QUJD".into(),
            },
        ));
        assert_eq!(body["store"], json!(false));
        assert_eq!(
            body["input"][0]["content"][0],
            json!({"type": "input_text", "text": "Describe"})
        );
        assert_eq!(
            body["input"][0]["content"][1]["image_url"],
            json!("data:image/jpeg;base64,QUJD")
        );
        assert_eq!(body["text"]["format"]["strict"], json!(true));
        assert_eq!(
            body["text"]["format"]["schema"]["additionalProperties"],
            json!(false)
        );
        assert_eq!(body["reasoning"], json!({"effort": "low"}));
    }

    #[test]
    fn bool_think_sends_no_reasoning() {
        let mut c = cfg();
        c.think = Some(Think::Bool(true));
        assert!(request_body(
            &c,
            &ImagePayload {
                mime: "image/jpeg",
                base64: String::new()
            }
        )
        .get("reasoning")
        .is_none());
    }

    #[test]
    fn answer_text_skips_reasoning_items() {
        let r = json!({"status": "completed", "output": [
            {"type": "reasoning", "summary": []},
            {"type": "message", "content": [{"type": "output_text", "text": "{\"title\":"}, {"type": "output_text", "text": "\"x\"}"}]}
        ]});
        assert_eq!(answer_text(&r, "OpenAI").unwrap(), "{\"title\":\"x\"}");
    }

    #[test]
    fn answer_text_reports_refusal_and_incomplete() {
        let refusal = json!({"status": "completed", "output": [{"type": "message", "content": [{"type": "refusal", "refusal": "no"}]}]});
        assert!(answer_text(&refusal, "OpenAI")
            .unwrap_err()
            .contains("declined"));
        let incomplete = json!({"status": "incomplete", "incomplete_details": {"reason": "max_output_tokens"}, "output": []});
        assert!(answer_text(&incomplete, "OpenAI")
            .unwrap_err()
            .contains("max_output_tokens"));
    }

    #[test]
    fn model_filter_keeps_vision_chat_models_only() {
        assert!(is_vision_chat_model("gpt-6-luna"));
        assert!(is_vision_chat_model("gpt-5.4-mini-2026-03-17"));
        assert!(!is_vision_chat_model("gpt-realtime"));
        assert!(!is_vision_chat_model("gpt-image-2"));
        assert!(!is_vision_chat_model("text-embedding-3-large"));
        assert!(!is_vision_chat_model("whisper-1"));
        assert!(!is_vision_chat_model("o3-mini"));
        assert!(!is_vision_chat_model("chatgpt-4o-latest"));
        assert!(is_vision_chat_model("o3"));
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use crate::ai::http::mock;

    #[tokio::test]
    async fn generate_posts_responses_request() {
        let reply = json!({"status": "completed", "output": [{"type": "message", "content": [{"type": "output_text", "text": "{\"title\":\"x\"}"}]}]});
        let (base, server) = mock::serve_once(200, reply).await;
        let cfg: AttributionConfig = serde_json::from_value(json!({
            "provider": "openai", "baseUrl": format!("{base}/v1"), "model": "gpt-6-luna", "prompt": "Describe",
            "think": "low", "options": {}, "format": {"type": "object", "properties": {"title": {"type": "string"}}, "required": ["title"]}
        }))
        .unwrap();
        let text = generate(
            &cfg,
            "sk-test",
            &ImagePayload {
                mime: "image/jpeg",
                base64: "QUJD".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(text, "{\"title\":\"x\"}");
        let req = server.await.unwrap();
        assert_eq!(req.request_line(), "POST /v1/responses HTTP/1.1");
        assert_eq!(req.header("authorization"), Some("Bearer sk-test"));
        assert_eq!(req.body["store"], json!(false));
        assert_eq!(req.body["text"]["format"]["strict"], json!(true));
    }

    #[tokio::test]
    async fn list_models_reports_invalid_keys() {
        let (base, _server) = mock::serve_once(
            401,
            json!({"error": {"message": "Incorrect API key provided", "code": "invalid_api_key"}}),
        )
        .await;
        let err = list_models(&format!("{base}/v1"), "bad").await.unwrap_err();
        assert!(
            err.contains("401") && err.contains("Incorrect API key"),
            "{err}"
        );
    }
}
