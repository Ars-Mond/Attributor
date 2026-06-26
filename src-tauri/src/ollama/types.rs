//! Types crossing the IPC boundary for Ollama vision attribution. All `camelCase` (Constitution IX).

use serde::{Deserialize, Serialize};

/// Ollama presence: `installed` = the `ollama` command exists on the system (drives the Install button);
/// `reachable` = the daemon answers the heartbeat now. Inference auto-starts the daemon when needed.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    pub installed: bool,
    pub reachable: bool,
    pub version: Option<String>,
}

/// One locally-installed model (from `GET /api/tags`).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
}

/// Thinking mode: a boolean, or an effort level ("low"|"medium"|"high"|"max"). Top-level request field.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Think {
    Bool(bool),
    Level(String),
}

/// Per-request config assembled by the frontend from settings + the active model's profile.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AttributionConfig {
    pub base_url: String,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub think: Option<Think>,
    #[serde(default)]
    pub keep_alive: Option<String>,
    /// Run options bag (num_ctx, temperature, …) — passed through to the request `options`.
    #[serde(default)]
    pub options: serde_json::Value,
    /// The enforced JSON Schema (request `format`).
    pub format: serde_json::Value,
}

/// Applied fields of a single attribution. The model also returns editorial/mature_content/illustration,
/// which are accepted in the schema but ignored this feature (deferred follow-up).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AttributionResult {
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}
