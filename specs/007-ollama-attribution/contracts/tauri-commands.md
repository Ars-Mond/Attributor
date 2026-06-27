# Contract: Tauri IPC commands (feature 007)

All commands follow Constitution IX: `Result<T, String>`, never panic across the boundary, `serde`
`camelCase` payloads, `tauri::ipc::Channel` for streaming. New managed state `OllamaState` holds the
`Arc<AtomicBool>` cancel flag (same shape as `BatchState`).

## Status & models

```rust
#[tauri::command]
async fn ollama_status(base_url: String) -> Result<OllamaStatus, String>;
// { installed: bool, reachable: bool, version: Option<String> }
// installed = `ollama --version` succeeds (or reachable); reachable = /api/version heartbeat.
// Attribution and pull call client::ensure_running() first, which spawns `ollama serve` if down.

#[tauri::command]
async fn ollama_list_models(base_url: String) -> Result<Vec<OllamaModel>, String>;
// [{ name: String, size: u64 }]  (from GET /api/tags)

#[tauri::command]
async fn install_ollama() -> Result<(), String>;
// runs the official per-OS install command via std::process::Command, then the caller re-checks status:
//   macOS/Linux: sh -c "curl -fsSL https://ollama.com/install.sh | sh"
//   Windows:     powershell -NoProfile -Command "irm https://ollama.com/install.ps1 | iex"
// surfaces non-zero exit / stderr as Err(String); may require user elevation/terminal interaction.
```

## Model download (streaming progress, cancelable)

```rust
#[tauri::command]
async fn ollama_pull_model(
    base_url: String,
    model: String,
    on_progress: tauri::ipc::Channel<PullProgress>,
    state: tauri::State<'_, OllamaState>,
) -> Result<(), String>;
// streams PullProgress { status, digest, total, completed } per NDJSON line; cancelable.
```

## Attribution

```rust
#[tauri::command]
async fn attribute_photo(path: String, config: AttributionConfig) -> Result<AttributionResult, String>;
// single image inference -> parsed+validated result; frontend applies to the form (no save).

#[tauri::command]
async fn attribute_batch(
    paths: Vec<String>,
    config: AttributionConfig,
    on_progress: tauri::ipc::Channel<BatchProgress>,   // reused: { index, status: ItemStatus }
    state: tauri::State<'_, OllamaState>,
) -> Result<Vec<ItemStatus>, String>;
// SEQUENTIAL per image: cancel-check -> infer -> read existing metadata -> merge (overwrite text/categories,
// append+dedupe keywords) -> save via existing per-file write -> emit BatchProgress. ALWAYS saves.

#[tauri::command]
fn ollama_cancel(state: tauri::State<'_, OllamaState>);   // sets the cancel flag; safe with nothing running.
```

## Payload types (Rust; `#[serde(rename_all = "camelCase")]`; ts-rs for streamed ones)

```rust
struct OllamaStatus { installed: bool, reachable: bool, version: Option<String> }
struct OllamaModel  { name: String, size: u64 }

struct AttributionConfig {
    base_url: String,
    model: String,
    prompt: String,
    think: Option<Think>,          // untagged enum: Bool(bool) | Level(String)
    keep_alive: Option<String>,
    options: serde_json::Value,    // run options bag (num_ctx, temperature, …)
    format: serde_json::Value,     // the enforced JSON Schema
}

struct AttributionResult {         // only the applied fields; the 3 flags are validated-then-ignored
    title: String,
    description: String,
    keywords: Vec<String>,
    categories: Vec<String>,
}

// events.rs additions (ts-rs exported to src/lib/generated/events.d.ts; events_contract test guards drift)
struct PullProgress { status: String, digest: Option<String>, total: Option<u64>, completed: Option<u64> }
// reused: enum ItemStatus { Ok{path}, Failed{error}, Cancelled }; struct BatchProgress { index, status }
```

## Error & cancellation semantics

- Connection refused → `Err("Ollama not reachable")`-style message; the UI already gates on `ollama_status`.
- Model not found (404) / bad schema (400) / timeout → `Err(String)`, surfaced to the user; no metadata touched.
- Invalid/unparseable model `response` → `Err(String)` (single) or `ItemStatus::Failed` (batch); never a
  partial/corrupt write (FR-015).
- `ollama_cancel` sets the flag; the sequential loop stops before the next item; an in-flight inference may
  finish first; already-saved files remain (FR-019).
- Inference uses a generous read timeout (cold model load); the status heartbeat uses a short connect timeout.
