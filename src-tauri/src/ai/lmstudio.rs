//! LM Studio specifics on top of the OpenAI-compatible client: the `lms` CLI (detection and server auto-start, mirroring Ollama's "inference starts the daemon") and the native model list, which tells vision models and loaded instances apart.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::http;
use super::openai_compat;
use super::types::{ModelInfo, Provider};

/// The server root (`http://host:port`) of an OpenAI-compatible base URL (`…/v1`).
fn server_root(base_url: &str) -> &str {
    let trimmed = base_url.trim_end_matches('/');
    trimmed.strip_suffix("/v1").unwrap_or(trimmed)
}

/// Candidate `lms` executables: PATH first, then the directories LM Studio's installers use (`<LMStudioHome>/bin`, with `~/.lmstudio-home-pointer` naming a relocated home).
fn lms_candidates() -> Vec<PathBuf> {
    let exe = if cfg!(windows) { "lms.exe" } else { "lms" };
    let mut v = vec![PathBuf::from("lms")];
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if let Some(home) = home {
        if let Ok(pointer) = std::fs::read_to_string(home.join(".lmstudio-home-pointer")) {
            let pointed = pointer.trim();
            if !pointed.is_empty() {
                v.push(PathBuf::from(pointed).join("bin").join(exe));
            }
        }
        v.push(home.join(".lmstudio").join("bin").join(exe));
        v.push(home.join(".cache").join("lm-studio").join("bin").join(exe));
    }
    v
}

/// A command that does not flash a console window when started from the GUI on Windows.
fn quiet_command(program: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// The first `lms` executable that runs. Blocking — call inside `spawn_blocking`.
fn find_lms() -> Option<PathBuf> {
    lms_candidates().into_iter().find(|bin| {
        quiet_command(bin)
            .arg("version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Whether LM Studio's `lms` CLI exists (installed), regardless of the server running. Blocking — call inside `spawn_blocking`.
pub fn is_installed() -> bool {
    find_lms().is_some()
}

/// Whether the server answers its unauthenticated greeting.
async fn greets(base_url: &str) -> bool {
    let req = http::client()
        .get(http::url(server_root(base_url), "lmstudio-greeting"))
        .timeout(Duration::from_secs(2));
    matches!(req.send().await, Ok(resp) if resp.status().is_success())
}

/// Ensures the LM Studio server is up before inference: if it does not answer, runs `lms server start` on the configured port (the CLI wakes the app or its headless daemon) and waits until the server greets. Without the CLI the request simply proceeds and reports the connection error.
///
/// # Errors
///
/// Returns an error when `lms` exists but the server does not come up within about a minute.
pub async fn ensure_running(base_url: &str) -> Result<(), String> {
    if greets(base_url).await {
        return Ok(());
    }
    let port = reqwest::Url::parse(base_url)
        .ok()
        .and_then(|u| u.port_or_known_default());
    let started = tokio::task::spawn_blocking(move || {
        let lms = find_lms()?;
        log::info!("ensure_running: LM Studio not reachable, running `lms server start`");
        let mut cmd = quiet_command(&lms);
        cmd.args(["server", "start"]);
        if let Some(port) = port {
            cmd.args(["--port", &port.to_string()]);
        }
        Some(cmd.status().map(|s| s.success()).unwrap_or(false))
    })
    .await
    .map_err(|e| e.to_string())?;
    match started {
        None => return Ok(()),
        Some(false) => {
            log::warn!("`lms server start` reported a failure; waiting for the server anyway")
        }
        Some(true) => {}
    }
    for _ in 0..60 {
        if greets(base_url).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err("LM Studio was started but its server did not become ready in time".to_string())
}

/// Vision models known to LM Studio. Uses the native `GET /api/v1/models` (0.4+), which marks vision models and lists every downloaded one (JIT loading loads it on first use); older versions fall back to the OpenAI-compatible list.
///
/// # Errors
///
/// Returns an error when the server is not reachable.
pub async fn list_models(base_url: &str, key: Option<&str>) -> Result<Vec<ModelInfo>, String> {
    let mut req = http::client()
        .get(http::url(server_root(base_url), "api/v1/models"))
        .timeout(http::QUICK_TIMEOUT);
    if let Some(key) = key {
        req = req.bearer_auth(key);
    }
    if let Ok(resp) = req.send().await {
        if resp.status().is_success() {
            if let Ok(body) = resp.json::<Value>().await {
                if let Some(models) = native_vision_models(&body) {
                    return Ok(models);
                }
            }
        }
    }
    openai_compat::list_models(Provider::Lmstudio, base_url, key).await
}

/// Parses the native model list into vision LLMs; `None` when the body is not the native shape.
fn native_vision_models(body: &Value) -> Option<Vec<ModelInfo>> {
    let models = body.get("models")?.as_array()?;
    Some(
        models
            .iter()
            .filter(|m| m.get("type").and_then(Value::as_str) == Some("llm"))
            .filter(|m| {
                m.pointer("/capabilities/vision")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .filter_map(|m| {
                let key = m.get("key").and_then(Value::as_str)?;
                Some(ModelInfo {
                    name: key.to_string(),
                    size: m.get("size_bytes").and_then(Value::as_u64).unwrap_or(0),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn server_root_strips_v1() {
        assert_eq!(
            server_root("http://127.0.0.1:1234/v1"),
            "http://127.0.0.1:1234"
        );
        assert_eq!(
            server_root("http://127.0.0.1:1234/v1/"),
            "http://127.0.0.1:1234"
        );
        assert_eq!(
            server_root("http://127.0.0.1:1234"),
            "http://127.0.0.1:1234"
        );
    }

    #[test]
    fn native_list_keeps_vision_llms_only() {
        let body = json!({"models": [
            {"type": "llm", "key": "qwen/qwen3-vl-8b", "size_bytes": 5, "capabilities": {"vision": true}},
            {"type": "llm", "key": "qwen/qwen3-8b", "capabilities": {"vision": false}},
            {"type": "embedding", "key": "nomic-embed"}
        ]});
        let models = native_vision_models(&body).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "qwen/qwen3-vl-8b");
        assert_eq!(models[0].size, 5);
        assert!(native_vision_models(&json!({"data": []})).is_none());
    }
}
