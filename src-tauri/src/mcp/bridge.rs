//! The stdio bridge `attributor --mcp`, for clients that start local commands (Claude Desktop).
//! It answers discovery itself from the built-in catalogue, so the client sees the tools even while the app is closed, and forwards tool calls to the running app's HTTP endpoint with the token from `mcp.json`.
//! stdout carries only protocol messages; diagnostics go to stderr, which the clients keep in their logs.

use std::future::Future;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::config::{self, McpConfig, CONFIG_FILE};
use super::protocol::{
    self, strip_modern_fields, tool_error, CallContext, ToolHost, META_CLIENT_CAPABILITIES,
    META_CLIENT_INFO, META_VERSION, MODERN_VERSION,
};

/// How long calls still running when stdin closes may take to answer.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound for one forwarded call; tools themselves return within a few minutes.
const CALL_TIMEOUT: Duration = Duration::from_secs(900);

const NOT_RUNNING: &str =
    "Attributor is not running. Ask the user to open Attributor, then call the tool again.";
const TURNED_OFF: &str = "The MCP server is turned off in Attributor. Ask the user to turn it on in Attributor > Settings > MCP, then call the tool again.";

/// Runs the bridge until stdin closes and returns the process exit code.
pub(crate) fn run() -> i32 {
    tauri::async_runtime::block_on(serve())
}

async fn serve() -> i32 {
    eprintln!("attributor: MCP bridge started (protocol {MODERN_VERSION} and legacy)");
    let (out_tx, out_rx) = std::sync::mpsc::channel::<String>();
    let writer = std::thread::spawn(move || {
        let mut stdout = std::io::stdout().lock();
        for line in out_rx {
            if writeln!(stdout, "{line}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                break;
            }
        }
    });
    let (in_tx, mut in_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if in_tx.send(line).is_err() {
                break;
            }
        }
    });

    let host = Arc::new(Forwarder::new());
    let mut calls = tokio::task::JoinSet::new();
    while let Some(line) = in_rx.recv().await {
        if line.trim().is_empty() {
            continue;
        }
        host.note_client(&line);
        let host = host.clone();
        let out = out_tx.clone();
        // Each message runs on its own, so a long tool call never delays a ping.
        calls.spawn(async move {
            let reply = protocol::handle(&*host, line.as_bytes(), None).await;
            if let Some(body) = reply.body {
                let _ = out.send(body.to_string());
            }
        });
        while calls.try_join_next().is_some() {}
    }
    let _ = tokio::time::timeout(DRAIN_TIMEOUT, async {
        while calls.join_next().await.is_some() {}
    })
    .await;
    // Calls still running hold senders of the writer's channel; ending them lets the writer finish.
    calls.shutdown().await;
    drop(out_tx);
    let _ = tokio::task::spawn_blocking(move || writer.join()).await;
    eprintln!("attributor: MCP bridge stopped (stdin closed)");
    0
}

/// Forwards tool calls to the app.
struct Forwarder {
    http: reqwest::Client,
    /// `clientInfo` from the legacy `initialize`, passed along so the app's activity log names the client.
    client_info: Mutex<Option<Value>>,
}

impl Forwarder {
    fn new() -> Self {
        // Loopback only: a system proxy must not see the token.
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(CALL_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            http,
            client_info: Mutex::new(None),
        }
    }

    fn note_client(&self, line: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if msg["method"] == "initialize" {
            if let Some(info) = msg.pointer("/params/clientInfo") {
                *self.client_info.lock().unwrap_or_else(|e| e.into_inner()) = Some(info.clone());
            }
        }
    }

    fn client_info(&self) -> Value {
        self.client_info
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_else(
                || json!({"name": "stdio-bridge", "version": env!("CARGO_PKG_VERSION")}),
            )
    }
}

impl ToolHost for Forwarder {
    fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        ctx: CallContext,
    ) -> impl Future<Output = Value> + Send {
        let http = self.http.clone();
        let name = name.to_string();
        let info = self.client_info();
        async move {
            match forward(&http, &name, arguments, info).await {
                Ok(mut result) => {
                    if !ctx.modern {
                        strip_modern_fields(&mut result);
                    }
                    result
                }
                Err(message) => tool_error(message),
            }
        }
    }
}

async fn load_config() -> Option<McpConfig> {
    let path = config::app_data_dir()?.join(CONFIG_FILE);
    tokio::task::spawn_blocking(move || path.is_file().then(|| config::load(&path)))
        .await
        .ok()
        .flatten()
}

/// Sends one `tools/call` to the app as a modern request and returns its result.
async fn forward(
    http: &reqwest::Client,
    name: &str,
    arguments: Value,
    client_info: Value,
) -> Result<Value, String> {
    let cfg = load_config().await.ok_or(NOT_RUNNING)?;
    if !cfg.enabled || cfg.token.is_empty() {
        return Err(TURNED_OFF.into());
    }
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments,
            "_meta": {
                META_VERSION: MODERN_VERSION,
                META_CLIENT_CAPABILITIES: {},
                META_CLIENT_INFO: client_info,
            },
        },
    });
    let response = http
        .post(cfg.url())
        .bearer_auth(&cfg.token)
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .header("MCP-Protocol-Version", MODERN_VERSION)
        .header("Mcp-Method", "tools/call")
        .header("Mcp-Name", name)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                NOT_RUNNING.to_string()
            } else {
                format!("cannot reach Attributor: {e}")
            }
        })?;
    let status = response.status();
    let reply: Value = response
        .json()
        .await
        .map_err(|e| format!("unexpected answer from Attributor (HTTP {status}): {e}"))?;
    if let Some(result) = reply.get("result") {
        return Ok(result.clone());
    }
    let message = reply
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("unknown error");
    Err(format!(
        "Attributor refused the call (HTTP {status}): {message}"
    ))
}
