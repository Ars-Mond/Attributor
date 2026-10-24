//! Client for local servers that speak the OpenAI Chat Completions protocol: llama.cpp (`llama-server`), LM Studio, Jan, vLLM and Unsloth.
//! The base URL is the API root including `/v1`; an API key is optional and sent as a bearer token when the user stored one.

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::http;
use super::image::ImagePayload;
use super::schema;
use super::types::{AttributionConfig, ModelInfo, Provider, Think};

/// Name of the structured-output format (OpenAI-style servers require one).
const FORMAT_NAME: &str = "photo_metadata";

fn with_key(req: reqwest::RequestBuilder, key: Option<&str>) -> reqwest::RequestBuilder {
    match key {
        Some(key) => req.bearer_auth(key),
        None => req,
    }
}

/// Models the server offers (`GET {base}/models`), without entries that cannot take a photo locally.
///
/// # Errors
///
/// Returns an error when the server is not reachable or answers with a non-success status.
pub async fn list_models(
    provider: Provider,
    base_url: &str,
    key: Option<&str>,
) -> Result<Vec<ModelInfo>, String> {
    #[derive(Deserialize)]
    struct List {
        data: Vec<Value>,
    }
    let label = provider.label();
    let resp = with_key(http::client().get(http::url(base_url, "models")), key)
        .timeout(http::QUICK_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("{label} not reachable at {base_url}: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!(
            "{label} returned {status}: {}",
            http::error_detail(resp).await
        ));
    }
    let list: List = resp
        .json()
        .await
        .map_err(|e| format!("{label} returned an unexpected model list: {e}"))?;
    Ok(list
        .data
        .iter()
        .filter(|m| is_local_vision_candidate(m))
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .map(|id| ModelInfo {
            name: id.to_string(),
            size: 0,
        })
        .collect())
}

/// Drops model-list entries that would not analyze a photo on this machine: Jan's proxied cloud models (`owned_by: "remote"`), Unsloth's non-chat rows (they carry a `task`), and llama.cpp models whose `architecture.input_modalities` lacks images.
/// Entries without these fields are kept, since most servers report no capabilities at all.
fn is_local_vision_candidate(model: &Value) -> bool {
    if model.get("owned_by").and_then(Value::as_str) == Some("remote")
        || model.get("task").is_some()
    {
        return false;
    }
    match model
        .pointer("/architecture/input_modalities")
        .and_then(Value::as_array)
    {
        Some(modalities) => modalities.iter().any(|m| m.as_str() == Some("image")),
        None => true,
    }
}

/// Builds the Chat Completions body: one user message with the image first and the prompt after it, and a JSON-Schema `response_format`.
fn request_body(cfg: &AttributionConfig, image: &ImagePayload) -> Value {
    let mut body = Map::new();
    body.insert("model".into(), json!(cfg.model));
    body.insert("stream".into(), json!(false));
    body.insert(
        "messages".into(),
        json!([{
            "role": "user",
            "content": [
                {"type": "image_url", "image_url": {"url": image.data_url()}},
                {"type": "text", "text": cfg.prompt}
            ]
        }]),
    );
    if schema::is_present(&cfg.format) {
        body.insert("response_format".into(), json!({"type": "json_schema", "json_schema": {"name": FORMAT_NAME, "strict": true, "schema": cfg.format}}));
    }
    match &cfg.think {
        Some(Think::Bool(enabled)) => {
            // Hybrid reasoning models (Qwen3, Gemma 4, …) toggle thinking through their chat template; `reasoning_effort: "none"` is the template-independent off switch of llama.cpp, vLLM and Unsloth. LM Studio documents only `reasoning_effort`.
            if cfg.provider != Provider::Lmstudio {
                body.insert(
                    "chat_template_kwargs".into(),
                    json!({"enable_thinking": enabled}),
                );
            }
            if !enabled {
                body.insert("reasoning_effort".into(), json!("none"));
            }
        }
        Some(Think::Level(level)) => {
            body.insert("reasoning_effort".into(), json!(level));
        }
        None => {}
    }
    if cfg.provider == Provider::Unsloth {
        // Unsloth runs server-side tools (code, terminal, web search) by default; describing a photo must not trigger them.
        body.insert("enable_tools".into(), json!(false));
    }
    http::merge_options(&mut body, &cfg.options);
    Value::Object(body)
}

