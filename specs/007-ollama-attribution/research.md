# Phase 0 Research: Ollama Vision Auto-Attribution

**Feature**: 007-ollama-attribution | **Date**: 2026-06-26 | **Spec**: [spec.md](./spec.md)

Decisions that resolve the Technical Context unknowns. Verified against the current (2026) Ollama HTTP API.

## Decision 1 — HTTP client to local Ollama: `reqwest` (rustls, async)

**Decision**: Talk to the local Ollama daemon at `http://localhost:11434` (base URL is a setting) with
`reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "stream"] }`.
Build one `Client` (short connect timeout for the heartbeat; generous read timeout for inference). Use our
own serde request/response structs; `.json()` for non-streaming calls; `.bytes_stream()` + `futures-util`
`StreamExt` to read NDJSON progress from `/api/pull`.

**Rationale**: `rustls-tls` is pure Rust (no OpenSSL/native-tls) — Constitution I; the local API is plain
HTTP so TLS is not even on the hot path. It reuses the in-tree async stack (tokio + Tauri runtime), so async
Tauri commands (Result<T,String> + `tauri::ipc::Channel`) compose naturally with streaming pull progress
(Constitution IX/VIII). serde/serde_json are already present, giving full control over the strict-JSON
`format` schema and the `think`/`options` fields — less surface than a wrapper crate (Constitution X).

**Alternatives**: `ollama-rs 0.3` (typed wrapper — convenient but ships on top of reqwest and lags our exact
schema, weaker under X); `ureq 3` (blocking, leanest deps — but bridges awkwardly to async Channel
streaming); plain reqwest with no TLS feature (even leaner, HTTP-only — viable since localhost, but keep
rustls for robustness); the OpenAI-compatible `/v1` endpoint (no native pull-progress or simple `images[]`).

## Decision 2 — New dependencies (Constitution X)

| Crate | Why |
|-------|-----|
| `reqwest 0.12` (default-features off; `rustls-tls`, `json`, `stream`) | async HTTP to the local Ollama API; pure-Rust TLS |
| `base64 0.22` | encode image bytes into the base64 strings the `/api/generate` `images` array needs |
| `futures-util 0.3` | `StreamExt::next()` to consume `bytes_stream()` and read NDJSON pull progress |

Already present (no new dep): `tokio`, `serde`/`serde_json`, `image 0.25` (optional downscale before base64),
`tauri-plugin-opener` (guided install), `rayon` (CPU-bound image prep if needed).

## Decision 3 — Availability detection & guided install

**Availability**: two-level check against the base URL. **Reachable** = `GET /api/version` succeeds within a
short timeout (daemon up). If it fails, **probe the binary** cross-platform (`ollama`/`ollama.exe` on PATH;
Windows `%LOCALAPPDATA%\Programs\Ollama\ollama.exe`, macOS `/usr/local/bin/ollama` or `/Applications/Ollama.app`,
Linux `/usr/local/bin/ollama`/`/usr/bin/ollama`) → "installed but not running" vs "not installed". The status
is `{reachable, installed, version}`; attribution is available only when `reachable && activeModel != ""`
(FR-007). The heartbeat is cheap, debounced, and never panics (Result).

**Install (FR-003)**: guided only (per clarification default). Use the existing `tauri-plugin-opener` to open
`https://ollama.com/download` (optionally OS-deep-linked via `std::env::consts::OS`). No scripted/unattended
installer — respects Constitution I and the spec assumption. No new dependency.

## Decision 4 — Ollama endpoints used

| Need | Endpoint | Notes |
|------|----------|-------|
| Heartbeat / version | `GET /api/version` | `{version}`; short connect timeout |
| Installed models (FR-005) | `GET /api/tags` | `{models:[{name,model,size,details,…}]}`; use `name` as the id |
| Download a model (FR-004) | `POST /api/pull` `{model, stream:true}` | NDJSON lines `{status,digest,total,completed}`; `completed` absent until bytes flow; per-layer totals — compute % per current digest; on `success` refresh `/api/tags` |
| Single inference (FR-010/011) | `POST /api/generate` `{model, prompt, images:[b64], stream:false, format:<schema>, options:{…}, think?, keep_alive}` | one object back; the strict JSON is the `response` STRING — parse + re-validate |

## Decision 5 — Strict JSON (structured output)

