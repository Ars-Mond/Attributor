//! Pure-Rust HTTP client for the local Ollama daemon (reqwest + rustls). Plain HTTP to localhost.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use futures_util::StreamExt;
use serde::Deserialize;

use super::types::{AttributionConfig, OllamaModel};

fn http() -> reqwest::Client {
    reqwest::Client::new()
}

fn base(url: &str) -> &str {
    url.trim_end_matches('/')
}

/// Reachability heartbeat — returns the daemon version on success.
pub async fn version(base_url: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct V {
        version: String,
    }
    let url = format!("{}/api/version", base(base_url));
    let resp = http()
        .get(&url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let v: V = resp.json().await.map_err(|e| e.to_string())?;
    Ok(v.version)
}

/// Locally-installed models (`GET /api/tags`).
pub async fn list_tags(base_url: &str) -> Result<Vec<OllamaModel>, String> {
    #[derive(Deserialize)]
    struct Tags {
        models: Vec<Tag>,
    }
    #[derive(Deserialize)]
    struct Tag {
        name: String,
        #[serde(default)]
        size: u64,
    }
    let url = format!("{}/api/tags", base(base_url));
    let resp = http()
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let t: Tags = resp.json().await.map_err(|e| e.to_string())?;
    Ok(t.models.into_iter().map(|m| OllamaModel { name: m.name, size: m.size }).collect())
}

/// One NDJSON progress line from `/api/pull`.
#[derive(Deserialize)]
pub struct PullLine {
    pub status: String,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub completed: Option<u64>,
}

/// Pull (download) a model, invoking `on_line` per NDJSON line. Stops early (Ok) if `cancel` is set.
pub async fn pull(
    base_url: &str,
    model: &str,
    cancel: &Arc<AtomicBool>,
    mut on_line: impl FnMut(PullLine),
) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Req<'a> {
        model: &'a str,
        stream: bool,
    }
    let url = format!("{}/api/pull", base(base_url));
    let resp = http()
        .post(&url)
        .json(&Req { model, stream: true })
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Ok(()); // cooperative cancel; partial pulls are resumable
        }
        let chunk = chunk.map_err(|e| e.to_string())?;
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let trimmed = &line[..line.len().saturating_sub(1)];
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(parsed) = serde_json::from_slice::<PullLine>(trimmed) {
                on_line(parsed);
            }
        }
    }
    Ok(())
}

/// One non-streaming vision inference. Returns the model's `response` string (the strict JSON text).
pub async fn generate(cfg: &AttributionConfig, image_b64: String) -> Result<String, String> {
    let mut body = serde_json::json!({
        "model": cfg.model,
        "prompt": cfg.prompt,
        "images": [image_b64],
        "stream": false,
        "format": cfg.format,
    });
    let obj = body.as_object_mut().ok_or("internal: request body is not an object")?;
    if !cfg.options.is_null() {
        obj.insert("options".into(), cfg.options.clone());
    }
    if let Some(think) = &cfg.think {
        obj.insert("think".into(), serde_json::to_value(think).map_err(|e| e.to_string())?);
    }
    if let Some(keep_alive) = &cfg.keep_alive {
        obj.insert("keep_alive".into(), serde_json::Value::String(keep_alive.clone()));
    }

    #[derive(Deserialize)]
    struct Gen {
        response: String,
    }
    let url = format!("{}/api/generate", base(&cfg.base_url));
    let resp = http()
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(600))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let g: Gen = resp.json().await.map_err(|e| e.to_string())?;
    Ok(g.response)
}

/// Read an image file and base64-encode it for the `images` array (standard engine, no data: prefix).
pub fn image_to_base64(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Whether the `ollama` command exists on the system (installed), regardless of the daemon running.
/// Blocking — call inside `spawn_blocking`.
pub fn is_installed() -> bool {
    std::process::Command::new("ollama")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Ensure the daemon is running before an operation: if the heartbeat fails, spawn `ollama serve`
/// (detached) and poll until it answers (~15 s). Returns Err if Ollama is missing or never comes up.
pub async fn ensure_running(base_url: &str) -> Result<(), String> {
    if version(base_url).await.is_ok() {
        return Ok(());
    }
    log::info!("ensure_running: Ollama not reachable, starting `ollama serve`");
    std::process::Command::new("ollama")
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "Ollama is not installed or could not be started".to_string())?;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if version(base_url).await.is_ok() {
            return Ok(());
        }
    }
    Err("Ollama did not become ready in time".to_string())
}
