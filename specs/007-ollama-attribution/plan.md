# Implementation Plan: Ollama Vision Auto-Attribution

**Branch**: `007-ollama-attribution` | **Date**: 2026-06-26 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/007-ollama-attribution/spec.md`

## Summary

Add local-Ollama vision auto-attribution: a pure-Rust backend talks to the local Ollama daemon
(`reqwest` + rustls) to detect availability, list/pull models, and run a single-image inference that returns a
schema-locked JSON (`format`). The metadata panel gets an "Attribute via Ollama" action — single mode fills
the editor (title/description/categories overwritten, keywords appended+de-duped; the three flags are
ignored this feature), batch mode attributes every selected photo sequentially and always saves. A reusable
top-most progress overlay with cancel surfaces long operations and freezes the UI for batch save and batch
attribution, reusing the feature-004 `Channel`+`Arc<AtomicBool>` pattern. Settings gain an **Ollama** custom
section (check/install/download/select-model/JSON-format) and an **Ollama Models** custom section managing
per-model profiles (id, run params, prompt) via a list + Save/Cancel popup, persisted in the existing store.
Default prompts and the offered-model list are deferred. See [research.md](./research.md) for the decisions.

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript ~5.6 + Svelte 5 (runes); Tauri 2.

**Primary Dependencies**: Existing stack + **new Rust crates** `reqwest 0.12` (default-features off;
`rustls-tls`, `json`, `stream`), `base64 0.22`, `futures-util 0.3`. No new npm dependency. Reuses present
`tokio`, `serde`/`serde_json`, `image 0.25`, `tauri-plugin-opener`, `rayon`, and the i18n layer.

**Storage**: Existing `settings.json` via `tauri-plugin-store` — new keys `ollama.baseUrl`,
`ollama.activeModel`, `ollama.responseFormat`, `ollama.modelProfiles`. No new storage system.

**Testing**: `cargo test` (backend + the `events_contract` ts-rs drift guard for the new `PullProgress`);
`npm run check` (svelte-check, incl. new i18n keys). Manual validation per [quickstart.md](./quickstart.md).
(vitest/playwright remain out of scope per prior project decision.)

**Target Platform**: Windows, macOS, Linux desktop; Ollama runs locally (`http://localhost:11434`).

**Performance Goals**: UI never blocks on inference — HTTP/inference run on async commands off the UI thread;
progress streams over a Channel (no IPC in hot loops); batch attribution is sequential (Ollama serializes
inference) with prompt, responsive cancel.

**Constraints**: pure-Rust backend, rustls not native-tls (I); runes-only frontend (II); typed IPC with
`Channel` streaming and `Result<T,String>` (IX); reuse existing primitives (V); minimal/justified deps (X);
guided install only (no unattended installer).

**Scale/Scope**: Two new settings categories + a reusable overlay + an attribute action; ~8 new Tauri
commands; 1 new event payload (`PullProgress`) plus reuse of `BatchProgress`/`ItemStatus`; batches of dozens
to hundreds of photos. Default prompts/offered-models/params are deferred content (structures only).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | How this plan complies |
|-----------|--------|------------------------|
| I. Pure Rust Backend | ✅ | `reqwest` with `rustls-tls` (no OpenSSL/native-tls); `base64`/`futures-util` pure Rust. |
| II. Modern Svelte 5 (Runes) | ✅ | Progress store + overlay + settings pages use `$state`/`$derived`; no legacy stores. |
| III. Themed SCSS Tokens | ✅ | Overlay, settings pages, and the model popup use existing tokens/mixins. |
| IV. Cross-Platform Parity | ✅ | localhost API everywhere; `opener` for install; per-OS binary probe only smooths detection, never gates a feature. |
| V. Reuse UI Primitives | ✅ | Reuses the `Channel`+cancel pattern (004), settings custom-section registration, the dialog overlay, i18n. |
| VI. Mandatory Logging | ✅ | Backend logs Ollama calls/errors (`log`); frontend logs invoke failures (`@tauri-apps/plugin-log`). |
| VII. Phase-Based Commits | ✅ | This plan is committed as the `plan` phase. |
| VIII. Rust Performance First | ✅ | Inference/HTTP/image-prep in Rust off the UI thread; progress over a Channel; batch attribution sequential (Ollama-bound); batch save stays rayon. |
| IX. Typed Tauri IPC | ✅ | All new commands `Result<T,String>`, camelCase, never panic; `tauri::ipc::Channel` for pull/batch progress; `PullProgress` ts-rs-guarded. |
| X. Fixed Stack | ✅ | Three new justified Rust crates (research.md Decision 2); zero new npm. |
| XI. Code Style | ✅ | English comments/identifiers; no inner brace spaces in TS. |
| Comms & Docs | ✅ | All new UI localized (en+ru); Help may gain an Ollama section (English source mirrored to ru). |

