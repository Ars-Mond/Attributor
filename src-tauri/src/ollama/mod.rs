//! Ollama vision auto-attribution: HTTP client to the local daemon, model management, and single/batch
//! attribution. Progress streams over `tauri::ipc::Channel`; a shared `Arc<AtomicBool>` (in `OllamaState`)
//! gives cooperative cancellation, mirroring the `batch` module.

pub mod attribute;
pub mod client;
pub mod types;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::events::{BatchProgress, ItemStatus, PullProgress};
use types::{AttributionConfig, AttributionResult, OllamaModel, OllamaStatus};

/// Tauri-managed state holding the in-flight Ollama operation's cancel flag.
#[derive(Default)]
pub struct OllamaState {
    pub cancel: Mutex<Option<Arc<AtomicBool>>>,
}

/// Cancel any previous operation and install a fresh flag; returns the new flag.
fn swap_cancel(state: &OllamaState) -> Arc<AtomicBool> {
    let new_flag = Arc::new(AtomicBool::new(false));
    let mut guard = state.cancel.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = guard.take() {
        old.store(true, Ordering::Relaxed);
    }
    *guard = Some(new_flag.clone());
    new_flag
}

/// Status: `installed` (the `ollama` command exists) and `reachable` (the daemon answers now). Never
/// errors for "not running". The Install button keys off `installed`; inference auto-starts the daemon.
#[tauri::command]
pub async fn ollama_status(base_url: String) -> Result<OllamaStatus, String> {
    let (reachable, version) = match client::version(&base_url).await {
        Ok(v) => (true, Some(v)),
        Err(_) => (false, None),
    };
    let installed = if reachable {
        true
    } else {
        tokio::task::spawn_blocking(client::is_installed).await.unwrap_or(false)
    };
    Ok(OllamaStatus { installed, reachable, version })
}

/// Models currently installed in Ollama (`GET /api/tags`).
#[tauri::command]
pub async fn ollama_list_models(base_url: String) -> Result<Vec<OllamaModel>, String> {
    client::list_tags(&base_url).await
}

/// Download (pull) a model, streaming progress; cancelable via `ollama_cancel`.
#[tauri::command]
pub async fn ollama_pull_model(
    base_url: String,
    model: String,
    on_progress: tauri::ipc::Channel<PullProgress>,
    state: tauri::State<'_, OllamaState>,
) -> Result<(), String> {
    client::ensure_running(&base_url).await?;
    let cancel = swap_cancel(state.inner());
    client::pull(&base_url, &model, &cancel, |line| {
        let msg = PullProgress {
            status: line.status,
            digest: line.digest,
            total: line.total,
            completed: line.completed,
        };
        if let Err(e) = on_progress.send(msg) {
            log::warn!("pull progress send failed: {e}");
        }
    })
    .await
}

/// Run the official platform install command at the user's request, then the caller re-checks status.
#[tauri::command]
pub async fn install_ollama() -> Result<(), String> {
    use std::process::Command;
    log::info!("install_ollama: running the official install command");
    let output = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args(["-NoProfile", "-Command", "irm https://ollama.com/install.ps1 | iex"])
            .output()
    } else {
        Command::new("sh")
            .args(["-c", "curl -fsSL https://ollama.com/install.sh | sh"])
            .output()
    };
    let output = output.map_err(|e| format!("failed to launch installer: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        log::error!("install_ollama failed: {err}");
        Err(format!("install failed: {}", err.trim()))
    }
}

/// Request cancellation of the in-flight Ollama operation (pull or batch attribution).
#[tauri::command]
pub fn ollama_cancel(state: tauri::State<'_, OllamaState>) {
    let guard = state.cancel.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(flag) = guard.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Single-photo attribution → parsed result; the frontend applies it to the form (no save).
#[tauri::command]
pub async fn attribute_photo(
    path: String,
    config: AttributionConfig,
) -> Result<AttributionResult, String> {
    attribute::attribute_one(&path, &config).await
}

/// Batch attribution: sequentially attribute and ALWAYS save each photo; stream per-file progress.
#[tauri::command]
pub async fn attribute_batch(
    paths: Vec<String>,
    config: AttributionConfig,
    on_progress: tauri::ipc::Channel<BatchProgress>,
    state: tauri::State<'_, OllamaState>,
) -> Result<Vec<ItemStatus>, String> {
    let cancel = swap_cancel(state.inner());
    Ok(attribute::attribute_batch(&paths, &config, &cancel, |msg| {
        if let Err(e) = on_progress.send(msg) {
            log::warn!("attribute batch progress send failed: {e}");
        }
    })
    .await)
}