/// One vision inference. Returns the assistant message content (the JSON answer).
///
/// # Errors
///
/// Returns an error when the server is unreachable, answers with a non-success status, truncates the answer or returns no content.
pub async fn generate(
    cfg: &AttributionConfig,
    key: Option<&str>,
    image: &ImagePayload,
) -> Result<String, String> {
    let label = cfg.provider.label();
    let body = request_body(cfg, image);
    log::info!(
        "{label} generate → model={} options={} think={:?} promptChars={}",
        cfg.model,
        cfg.options,
        cfg.think,
        cfg.prompt.chars().count()
    );
    let resp = with_key(
        http::client().post(http::url(&cfg.base_url, "chat/completions")),
        key,
    )
    .json(&body)
    .timeout(http::GENERATE_TIMEOUT)
    .send()
    .await
    .map_err(|e| format!("{label} not reachable at {}: {e}", cfg.base_url))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!(
            "{label} returned {status}: {}",
            http::error_detail(resp).await
        ));
    }
    let completion: Value = resp.json().await.map_err(|e| e.to_string())?;
    let text = answer_text(&completion, label)?;
    log::info!(
        "{label} generate ← model={} responseChars={}",
        cfg.model,
        text.chars().count()
    );
    log::debug!("{label} generate raw response: {text}");
    Ok(text)
}

/// Extracts `choices[0].message.content`, turning truncation and refusals into errors. Reasoning text in `reasoning_content` / `reasoning`
/// is ignored, and `<think>` blocks left in the content (servers configured not to separate reasoning) are removed.
fn answer_text(completion: &Value, label: &str) -> Result<String, String> {
    // LM Studio answers unknown routes with HTTP 200 and an `error` body.
    if let Some(error) = completion.get("error").filter(|e| !e.is_null()) {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| error.as_str())
            .unwrap_or("unknown error");
        return Err(format!("{label} returned an error: {message}"));
    }
    let choice = completion
        .pointer("/choices/0")
        .ok_or_else(|| format!("{label} returned no choices"))?;
    if let Some(refusal) = choice.pointer("/message/refusal").and_then(Value::as_str) {
        return Err(format!(
            "{label} declined to describe this image: {refusal}"
        ));
    }
    if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
        return Err(format!("{label} stopped at the token limit before finishing the answer; raise max_tokens or the context size"));
    }
    let content = strip_think(
        choice
            .pointer("/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if content.trim().is_empty() {
        return Err(format!("{label} returned an empty answer"));
    }
    Ok(content.to_string())
}

