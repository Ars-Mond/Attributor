//! MCP server: lets AI assistants (Claude, ChatGPT, Codex) work with Attributor while the app is open.
//! The app serves Streamable HTTP on 127.0.0.1 ([`http`]); Claude Desktop, which only starts local commands, reaches it through the stdio bridge `attributor --mcp` ([`bridge`]).
//! The tools ([`tools`]) reuse the app's own code paths, and the MCP settings page writes the client entries ([`install`]).

mod bridge;
mod catalog;
mod config;
mod http;
mod install;
mod jobs;
mod metadata;
mod protocol;
mod tools;
mod ui;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

pub(crate) use bridge::run as run_bridge;
use config::McpConfig;
use install::{Client, ClientStatus, Endpoint};

/// Entries kept in the activity log.
const ACTIVITY_LIMIT: usize = 200;

/// Tauri-managed MCP state.
#[derive(Default)]
pub struct McpState {
    config_path: OnceLock<PathBuf>,
    config: Mutex<McpConfig>,
    server: tokio::sync::Mutex<Option<http::Running>>,
    status: Mutex<ServerStatus>,
    ui: ui::UiBridge,
    jobs: jobs::Jobs,
    activity: Mutex<VecDeque<Activity>>,
    /// Serializes changes to metadata, the folder and settings, so concurrent calls cannot interleave their reads and writes.
    write_lock: tokio::sync::Mutex<()>,
    /// Runs one attribution job at a time, as the app does.
    ai_lock: tokio::sync::Mutex<()>,
    /// Serializes changes to the MCP settings and the client files, so concurrent commands cannot lose an update or rewrite one file at once.
    config_lock: tokio::sync::Mutex<()>,
}

/// Whether the HTTP server runs, and why not.
#[derive(Default, Clone)]
struct ServerStatus {
    running: bool,
    error: Option<String>,
}

/// One tool call in the activity log.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Unix time in milliseconds.
    at: u64,
    client: Option<String>,
    tool: String,
    ok: bool,
    /// The error, for a failed call.
    message: Option<String>,
    duration_ms: u64,
}

/// The MCP settings page's view of the server.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpView {
    enabled: bool,
    port: u16,
    token: String,
    allow_ai_settings: bool,
    url: String,
    running: bool,
    error: Option<String>,
    /// The command Claude Desktop starts.
    bridge_command: String,
}

