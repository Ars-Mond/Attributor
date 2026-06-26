# Phase 1 Data Model: Ollama Vision Auto-Attribution

**Feature**: 007-ollama-attribution | **Date**: 2026-06-26

Types are TypeScript on the frontend and `serde`-`camelCase` Rust across the IPC boundary (Constitution IX).
No new on-disk schema beyond new keys in the existing `settings.json` (`tauri-plugin-store`).

## Persisted settings (registry keys in `settings.json`)

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `ollama.baseUrl` | string | `http://localhost:11434` | Local daemon endpoint (power-user override). |
| `ollama.activeModel` | string | `''` | Installed model id used for attribution (FR-005). Empty ⇒ attribution disabled. |
| `ollama.responseFormat` | string | the fixed JSON Schema (text) | Debug-only enforced `format` schema (FR-006); description warns not to edit. |
| `ollama.modelProfiles` | `ModelProfile[]` (custom) | `[]` | Per-model configs (FR-021–024); contents (default prompts/params) deferred. |

## Entities

### OllamaStatus (transient, from backend)

| Field | Type | Notes |
|-------|------|-------|
| installed | boolean | The `ollama` command exists on the system — drives the Install button. |
| reachable | boolean | `/api/version` heartbeat succeeded — daemon running now. |
| version | string \| null | Daemon version when reachable. |

- Derived **available** = `installed && activeModel != ''` → gates the attribute button (FR-007/009).
  Inference auto-starts the daemon (`ollama serve`) when it is not running.

### OllamaModel (installed; from `/api/tags`)

| Field | Type | Notes |
|-------|------|-------|
| name | string | Model id passed to `/api/generate` (e.g. `llama3.2-vision:11b`). |
| size | number | Bytes on disk (display only). |

- The offered-models list (FR-004) is an app-curated list `{id, label}`: `qwen2.5vl:7b`, `qwen2.5vl:3b`, `qwen3-vl:8b`, `llama3.2-vision:11b`, `gemma4:12b`, `gemma3:12b`.

### ModelProfile (FR-022/024)

| Field | Type | Notes |
|-------|------|-------|
| id | string | Stable internal id (profile identity for list/edit/delete). |
| name | string | Human label shown in the list. |
| modelId | string | The Ollama model id this profile configures. |
| prompt | string | The attribution prompt (default deferred). |
| think | `boolean \| 'low' \| 'medium' \| 'high' \| 'max'` | Thinking mode; top-level request field. |
| keepAlive | string \| null | e.g. `"5m"`; optional. |
| options | `Record<string, string \| number \| boolean>` | `num_ctx`, `temperature`, … → request `options`. |

- Validation: `modelId` and `prompt` are required to drive attribution. Attribution uses the profile whose
  `modelId === activeModel`; if none matches, a built-in default prompt/params (deferred) is used.

### AttributionConfig (frontend → backend per request)

Assembled from settings; passed to the attribution commands so the pure-Rust backend stays settings-agnostic.

| Field | Type | Notes |
|-------|------|-------|
| baseUrl | string | from `ollama.baseUrl`. |
| model | string | active model id. |
| prompt | string | from the matching profile. |
| think | `boolean \| string \| null` | optional. |
| keepAlive | string \| null | optional. |
| options | object | run options bag (skip-empty). |
| format | object (JSON Schema) | parsed from `ollama.responseFormat`. |

### AttributionResult (backend → frontend, single mode)

Parsed from the model's `response` string and validated against the schema.

| Field | Type | Applied as |
|-------|------|-----------|
| title | string | overwrite `title`. |
| description | string | overwrite `description`. |
| keywords | string[] | append to existing `keywords`, de-duplicated. |
| categories | string[] | join with `", "` → overwrite `categories` string. |

- The model also returns `editorial` / `mature_content` / `illustration` booleans; they are validated as
  present but **ignored** (not applied or persisted) this feature (clarification; FR-012).

### Long-running operation (progress overlay state, runes store)

| Field | Type | Notes |
|-------|------|-------|
| active | boolean | Whether the overlay is shown. |
| label | string | Localized description of the current operation. |
| kind | `'determinate' \| 'indeterminate'` | Inference is indeterminate; batch/pull are determinate. |
| value / total | number | Progress (items done / total, or bytes). |
| blocking | boolean | Freezes the rest of the UI when true. |
| cancelable | boolean | Whether the cancel control is shown. |
| onCancel | () => void | Invokes the backend cancel command. |

## Event / channel payloads (Rust `events.rs`, ts-rs exported)

- **Reused**: `ItemStatus` (`Ok{path}` / `Failed{error}` / `Cancelled`) and `BatchProgress {index, status}` —
  used for batch **attribution** progress as well as batch save.
- **New**: `PullProgress { status: string, digest: string|null, total: number|null, completed: number|null }`
  — streamed during model download.

## Relationships

```text
settings.json
 ├─ ollama.baseUrl ─────────────┐
 ├─ ollama.activeModel ─────────┤→ AttributionConfig ──> attribute_photo / attribute_batch (Rust)
 ├─ ollama.responseFormat ──────┤        │                         │
 └─ ollama.modelProfiles[] ──(match modelId == activeModel)         │
                                          │                         ├─ POST /api/generate (images[b64], format=schema)
 OllamaStatus (GET /api/version heartbeat) ─ gates ─ attribute │
 OllamaModel[] (GET /api/tags) ─ fills ─ activeModel dropdown        │
                                                                     ▼
 AttributionResult ─ single: applied to editor form (overwrite text, merge keywords)
                   └ batch: backend merges into existing metadata + saves per file (BatchProgress)
 PullProgress / BatchProgress ─ stream ─> ProgressOverlay (z-index 600, blocking freeze for batch save)
```
