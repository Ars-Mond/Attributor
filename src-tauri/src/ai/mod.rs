//! Vision auto-attribution: provider clients (Ollama, local OpenAI-compatible servers, OpenAI, ChatGPT, Claude), model listing, credentials and single/batch attribution. Progress streams over `tauri::ipc::Channel`; a shared `Arc<AtomicBool>` (in `AiState`)
//! gives cooperative cancellation, mirroring the `batch` module.

mod anthropic;
pub mod attribute;
mod chatgpt;
mod http;
pub(crate) mod image;
mod lmstudio;
mod ollama;
mod openai;
mod openai_compat;
mod schema;
mod secrets;
pub mod types;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::events::{BatchProgress, ItemStatus, PullProgress};
use types::{
    AttributionConfig, AttributionResult, ChatGptAccount, ModelInfo, Provider, ProviderStatus,
};

/// Tauri-managed state: the in-flight operation's cancel flag and the ChatGPT sign-in session.
#[derive(Default)]
pub struct AiState {
    pub cancel: Mutex<Option<Arc<AtomicBool>>>,
    chatgpt: chatgpt::Session,
}

impl AiState {
    /// Sets where sealed credentials are stored (the app's local data directory, so they do not roam with the profile).
    pub fn set_storage_dir(&self, dir: std::path::PathBuf) {
        self.chatgpt.set_storage_dir(dir);
    }