impl McpState {
    fn config(&self) -> McpConfig {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn view(&self) -> McpView {
        let cfg = self.config();
        let status = self
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        McpView {
            url: cfg.url(),
            enabled: cfg.enabled,
            port: cfg.port,
            token: cfg.token,
            allow_ai_settings: cfg.allow_ai_settings,
            running: status.running,
            error: status.error,
            bridge_command: bridge_command(),
        }
    }

    fn endpoint(&self) -> Endpoint {
        let cfg = self.config();
        Endpoint {
            url: cfg.url(),
            token: cfg.token,
            command: bridge_command(),
        }
    }

    /// Applies `change` to the settings and saves them.
    async fn update(&self, change: impl FnOnce(&mut McpConfig)) -> Result<McpConfig, String> {
        let path = self
            .config_path
            .get()
            .cloned()
            .ok_or("the app data directory is unavailable")?;
        let mut next = self.config();
        change(&mut next);
        let saved = next.clone();
        tokio::task::spawn_blocking(move || config::save(&path, &saved))
            .await
            .map_err(|e| e.to_string())??;
        *self.config.lock().unwrap_or_else(|e| e.into_inner()) = next.clone();
        Ok(next)
    }

    fn record(&self, tool: &str, client: Option<String>, error: Option<String>, elapsed: Duration) {
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let entry = Activity {
            at,
            client,
            tool: tool.to_string(),
            ok: error.is_none(),
            message: error,
            duration_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        };
        let mut log = self.activity.lock().unwrap_or_else(|e| e.into_inner());
        if log.len() >= ACTIVITY_LIMIT {
            log.pop_front();
        }
        log.push_back(entry);
    }
}

/// The command Claude Desktop starts the bridge with: this executable, or the AppImage it runs from on Linux (its own path is a temporary mount).
fn bridge_command() -> String {
    if cfg!(target_os = "linux") {
        if let Some(image) = config::non_empty_var("APPIMAGE") {
            return image.display().to_string();
        }
    }
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

/// Loads the MCP settings (creating the token on first run) and starts the server when it is enabled. Called from the app's setup.
pub(crate) fn init(app: &AppHandle) {
    let state = app.state::<McpState>();
    let path = match app.path().app_data_dir() {
        Ok(dir) => dir.join(config::CONFIG_FILE),
        Err(e) => {
            log::error!("mcp: app_data_dir unavailable ({e}); MCP disabled");
            return;
        }
    };
    let mut cfg = config::load(&path);
    if cfg.token.is_empty() {
        match config::new_token() {
            Ok(token) => {
                cfg.token = token;
                if let Err(e) = config::save(&path, &cfg) {
                    log::error!("mcp: {e}");
                }
            }
            Err(e) => log::error!("mcp: cannot create an access token: {e}"),
        }
    }
    *state.config.lock().unwrap_or_else(|e| e.into_inner()) = cfg;
    let _ = state.config_path.set(path);
    let app = app.clone();
    tauri::async_runtime::spawn(async move { apply_server(&app).await });
}

/// Starts, restarts or stops the HTTP server to match the settings.
async fn apply_server(app: &AppHandle) {
    let state = app.state::<McpState>();
    let mut server = state.server.lock().await;
    if let Some(running) = server.take() {
        running.stop().await;
    }
    let cfg = state.config();
    let status = if cfg.enabled {
        match http::start(cfg.port, Arc::new(tools::AppHost::new(app.clone()))).await {
            Ok(running) => {
                log::info!("mcp: listening on 127.0.0.1:{}", running.port);
                *server = Some(running);
                ServerStatus {
                    running: true,
                    error: None,
                }
            }
            Err(e) => {
                log::error!("mcp: {e}");
                ServerStatus {
                    running: false,
                    error: Some(e),
                }
            }
        }
    } else {
        ServerStatus::default()
    };
    *state.status.lock().unwrap_or_else(|e| e.into_inner()) = status;
}

/// Rewrites the HTTP client entries after the port or the token changed.
async fn refresh_clients(state: &McpState) {
    let endpoint = state.endpoint();
    let _ = tokio::task::spawn_blocking(move || install::refresh_http_clients(&endpoint)).await;
}

// ── Commands ──────────────────────────────────────────────────────────────

/// The MCP settings and the server state.
#[tauri::command]
pub fn mcp_get_config(state: tauri::State<'_, McpState>) -> McpView {
    state.view()
}

/// Changes the MCP settings; the server restarts when `enabled` or `port` changes. Turning MCP off also stops running jobs.
#[tauri::command]
pub async fn mcp_set_config(
    app: AppHandle,
    state: tauri::State<'_, McpState>,
    enabled: Option<bool>,
    port: Option<u16>,
    allow_ai_settings: Option<bool>,
) -> Result<McpView, String> {
    if port.is_some_and(|p| p < config::MIN_PORT) {
        return Err(format!(
            "the port must be between {} and 65535",
            config::MIN_PORT
        ));
    }
    let _serial = state.config_lock.lock().await;
    let before = state.config();
    let after = state
        .update(|cfg| {
            if let Some(v) = enabled {
                cfg.enabled = v;
            }
            if let Some(v) = port {
                cfg.port = v;
            }
            if let Some(v) = allow_ai_settings {
                cfg.allow_ai_settings = v;
            }
        })
        .await?;
    if !after.enabled {
        state.jobs.cancel_all();
    }
    if before.enabled != after.enabled || before.port != after.port {
        apply_server(&app).await;
    }
    if before.port != after.port {
        refresh_clients(&state).await;
    }
    Ok(state.view())
}

/// Replaces the access token and rewrites the HTTP client entries that use the old one.
#[tauri::command]
pub async fn mcp_regenerate_token(state: tauri::State<'_, McpState>) -> Result<McpView, String> {
    let _serial = state.config_lock.lock().await;
    let token = config::new_token()?;
    state.update(|cfg| cfg.token = token).await?;
    refresh_clients(&state).await;
    Ok(state.view())
}

/// Recent tool calls, oldest first.
#[tauri::command]
pub fn mcp_activity(state: tauri::State<'_, McpState>) -> Vec<Activity> {
    state
        .activity
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

/// Empties the activity log.
#[tauri::command]
pub fn mcp_clear_activity(state: tauri::State<'_, McpState>) {
    state
        .activity
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

/// Called by the window once it can answer `mcp-request` events.
#[tauri::command]
pub fn mcp_ui_ready(state: tauri::State<'_, McpState>) {
    state.ui.set_ready();
}

/// The window's answer to an `mcp-request` event.
#[tauri::command]
pub fn mcp_ui_reply(
    state: tauri::State<'_, McpState>,
    id: u64,
    ok: bool,
    value: Option<Value>,
    error: Option<String>,
) {
    let result = if ok {
        Ok(value.unwrap_or(Value::Null))
    } else {
        Err(error.unwrap_or_else(|| "the window could not complete the request".into()))
    };
    state.ui.reply(id, result);
}

/// The state of every supported client.
#[tauri::command]
pub async fn mcp_clients(state: tauri::State<'_, McpState>) -> Result<Vec<ClientStatus>, String> {
    let endpoint = state.endpoint();
    tokio::task::spawn_blocking(move || {
        Client::ALL
            .iter()
            .map(|c| install::status(*c, &endpoint))
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Adds or updates Attributor's entry in the client's configuration.
#[tauri::command]
pub async fn mcp_install_client(
    state: tauri::State<'_, McpState>,
    client: Client,
) -> Result<ClientStatus, String> {
    let _serial = state.config_lock.lock().await;
    let endpoint = state.endpoint();
    tokio::task::spawn_blocking(move || install::install(client, &endpoint))
        .await
        .map_err(|e| e.to_string())?
}

/// Removes Attributor's entry from the client's configuration.
#[tauri::command]
pub async fn mcp_uninstall_client(
    state: tauri::State<'_, McpState>,
    client: Client,
) -> Result<ClientStatus, String> {
    let _serial = state.config_lock.lock().await;
    let endpoint = state.endpoint();
    tokio::task::spawn_blocking(move || install::uninstall(client, &endpoint))
        .await
        .map_err(|e| e.to_string())?
}

/// The configuration to add to the client by hand.
#[tauri::command]
pub fn mcp_client_snippet(state: tauri::State<'_, McpState>, client: Client) -> String {
    install::snippet(client, &state.endpoint())
}
