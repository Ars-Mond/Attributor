//! Types crossing the IPC boundary for vision attribution. All `camelCase` (Constitution IX).

use serde::{Deserialize, Serialize};

/// Inference backend. Serialized as the lowercase id the frontend stores in `ai.provider`.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Ollama,
    Llamacpp,
    Lmstudio,
    Jan,
    Vllm,
    Unsloth,
    Openai,
    Chatgpt,
    Anthropic,
}

impl Provider {
    /// The serialized id (`ollama`, `llamacpp`, …), used to name credential-store entries.
    pub fn id(self) -> &'static str {
        match self {
            Provider::Ollama => "ollama",
            Provider::Llamacpp => "llamacpp",
            Provider::Lmstudio => "lmstudio",
            Provider::Jan => "jan",
            Provider::Vllm => "vllm",
            Provider::Unsloth => "unsloth",
            Provider::Openai => "openai",
            Provider::Chatgpt => "chatgpt",
            Provider::Anthropic => "anthropic",
        }
    }

    /// Human-readable name used in log lines and error messages.
    pub fn label(self) -> &'static str {
        match self {
            Provider::Ollama => "Ollama",
            Provider::Llamacpp => "llama.cpp",
            Provider::Lmstudio => "LM Studio",
            Provider::Jan => "Jan",
            Provider::Vllm => "vLLM",
            Provider::Unsloth => "Unsloth",
            Provider::Openai => "OpenAI",
            Provider::Chatgpt => "ChatGPT",
            Provider::Anthropic => "Claude",
        }
    }

    /// Local servers that speak the OpenAI Chat Completions protocol.
    pub fn is_openai_compatible(self) -> bool {
        matches!(
            self,
            Provider::Llamacpp
                | Provider::Lmstudio
                | Provider::Jan
                | Provider::Vllm
                | Provider::Unsloth
        )
    }

    /// Whether the provider authenticates with an API key stored in the OS keychain (optional for local servers).
    pub fn uses_api_key(self) -> bool {
        self.is_openai_compatible() || matches!(self, Provider::Openai | Provider::Anthropic)
    }
}

/// Provider presence. `installed`: the provider's command-line tool exists (Ollama, LM Studio) or, for providers without one, the same as `reachable`.
/// `reachable`: the server or cloud API answered now (with valid credentials for cloud providers). `detail`: why the check failed.
/// Inference auto-starts Ollama and LM Studio when they are installed but not running.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub installed: bool,
    pub reachable: bool,
    pub version: Option<String>,
    pub detail: Option<String>,
}

/// One model offered by a provider. `size` is the on-disk size in bytes when the provider reports it.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
}

/// Thinking mode: a boolean, or an effort level ("low"|"medium"|"high"|"max"|…). Mapped per provider.
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
    #[serde(default)]
    pub provider: Provider,
    /// Server root (Ollama), API root including `/v1` (OpenAI-compatible servers, OpenAI) or API host (Claude). Unused for ChatGPT.
    #[serde(default)]
    pub base_url: String,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub think: Option<Think>,
    /// Ollama-only: how long the model stays loaded after the request.
    #[serde(default)]
    pub keep_alive: Option<String>,
    /// Run options bag: Ollama's request `options` (num_ctx, temperature, …); top-level request fields for every other provider.
    #[serde(default)]
    pub options: serde_json::Value,
    /// The enforced JSON Schema (Ollama `format`, `response_format` / `text.format` / `output_config.format` elsewhere).
    pub format: serde_json::Value,
}

/// Applied fields of a single attribution. The editorial/mature_content/illustration flags are surfaced
/// to the frontend as checkboxes; they are not written to file metadata.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AttributionResult {
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub editorial: bool,
    pub mature_content: bool,
    pub illustration: bool,
}

/// The ChatGPT account the app is signed in with. Sign in with ChatGPT does not reveal the plan type to apps.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptAccount {
    pub email: Option<String>,
}