    /// Attributes one photo under the caller's own cancel flag, leaving the shared one (and so the UI's operation) alone. Used by MCP jobs.
    ///
    /// # Errors
    ///
    /// Returns the provider's or the parser's error message, or `cancelled`.
    pub(crate) async fn attribute_detached(
        &self,
        path: &str,
        config: &AttributionConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AttributionResult, String> {
        attribute::attribute_one(path, config, cancel, &self.chatgpt).await
    }
}

/// Cancel any previous operation and install a fresh flag; returns the new flag.
fn swap_cancel(state: &AiState) -> Arc<AtomicBool> {
    let new_flag = Arc::new(AtomicBool::new(false));
    let mut guard = state.cancel.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = guard.take() {
        old.store(true, Ordering::Relaxed);
    }
    *guard = Some(new_flag.clone());
    new_flag
}

/// Provider status. Never errors for "not running" or "not configured": those are reported in the status itself.
/// Ollama: `installed` = the `ollama` command exists (drives the Install button), `reachable` = the daemon answers now.
/// Local servers: `reachable` = the model list answers; LM Studio also reports `installed` when its `lms` CLI exists.
/// Cloud: `reachable` = the credentials are accepted (the model list answers).
#[tauri::command]
pub async fn ai_status(
    provider: Provider,
    base_url: String,
    state: tauri::State<'_, AiState>,
) -> Result<ProviderStatus, String> {
    let status = match provider {
        Provider::Ollama => {
            let (reachable, version) = match ollama::version(&base_url).await {
                Ok(v) => (true, Some(v)),
                Err(_) => (false, None),
            };
            let installed = if reachable {
                true
            } else {
                tokio::task::spawn_blocking(ollama::is_installed)
                    .await
                    .unwrap_or(false)
            };
            ProviderStatus {
                installed,
                reachable,
                version,
                detail: None,
            }
        }
        Provider::Chatgpt => match state.chatgpt.check().await {
            Ok(()) => ProviderStatus {
                installed: true,
                reachable: true,
                ..Default::default()
            },
            Err(detail) => ProviderStatus {
                detail: Some(detail),
                ..Default::default()
            },
        },
        _ => {
            let reachable = list_models_inner(provider, &base_url, &state).await;
            let installed = match (&reachable, provider) {
                (Ok(_), _) => true,
                (Err(_), Provider::Lmstudio) => tokio::task::spawn_blocking(lmstudio::is_installed)
                    .await
                    .unwrap_or(false),
                (Err(_), _) => false,
            };
            ProviderStatus {
                installed,
                reachable: reachable.is_ok(),
                version: None,
                detail: reachable.err(),
            }
        }
    };
    Ok(status)
}

async fn list_models_inner(
    provider: Provider,
    base_url: &str,
    state: &AiState,
) -> Result<Vec<ModelInfo>, String> {
    match provider {
        Provider::Ollama => ollama::list_tags(base_url).await,
        Provider::Lmstudio => {
            lmstudio::list_models(
                base_url,
                secrets::optional_api_key(provider).await.as_deref(),
            )
            .await
        }
        Provider::Llamacpp | Provider::Jan | Provider::Vllm | Provider::Unsloth => {
            openai_compat::list_models(
                provider,
                base_url,
                secrets::optional_api_key(provider).await.as_deref(),
            )
            .await
        }
        Provider::Openai => match secrets::load_api_key(provider).await? {
            Some(key) => {
                http::check_key_destination(base_url)?;
                openai::list_models(base_url, &key).await
            }
            None => Err("OpenAI API key is not set".to_string()),
        },
        Provider::Anthropic => match secrets::load_api_key(provider).await? {
            Some(key) => {
                http::check_key_destination(base_url)?;
                anthropic::list_models(base_url, &key).await
            }
            None => Err("Claude API key is not set".to_string()),
        },
        Provider::Chatgpt => state.chatgpt.list_models().await,
    }
}

/// Models the provider offers: installed (Ollama), served or loadable (local servers), or available to the account (cloud).
#[tauri::command]
pub async fn ai_list_models(
    provider: Provider,
    base_url: String,
    state: tauri::State<'_, AiState>,
) -> Result<Vec<ModelInfo>, String> {
    list_models_inner(provider, &base_url, &state).await
}

/// Store the provider's API key in the OS keychain; an empty key removes it. The key never travels back to the frontend.
#[tauri::command]
pub async fn ai_set_api_key(provider: Provider, key: String) -> Result<(), String> {
    if !provider.uses_api_key() {
        return Err(format!("{} does not use an API key", provider.label()));
    }
    tokio::task::spawn_blocking(move || secrets::set_api_key(provider, key.trim()))
        .await
        .map_err(|e| e.to_string())?
}

/// Whether an API key is stored for the provider.
#[tauri::command]
pub async fn ai_has_api_key(provider: Provider) -> Result<bool, String> {
    Ok(secrets::load_api_key(provider).await?.is_some())
}

/// Sign in with ChatGPT: opens the browser on OpenAI's sign-in page and waits for the redirect. Cancelable via `ai_cancel`.
/// `new_account` registers the app again, for signing in with a different ChatGPT account or workspace.
#[tauri::command]
pub async fn chatgpt_login(
    app: tauri::AppHandle,
    new_account: Option<bool>,
    state: tauri::State<'_, AiState>,
) -> Result<ChatGptAccount, String> {
    let cancel = swap_cancel(state.inner());
    state
        .chatgpt
        .login(&app, &cancel, new_account.unwrap_or(false))
        .await
}

/// Forget the ChatGPT sign-in (removes the stored tokens).
#[tauri::command]
pub async fn chatgpt_logout(state: tauri::State<'_, AiState>) -> Result<(), String> {
    state.chatgpt.logout().await
}

/// The signed-in ChatGPT account, or `None` when signed out.
#[tauri::command]
pub async fn chatgpt_account(
    state: tauri::State<'_, AiState>,
) -> Result<Option<ChatGptAccount>, String> {
    state.chatgpt.account().await
}

/// Download (pull) a model, streaming progress; cancelable via `ai_cancel`.
#[tauri::command]
pub async fn ollama_pull_model(
    base_url: String,
    model: String,
    on_progress: tauri::ipc::Channel<PullProgress>,
    state: tauri::State<'_, AiState>,
) -> Result<(), String> {
    ollama::ensure_running(&base_url).await?;
    let cancel = swap_cancel(state.inner());
    ollama::pull(&base_url, &model, &cancel, |line| {
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
            .args([
                "-NoProfile",
                "-Command",
                "irm https://ollama.com/install.ps1 | iex",
            ])
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

/// Request cancellation of the in-flight operation (model pull, ChatGPT sign-in, or single/batch attribution).
#[tauri::command]
pub fn ai_cancel(state: tauri::State<'_, AiState>) {
    let guard = state.cancel.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(flag) = guard.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Single-photo attribution → parsed result; the frontend applies it to the form (no save).
/// Cancelable via `ai_cancel` (the progress overlay's Cancel button) — aborts the inference.
#[tauri::command]
pub async fn attribute_photo(
    path: String,
    config: AttributionConfig,
    state: tauri::State<'_, AiState>,
) -> Result<AttributionResult, String> {
    let cancel = swap_cancel(state.inner());
    attribute::attribute_one(&path, &config, &cancel, &state.chatgpt).await
}

/// Batch attribution: sequentially attribute each photo and store the result as app-only (feature 008
/// — files are not modified); stream per-file progress.
#[tauri::command]
pub async fn attribute_batch(
    paths: Vec<String>,
    config: AttributionConfig,
    on_progress: tauri::ipc::Channel<BatchProgress>,
    state: tauri::State<'_, AiState>,
    db: tauri::State<'_, crate::store::DbState>,
) -> Result<Vec<ItemStatus>, String> {
    let cancel = swap_cancel(state.inner());
    let db = db.share();
    Ok(
        attribute::attribute_batch(&paths, &config, &cancel, &db, &state.chatgpt, |msg| {
            if let Err(e) = on_progress.send(msg) {
                log::warn!("attribute batch progress send failed: {e}");
            }
        })
        .await,
    )
}
