# Implementation Plan: Batch Metadata Save & Unified Event Contract

**Branch**: `004-batch-metadata-save` | **Date**: 2026-06-23 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/004-batch-metadata-save/spec.md`

## Summary

Replace the frontend's sequential per-file batch save loop with a single async backend
command that writes the whole selection concurrently with `rayon`, streams per-file results
and incremental progress back over a `tauri::ipc::Channel<T>`, and is cancellable via a shared
flag flipped by a separate command. In parallel, formalize the backend→frontend **event
contract**: a single Rust module holds event-name constants and `serde`-camelCase payload
structs (the source of truth), domain modules keep emitting in place, and `ts-rs` generates a
checked-in TypeScript type file guarded by a drift test — so a name/payload change becomes a
build/type error instead of silent runtime drift. Existing broadcasts (`folder-changed`,
`thumbnail-ready`) migrate onto the contract unchanged; per-batch progress flows over the
channel (not a broadcast), applying "mechanism by signal nature".

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript + Svelte 5 (runes) frontend.

**Primary Dependencies**: Tauri 2 (`tauri::ipc::Channel`, managed `State`, `Emitter`); **new**:
`rayon` (batch parallelism, §VIII) and `ts-rs` (dev-only, event-type generation); existing
`serde`, `little_exif` (fork), `image`, `notify`, `tokio` (sync + blocking pool), `log`.

**Storage**: Photo files on disk; metadata written in place via `little_exif`. No database.

**Testing**: `cargo test` (unit + integration under `src-tauri/tests/`); `npx svelte-check`.
New: batch write/cancel integration tests and an events-contract drift guard test.

**Target Platform**: Windows, Linux, macOS desktop (Tauri 2 + SvelteKit).

**Project Type**: Desktop application (Rust backend + SvelteKit/Svelte 5 frontend).

**Performance Goals**: A 50-photo batch saves ≥3× faster than the current one-by-one flow on a
4-core machine (SC-001); incremental progress visible before completion (SC-004); UI responsive
throughout (SC-008).

**Constraints**: Pure-Rust dependency graph (§I); cross the IPC boundary once per batch, not per
file (§VIII); never panic across IPC, `Result<T, String>`, camelCase payloads (§IX).

**Scale/Scope**: Tens–hundreds of photos per batch typical; bounded behavior for thousands.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Pure Rust Backend | PASS | `rayon` and `ts-rs` are pure Rust, no FFI/native libs. |
| II. Modern Svelte 5 (Runes) | PASS | Batch progress/cancel UI uses `$state`/`$derived`; no legacy stores. |
| III. Themed SCSS Tokens | PASS | New progress/cancel UI sources colors/spacing from tokens. |
| IV. Cross-Platform Parity | PASS | `rayon`/Channel are cross-platform; the contract guard test normalizes CRLF/LF. |
| V. Reuse UI Primitives | PASS | Reuse existing button/dialog primitives for the cancel control and progress display. |
| VI. Mandatory Logging | PASS | Per-file write failures and `Channel::send` failures are logged (English). |
| VII. Phase-Based Commits | PASS | One commit per Spec Kit phase via the git hook. |
| VIII. Rust Performance First | PASS | Batch work runs in Rust via `rayon`; one command + one channel, no per-file IPC. This feature is the canonical fulfillment of §VIII. |
| IX. Typed Tauri IPC | PASS | Commands return `Result<T, String>`, never panic; all payloads `#[serde(rename_all = "camelCase")]`. |
| X. Fixed Stack | PASS (with justification) | Two new deps — see Complexity Tracking. |
| XI. Code Style | PASS | English comments; no inner brace spaces / alignment padding in TS. |

**Gate result**: PASS (no unjustified violations). New dependencies justified below.

## Project Structure

### Documentation (this feature)

```text
specs/004-batch-metadata-save/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── ipc-commands.md      # save_metadata_batch / cancel_batch
│   ├── channel-protocol.md  # BatchProgress / ItemStatus streamed over Channel
│   └── events.md            # unified event contract (names + payloads)
└── tasks.md             # Phase 2 output (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
src-tauri/src/
├── lib.rs               # register save_metadata_batch + cancel_batch; .manage(BatchState);
│                        #   single save_metadata delegates to batch::save_one
├── events.rs            # NEW — event CONTRACT: name constants + payload structs
│                        #   (ThumbnailReady, FolderChanged, BatchProgress, ItemStatus),
│                        #   ts-rs export gated to cfg(test)
├── batch/
│   └── mod.rs           # NEW — BatchState (cancel flag), save_metadata_batch logic,
│                        #   cancel_batch, save_one (extracted rename+write)
├── types.rs             # SaveRequest (reused as the per-item type), ReadResult
├── photo/               # write.rs/read.rs unchanged in behavior; write reused via save_one
└── folder/
    ├── pipeline.rs      # emit thumbnail-ready via events::ThumbnailReady (moved out of here)
    └── watch.rs         # emit folder-changed via events::FolderChanged

src/lib/
├── generated/
│   └── events.d.ts      # NEW — ts-rs generated payload types (checked in)
├── events.ts            # NEW — EVENT name catalog + typed listenEvent wrapper + BatchMsg type
├── panel/
│   ├── MetadataPanel.svelte   # replace handleBatchSave loop with one batched invoke + Channel
│   └── filesPanelStore.svelte.ts
└── reusable/            # reuse existing primitives for progress/cancel UI

src-tauri/tests/
├── batch_test.rs            # NEW — concurrent save correctness, best-effort failure, cancellation
└── events_contract_test.rs  # NEW — ts-rs regen matches the checked-in events.d.ts (drift guard)
```

**Structure Decision**: Keep the established modular backend (`folder/`, `photo/`). Add a
`batch/` module owning the batch command, its cancel state, and the shared `save_one`; add a
single `events.rs` contract module that centralizes event names + payload TYPES only (emission
stays in the owning domains, FR-013). Frontend gains a `generated/events.d.ts` (ts-rs output)
and an `events.ts` catalog/wrapper; the batch UI changes are confined to `MetadataPanel.svelte`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New dependency: `rayon` | §VIII explicitly mandates `rayon` for batch photo processing; it provides the concurrent `par_iter` over the file list with ordered result collection. | Frontend `Promise.all` fan-out violates §VIII and floods IPC; a hand-rolled `std::thread` pool duplicates `rayon` for no benefit and contradicts the constitution's named tool. |
| New dependency: `ts-rs` (dev-only) | FR-012/FR-016 require a single event contract where a payload/name change is a build/type error. `ts-rs` derives TS types from the Rust source structs (real linkage), pure Rust, zero shipped-binary weight. | Manual mirror + text-equality test has no derive-level linkage and is brittle; `tauri-specta` pulls a much heavier tree and pushes replacing the existing invoke/listen surface — more change and risk than the requirement needs. |

> Both additions are pure Rust (§I-compliant). `ts-rs` is confined to `dev-dependencies` and
> gated to `cfg(test)`, so normal/release builds are unaffected.
