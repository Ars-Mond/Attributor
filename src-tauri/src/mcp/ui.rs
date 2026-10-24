//! Request/reply channel to the main window.
//! The window owns the open folder, the selection, the editor form and the settings registry, so tools that touch them send an `mcp-request` event and wait for the window to answer through the `mcp_ui_reply` command.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, watch};

use crate::events::{McpRequest, MCP_REQUEST};

/// How long a request waits for the window to finish starting.
const READY_WAIT: Duration = Duration::from_secs(20);
/// Default time the window has to answer.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);

type Pending = oneshot::Sender<Result<Value, String>>;

/// Why a window request failed.
#[derive(Debug)]
pub(crate) enum UiError {
    /// The window did not answer operation `.0` in time; it may still finish the work.
    Timeout(String),
    /// The window refused or could not be reached; the message says why.
    Failed(String),
}

impl From<UiError> for String {
    fn from(e: UiError) -> Self {
        match e {
            UiError::Timeout(op) => format!("the Attributor window did not answer {op} in time"),
            UiError::Failed(message) => message,
        }
    }
}

/// Requests waiting for the window, keyed by id.
pub(crate) struct UiBridge {
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Pending>>,
    ready: watch::Sender<bool>,
}

impl Default for UiBridge {
    fn default() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            ready: watch::channel(false).0,
        }
    }
}

impl UiBridge {
    /// Marks the window ready to answer. A reloaded window starts over, so requests still waiting on the previous page fail.
    pub(crate) fn set_ready(&self) {
        let stale: Vec<Pending> = self.lock().drain().map(|(_, tx)| tx).collect();
        for tx in stale {
            let _ = tx.send(Err("the Attributor window was reloaded".into()));
        }
        self.ready.send_replace(true);
    }

    /// Whether the window has finished starting.
    pub(crate) fn is_ready(&self) -> bool {
        *self.ready.borrow()
    }

    /// Delivers the window's answer to request `id`; unknown ids (timed out or stale) are ignored.
    pub(crate) fn reply(&self, id: u64, result: Result<Value, String>) {
        if let Some(tx) = self.lock().remove(&id) {
            let _ = tx.send(result);
        }
    }

    /// Asks the window to run `op` with `params` and waits up to `timeout` for its answer.
    ///
    /// # Errors
    ///
    /// Returns [`UiError::Timeout`] when the window does not answer in time, and [`UiError::Failed`] with the window's message or with the reason it cannot be reached.
    pub(crate) async fn call(
        &self,
        app: &AppHandle,
        op: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, UiError> {
        self.wait_ready().await.map_err(UiError::Failed)?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.lock().insert(id, tx);
        let request = McpRequest {
            id,
            op: op.to_string(),
            params,
        };
        if let Err(e) = app.emit_to("main", MCP_REQUEST, request) {
            self.lock().remove(&id);
            return Err(UiError::Failed(format!(
                "cannot reach the Attributor window: {e}"
            )));
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result.map_err(UiError::Failed),
            Ok(Err(_)) => Err(UiError::Failed(
                "the Attributor window dropped the request".into(),
            )),
            Err(_) => {
                self.lock().remove(&id);
                Err(UiError::Timeout(op.to_string()))
            }
        }
    }

    async fn wait_ready(&self) -> Result<(), String> {
        let mut rx = self.ready.subscribe();
        let ready = tokio::time::timeout(READY_WAIT, rx.wait_for(|ready| *ready))
            .await
            .is_ok_and(|r| r.is_ok());
        if ready {
            Ok(())
        } else {
            Err("the Attributor window is not ready yet; try again in a moment".into())
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Pending>> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reload_fails_waiting_requests_and_replies_reach_their_caller() {
        let bridge = UiBridge::default();
        assert!(!bridge.is_ready());
        let (tx, rx) = oneshot::channel();
        bridge.lock().insert(7, tx);
        bridge.set_ready();
        assert!(bridge.is_ready());
        assert!(rx.await.unwrap().is_err());

        let (tx, rx) = oneshot::channel();
        bridge.lock().insert(8, tx);
        bridge.reply(9, Ok(Value::Null));
        bridge.reply(8, Ok(Value::Bool(true)));
        assert_eq!(rx.await.unwrap(), Ok(Value::Bool(true)));
    }
}