Set the top-level `format` to the **fixed JSON Schema** (not just `"json"`) so the output is constrained to
the exact structure. The settings debug field (FR-006) holds this schema as text → deserialized to
`serde_json::Value` → dropped into `format`. Best practice for reliability: also say "return as JSON
matching the schema" in the prompt and set `options.temperature = 0`. The model can still drift, so the
backend MUST re-parse and validate the `response` string against the schema before applying (FR-015). The
`editorial`/`mature_content`/`illustration` booleans are present in the schema (the model returns them) but
parsed-and-ignored this feature (clarification; FR-012).

## Decision 6 — Run parameters & model profile shape

A model profile (FR-022/024) maps onto the request as: top-level `model`, `prompt`, `think`
(bool **or** an effort level `"low"|"medium"|"high"|"max"` — top-level, not in `options`; honored only by
thinking-capable models), optional `keep_alive`, and a nested `options` bag (`num_ctx`/context length,
`temperature`, `top_k`, `top_p`, `seed`, `num_predict`, …). Serde: `think` as an untagged enum
`{Bool(bool)|Level(String)}`; `options` as a flexible map; absent fields skip-serialize so deferred defaults
apply. Default prompt/params and the offered-model list are deferred (supplied later) — only the structures
are built here.

## Decision 7 — Progress, cancellation & batch concurrency (reuse feature 004)

**Reuse** the feature-004 streaming/cancel infrastructure: `tauri::ipc::Channel<T>` for progress and an
`Arc<AtomicBool>` cancel flag held in managed state (`swap_cancel`/`cancel_*` pattern in `batch/mod.rs`,
`ItemStatus`/`BatchProgress` in `events.rs`).

- **Batch attribution is SEQUENTIAL**, not rayon-parallel: a single local Ollama serializes inference, so
  concurrent `/api/generate` calls would just queue. A sequential async loop keeps progress ordered and
  cancel responsive (Constitution VIII — the heavy work is in Ollama, not our process; we await one call at
  a time and stream one `BatchProgress` per item). Each item: cancel-check → infer → read existing metadata
  → merge → save → emit status. (This differs from batch **save**, which stays rayon-parallel for file I/O.)
- **Cancellation is cooperative**: stop before the next item / after the current inference returns; dropping
  the in-flight reqwest future aborts a running pull.
- **Pull progress** streams `PullProgress {status, digest, total, completed}` over a Channel.

## Decision 8 — Reusable global progress overlay (Constitution V, II)

A single Svelte 5 component `ProgressOverlay` driven by a runes store (`progress.svelte.ts`), mounted once in
`+page.svelte`, at **z-index 600** — above the loading overlay/dialogs (200) and `SettingsDialog` (500), below
the DockLayout drag ghost (9999). Store API: `progress.run({label, total?, cancelable, onCancel})`,
`update({value,total,label})`, `done()`. A **blocking** operation covers the screen and swallows pointer
events (freeze); cancel invokes the registered `onCancel` (which calls the backend cancel command). Per the
clarification, **batch save is routed through this overlay with a freeze**, and so is batch attribution and
model pull — one operation at a time (a second request is rejected while one is active).

## Decision 9 — Metadata mapping & application

The attribution result maps to the editor: `title`/`description` overwrite; `categories[]` joined with
`", "` overwrite the categories string; `keywords[]` appended to existing and de-duplicated (FR-010/012,
clarification). The three booleans are ignored. Reuse the existing `Metadata`/`SaveRequest` types and the
`save_metadata` / per-file `save_one` write path:

- **Single mode**: command returns an `AttributionResult`; the frontend applies it to the form and marks it
  dirty; the user saves (no auto-save).
- **Batch mode**: the backend, per image, reads existing metadata, applies the result (merge), and saves via
  the existing per-file save — always saving — streaming `BatchProgress`.

## Decision 10 — Settings persistence (reuse existing store)

Reuse `tauri-plugin-store` (settings.json) via the settings registry. The Ollama and "Ollama Models"
categories are **custom section components** (`registerSection({component})`, as `ShortcutsPage` does). Keys:
`ollama.baseUrl`, `ollama.activeModel`, `ollama.responseFormat` (schema text), `ollama.modelProfiles`
(array of profiles). No separate storage system is introduced (spec assumption). The per-model editor is a
popup component (reuse the `ConfirmDialog`/dialog overlay pattern) with Save/Cancel.
