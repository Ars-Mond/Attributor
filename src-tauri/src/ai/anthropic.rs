//! Claude Messages API client (raw HTTP: there is no official Anthropic SDK for Rust).
//! One image + prompt per request; the answer is constrained by `output_config.format` and depth is set with `output_config.effort`.

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::http;
use super::image::ImagePayload;
use super::schema;
use super::types::{AttributionConfig, ModelInfo, Think};

/// API version header. From this version on the API rejects `temperature`, `top_p` and `top_k` on every model.
const API_VERSION: &str = "2026-10-21";

/// Output cap for one non-streaming request; leaves room for adaptive thinking before the JSON answer.
const MAX_TOKENS: u32 = 16000;

/// Beta that enables `fallbacks: "default"`: a request declined by a safety classifier is re-run on Anthropic's recommended model.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// Model families that accept server-side fallback.
const FALLBACK_MODEL_PREFIXES: &[&str] = &[
    "claude-fable-5",
    "claude-mythos-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];

fn authorized(req: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    req.header("x-api-key", key)
        .header("anthropic-version", API_VERSION)
}

/// Vision-capable models available to the key (`GET /v1/models`, all pages).
///
/// # Errors
///
/// Returns the API's error message for an invalid key or an unreachable host.
pub async fn list_models(base_url: &str, key: &str) -> Result<Vec<ModelInfo>, String> {
    #[derive(Deserialize)]
    struct Page {
        data: Vec<Model>,
        #[serde(default)]
        has_more: bool,
        #[serde(default)]
        last_id: Option<String>,
    }
    #[derive(Deserialize)]
    struct Model {
        id: String,
        #[serde(default)]
        capabilities: Value,
    }

    let mut out = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let mut req = authorized(http::client().get(http::url(base_url, "v1/models")), key)
            .query(&[("limit", "1000")])
            .timeout(http::QUICK_TIMEOUT);
        if let Some(after) = &after {
            req = req.query(&[("after_id", after.as_str())]);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| format!("Claude API not reachable: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            return Err(format!(
                "Claude API returned {status}: {}",
                http::error_detail(resp).await
            ));
        }
        let page: Page = resp.json().await.map_err(|e| e.to_string())?;
        // Models without a capability tree (older API deployments) are kept rather than hidden.
        out.extend(
            page.data
                .into_iter()
                .filter(|m| {
                    m.capabilities
                        .pointer("/image_input/supported")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                })
                .map(|m| ModelInfo {
                    name: m.id,
                    size: 0,
                }),
        );
        match (page.has_more, page.last_id) {
            (true, Some(last)) => after = Some(last),
            _ => break,
        }
    }
    Ok(out)
}

/// Builds the Messages request body. Separated from [`generate`] so the shape can be unit-tested.
fn request_body(cfg: &AttributionConfig, image: &ImagePayload, fallback: bool) -> Value {
    let mut body = Map::new();
    body.insert("model".into(), json!(cfg.model));
    body.insert("max_tokens".into(), json!(MAX_TOKENS));
    // The prompt is identical for every photo, so it goes into a cached system block; the user turn carries only the image.
    body.insert(
        "system".into(),
        json!([{"type": "text", "text": cfg.prompt, "cache_control": {"type": "ephemeral"}}]),
    );
    body.insert(
        "messages".into(),
        json!([{
            "role": "user",
            "content": [{"type": "image", "source": {"type": "base64", "media_type": image.mime, "data": image.base64}}]
        }]),
    );
    let mut output_config = Map::new();
    if schema::is_present(&cfg.format) {
        output_config.insert(
            "format".into(),
            json!({"type": "json_schema", "schema": schema::for_anthropic(&cfg.format)}),
        );
    }
    // Thinking stays adaptive (the default on current models); only an explicit effort level changes it.
    if let Some(Think::Level(level)) = &cfg.think {
        output_config.insert("effort".into(), json!(level));
    }
    if !output_config.is_empty() {
        body.insert("output_config".into(), Value::Object(output_config));
    }
    if fallback {
        body.insert("fallbacks".into(), json!("default"));
    }
    http::merge_options(&mut body, &cfg.options);
    Value::Object(body)
}

/// Server-side fallback exists only on the first-party API and for some model families.
fn fallback_supported(base_url: &str, model: &str) -> bool {
    base_url.contains("api.anthropic.com")
        && FALLBACK_MODEL_PREFIXES.iter().any(|p| model.starts_with(p))
}

/// One vision inference. Returns the concatenated text blocks of the answer (the JSON document).
///
/// # Errors
///
/// Returns an error for transport failures, non-success statuses (with the API's message), refusals and answers cut off at `max_tokens`.
pub async fn generate(
    cfg: &AttributionConfig,
    key: &str,
    image: &ImagePayload,
) -> Result<String, String> {
    let fallback = fallback_supported(&cfg.base_url, &cfg.model);
    let body = request_body(cfg, image, fallback);
    log::info!(
        "claude generate → model={} options={} think={:?} fallback={fallback} promptChars={}",
        cfg.model,
        cfg.options,
        cfg.think,
        cfg.prompt.chars().count()
    );

    let mut req = authorized(
        http::client().post(http::url(&cfg.base_url, "v1/messages")),
        key,
    )
    .json(&body)
    .timeout(http::GENERATE_TIMEOUT);
    if fallback {
        req = req.header("anthropic-beta", FALLBACK_BETA);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Claude API not reachable: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!(
            "Claude API returned {status}: {}",
            http::error_detail(resp).await
        ));
    }
    let message: Value = resp.json().await.map_err(|e| e.to_string())?;
    let text = answer_text(&message)?;
    log::info!(
        "claude generate ← model={} responseChars={}",
        message
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(&cfg.model),
        text.chars().count()
    );
    log::debug!("claude generate raw response: {text}");
    Ok(text)
}