**Result**: PASS — no violations. Complexity Tracking is empty (new deps are justified, not violations).

## Project Structure

### Documentation (this feature)

```text
specs/007-ollama-attribution/
├── plan.md              # This file
├── research.md          # Phase 0 — decisions
├── data-model.md        # Phase 1 — entities, settings keys, payloads
├── quickstart.md        # Phase 1 — validation scenarios
├── contracts/
│   ├── tauri-commands.md # IPC commands + payloads
│   ├── ollama-http.md    # external Ollama HTTP API consumed by the backend
│   └── frontend-ui.md    # overlay store, settings sections, attribute action
├── checklists/requirements.md
└── tasks.md             # Phase 2 — /speckit-tasks (not here)
```

### Source Code (repository root)

```text
src-tauri/
├── Cargo.toml                 # EDIT — add reqwest, base64, futures-util
└── src/
    ├── ollama/                # NEW — backend module
    │   ├── mod.rs            # OllamaState (cancel flag), command entry points
    │   ├── client.rs         # reqwest client: version, tags, pull(stream), generate(vision)
    │   ├── attribute.rs      # single + sequential batch attribution; merge + save reuse
    │   └── types.rs          # AttributionConfig, AttributionResult, OllamaStatus, OllamaModel, Think
    ├── events.rs              # EDIT — add PullProgress (ts-rs); reuse BatchProgress/ItemStatus
    └── lib.rs                 # EDIT — register ollama commands + manage(OllamaState)

src/lib/
├── ollama/                    # NEW — frontend glue
│   └── ollama.ts             # invoke wrappers + AttributionConfig assembly from settings
├── reusable/
│   └── ProgressOverlay.svelte # NEW — global top-most overlay (z-index 600)
├── progress.svelte.ts         # NEW — runes progress store (run/update/done, blocking, cancel)
├── settings/
│   ├── index.ts              # EDIT — register 'ollama' + 'ollama-models' sections; new keys
│   ├── OllamaSettingsPage.svelte    # NEW — check/install/download/select/JSON-format
│   ├── OllamaModelsPage.svelte      # NEW — profile list + Create/Edit/Delete
│   └── OllamaModelDialog.svelte     # NEW — profile editor popup (Save/Cancel)
├── panel/MetadataPanel.svelte # EDIT — Attribute button (single+batch), apply result, batch via overlay
├── i18n/{en,ru}.ts            # EDIT — new keys (settings.ollama.*, ollama.*)
└── generated/events.d.ts      # EDIT (regen) — PullProgress

src/routes/+page.svelte        # EDIT — mount <ProgressOverlay /> once
```

**Structure Decision**: A new backend module `src-tauri/src/ollama/` (client + attribution + types) mirrors
the existing `batch/`/`folder/` module style and keeps the HTTP concern isolated. A new frontend
`ProgressOverlay` + `progress.svelte.ts` is the single reusable surface (Constitution V); the two settings
categories are custom section components like `ShortcutsPage`. Everything else is in-place edits to existing
files. Default prompts, the offered-model list, and default run parameters are deferred — only their holding
structures are created.

## Complexity Tracking

> No constitution violations — table intentionally empty. (The three new Rust crates are justified under
> Principle X in research.md Decision 2.)