/// Removes leading `<think>…</think>` blocks; their text may contain braces that would confuse JSON extraction.
fn strip_think(content: &str) -> &str {
    let mut rest = content.trim_start();
    while let Some(after_open) = rest.strip_prefix("<think>") {
        match after_open.find("</think>") {
            Some(end) => rest = after_open[end + "</think>".len()..].trim_start(),
            None => return "",
        }
    }
    rest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(think: Value) -> AttributionConfig {
        serde_json::from_value(json!({
            "provider": "llamacpp",
            "baseUrl": "http://127.0.0.1:8080/v1",
            "model": "qwen2.5-vl",
            "prompt": "Describe",
            "think": think,
            "options": {"temperature": 0.4, "top_k": 20},
            "format": {"type": "object", "properties": {"keywords": {"type": "array", "maxItems": 50}}}
        }))
        .unwrap()
    }

    #[test]
    fn body_sends_image_prompt_schema_and_options() {
        let body = request_body(
            &cfg(Value::Null),
            &ImagePayload {
                mime: "image/jpeg",
                base64: "QUJD".into(),
            },
        );
        assert_eq!(
            body["messages"][0]["content"][0]["image_url"]["url"],
            json!("data:image/jpeg;base64,QUJD")
        );
        assert_eq!(body["messages"][0]["content"][1]["text"], json!("Describe"));
        assert_eq!(
            body["response_format"]["json_schema"]["schema"]["properties"]["keywords"]["maxItems"],
            json!(50)
        );
        assert_eq!(body["temperature"], json!(0.4));
        assert_eq!(body["top_k"], json!(20));
        assert!(body.get("chat_template_kwargs").is_none());
    }

    #[test]
    fn think_maps_to_template_kwargs_or_effort() {
        let image = ImagePayload {
            mime: "image/jpeg",
            base64: String::new(),
        };
        let off = request_body(&cfg(json!(false)), &image);
        assert_eq!(
            off["chat_template_kwargs"],
            json!({"enable_thinking": false})
        );
        assert_eq!(off["reasoning_effort"], json!("none"));
        assert!(request_body(&cfg(json!(true)), &image)
            .get("reasoning_effort")
            .is_none());
        assert_eq!(
            request_body(&cfg(json!("high")), &image)["reasoning_effort"],
            json!("high")
        );
    }

    #[test]
    fn lm_studio_gets_effort_only_and_unsloth_disables_tools() {
        let image = ImagePayload {
            mime: "image/jpeg",
            base64: String::new(),
        };
        let mut c = cfg(json!(false));
        c.provider = Provider::Lmstudio;
        let body = request_body(&c, &image);
        assert!(body.get("chat_template_kwargs").is_none());
        assert_eq!(body["reasoning_effort"], json!("none"));
        c.provider = Provider::Unsloth;
        assert_eq!(request_body(&c, &image)["enable_tools"], json!(false));
    }

    #[test]
    fn model_filter_drops_remote_task_and_text_only_entries() {
        assert!(is_local_vision_candidate(
            &json!({"id": "a", "owned_by": "llama.cpp"})
        ));
        assert!(!is_local_vision_candidate(
            &json!({"id": "gpt", "owned_by": "remote"})
        ));
        assert!(!is_local_vision_candidate(
            &json!({"id": "tts", "task": "tts"})
        ));
        assert!(!is_local_vision_candidate(
            &json!({"id": "t", "architecture": {"input_modalities": ["text"]}})
        ));
        assert!(is_local_vision_candidate(
            &json!({"id": "v", "architecture": {"input_modalities": ["text", "image"]}})
        ));
    }

    #[test]
    fn answer_text_strips_think_and_reports_200_errors() {
        let thought = json!({"choices": [{"finish_reason": "stop", "message": {"content": "<think>{maybe}</think>\n{\"a\":1}"}}]});
        assert_eq!(answer_text(&thought, "x").unwrap(), "{\"a\":1}");
        let error = json!({"error": "Unexpected endpoint or method. (POST /chat/completions)"});
        assert!(answer_text(&error, "x")
            .unwrap_err()
            .contains("Unexpected endpoint"));
    }

    #[test]
    fn answer_text_handles_length_and_empty() {
        let ok = json!({"choices": [{"finish_reason": "stop", "message": {"content": "{}", "reasoning_content": "hmm"}}]});
        assert_eq!(answer_text(&ok, "x").unwrap(), "{}");
        let cut = json!({"choices": [{"finish_reason": "length", "message": {"content": "{"}}]});
        assert!(answer_text(&cut, "x").is_err());
        let empty = json!({"choices": [{"finish_reason": "stop", "message": {"content": null}}]});
        assert!(answer_text(&empty, "x").is_err());
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use crate::ai::http::mock;

    fn cfg(base_url: &str) -> AttributionConfig {
        serde_json::from_value(json!({
            "provider": "jan",
            "baseUrl": base_url,
            "model": "gemma-4-vision",
            "prompt": "Describe",
            "options": {},
            "format": {"type": "object", "properties": {"title": {"type": "string"}}}
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn generate_posts_chat_completion_with_bearer_key() {
        let reply = json!({"choices": [{"finish_reason": "stop", "message": {"content": "{\"title\":\"x\"}"}}]});
        let (base, server) = mock::serve_once(200, reply).await;
        let image = ImagePayload {
            mime: "image/jpeg",
            base64: "QUJD".into(),
        };
        let text = generate(&cfg(&format!("{base}/v1")), Some("sk-local"), &image)
            .await
            .unwrap();
        assert_eq!(text, "{\"title\":\"x\"}");
        let req = server.await.unwrap();
        assert_eq!(req.request_line(), "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(req.header("authorization"), Some("Bearer sk-local"));
        assert_eq!(req.body["model"], json!("gemma-4-vision"));
        assert_eq!(req.body["stream"], json!(false));
        assert_eq!(
            req.body["response_format"]["json_schema"]["name"],
            json!("photo_metadata")
        );
    }

    #[tokio::test]
    async fn generate_without_key_sends_no_authorization() {
        let reply = json!({"choices": [{"finish_reason": "stop", "message": {"content": "{}"}}]});
        let (base, server) = mock::serve_once(200, reply).await;
        let image = ImagePayload {
            mime: "image/jpeg",
            base64: String::new(),
        };
        generate(&cfg(&format!("{base}/v1")), None, &image)
            .await
            .unwrap();
        assert!(server.await.unwrap().header("authorization").is_none());
    }

    #[tokio::test]
    async fn generate_surfaces_server_errors() {
        let reply = json!({"error": {"message": "image input is not supported - hint: provide the mmproj", "type": "server_error"}});
        let (base, _server) = mock::serve_once(500, reply).await;
        let image = ImagePayload {
            mime: "image/jpeg",
            base64: String::new(),
        };
        let err = generate(&cfg(&format!("{base}/v1")), None, &image)
            .await
            .unwrap_err();
        assert!(err.contains("500") && err.contains("mmproj"), "{err}");
    }

    #[tokio::test]
    async fn list_models_filters_remote_entries() {
        let reply = json!({"object": "list", "data": [{"id": "local-vl", "owned_by": "llama.cpp"}, {"id": "gpt-6", "owned_by": "remote"}]});
        let (base, server) = mock::serve_once(200, reply).await;
        let models = list_models(Provider::Jan, &format!("{base}/v1"), None)
            .await
            .unwrap();
        assert_eq!(
            models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            ["local-vl"]
        );
        assert_eq!(
            server.await.unwrap().request_line(),
            "GET /v1/models HTTP/1.1"
        );
    }
}