/// Extracts the answer from a Messages response, turning refusals and truncation into errors.
fn answer_text(message: &Value) -> Result<String, String> {
    match message.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => {
            let category = message
                .pointer("/stop_details/category")
                .and_then(Value::as_str)
                .unwrap_or("unspecified");
            return Err(format!(
                "Claude declined to describe this image (category: {category})"
            ));
        }
        Some("max_tokens") => {
            return Err(
                "Claude's answer was cut off by max_tokens; raise it in the model profile options"
                    .to_string(),
            )
        }
        _ => {}
    }
    let text: String = message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect();
    if text.trim().is_empty() {
        return Err("Claude returned no text".to_string());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> AttributionConfig {
        serde_json::from_value(json!({
            "provider": "anthropic",
            "baseUrl": "https://api.anthropic.com",
            "model": "claude-opus-5-5",
            "prompt": "Describe",
            "think": "high",
            "options": {"max_tokens": 4000},
            "format": {"type": "object", "properties": {"keywords": {"type": "array", "items": {"type": "string"}, "maxItems": 50}}, "required": ["keywords"]}
        }))
        .unwrap()
    }

    fn image() -> ImagePayload {
        ImagePayload {
            mime: "image/jpeg",
            base64: "QUJD".into(),
        }
    }

    #[test]
    fn body_has_image_text_schema_and_effort() {
        let body = request_body(&cfg(), &image(), true);
        assert_eq!(
            body["messages"][0]["content"][0]["source"],
            json!({"type": "base64", "media_type": "image/jpeg", "data": "QUJD"})
        );
        assert_eq!(body["system"][0]["text"], json!("Describe"));
        assert_eq!(
            body["system"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
        assert_eq!(
            body["output_config"]["format"]["type"],
            json!("json_schema")
        );
        assert_eq!(
            body["output_config"]["format"]["schema"]["additionalProperties"],
            json!(false)
        );
        assert!(
            body["output_config"]["format"]["schema"]["properties"]["keywords"]
                .get("maxItems")
                .is_none()
        );
        assert_eq!(body["output_config"]["effort"], json!("high"));
        assert_eq!(body["max_tokens"], json!(4000));
        assert_eq!(body["fallbacks"], json!("default"));
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn fallback_only_for_first_party_supported_models() {
        assert!(fallback_supported(
            "https://api.anthropic.com",
            "claude-opus-5-5"
        ));
        assert!(!fallback_supported(
            "https://api.anthropic.com",
            "claude-haiku-5-5"
        ));
        assert!(!fallback_supported(
            "https://proxy.example.com",
            "claude-opus-5-5"
        ));
    }

    #[test]
    fn answer_text_skips_thinking_and_reports_refusals() {
        let ok = json!({"stop_reason": "end_turn", "content": [{"type": "thinking", "thinking": ""}, {"type": "text", "text": "{\"a\":1}"}]});
        assert_eq!(answer_text(&ok).unwrap(), "{\"a\":1}");
        let refused = json!({"stop_reason": "refusal", "stop_details": {"category": "general_harms"}, "content": []});
        assert!(answer_text(&refused).unwrap_err().contains("general_harms"));
        let cut = json!({"stop_reason": "max_tokens", "content": [{"type": "text", "text": "{"}]});
        assert!(answer_text(&cut).is_err());
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use crate::ai::http::mock;

    #[tokio::test]
    async fn generate_posts_messages_request_with_version_header() {
        let reply = json!({"model": "claude-opus-5-5", "stop_reason": "end_turn", "content": [{"type": "text", "text": "{\"title\":\"x\"}"}]});
        let (base, server) = mock::serve_once(200, reply).await;
        let cfg: AttributionConfig = serde_json::from_value(json!({
            "provider": "anthropic", "baseUrl": base, "model": "claude-opus-5-5", "prompt": "Describe",
            "options": {}, "format": {"type": "object", "properties": {"title": {"type": "string"}}, "required": ["title"]}
        }))
        .unwrap();
        let text = generate(
            &cfg,
            "sk-ant-test",
            &ImagePayload {
                mime: "image/jpeg",
                base64: "QUJD".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(text, "{\"title\":\"x\"}");
        let req = server.await.unwrap();
        assert_eq!(req.request_line(), "POST /v1/messages HTTP/1.1");
        assert_eq!(req.header("x-api-key"), Some("sk-ant-test"));
        assert_eq!(req.header("anthropic-version"), Some(API_VERSION));
        // A non-Anthropic host gets no server-side fallback (the beta is first-party only).
        assert!(req.header("anthropic-beta").is_none());
        assert!(req.body.get("fallbacks").is_none());
    }

    #[tokio::test]
    async fn list_models_follows_pagination() {
        let (base, server) = mock::serve_once(200, json!({"data": [{"id": "claude-opus-5-5", "capabilities": {"image_input": {"supported": true}}}], "has_more": false})).await;
        let models = list_models(&base, "k").await.unwrap();
        assert_eq!(models[0].name, "claude-opus-5-5");
        assert!(server
            .await
            .unwrap()
            .request_line()
            .starts_with("GET /v1/models?limit=1000 "));
    }
}
