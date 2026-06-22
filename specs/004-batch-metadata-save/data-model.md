# Data Model: Batch Metadata Save & Unified Event Contract

Conceptual entities and their concrete Rust/TS shapes. All IPC/channel types use
`#[serde(rename_all = "camelCase")]` (§IX). Payload types for events/channel live in the
contract module `src-tauri/src/events.rs`; the per-item input reuses the existing `SaveRequest`.

## SaveRequest (reused, input — one batch item)

The existing `src-tauri/src/types.rs::SaveRequest`, unchanged. The batch command takes
`Vec<SaveRequest>`; each item is a fully-resolved target (the frontend already merged shared
fields with that file's own keywords — FR-006).

| Field | Type | Notes |
|-------|------|-------|
| `filepath` | `String` | Current full path. |
| `filename` | `String` | Desired stem (no ext/dir). In batch this equals the current stem → no rename. |
| `title` | `String` | Resolved final value. |
| `description` | `String` | Resolved final value. |
| `keywords` | `Vec<String>` | Resolved final list (merge done on the frontend). |
| `categories` | `String` | Resolved final value. |
| `release_filename` | `String` | Carried through unchanged. |

Validation: empty managed fields mean "clear that field" (existing single-file semantics, FR-010).
Duplicate `filepath` values in one batch are tolerated; a collision surfaces as a per-item failure.

## ItemStatus (output — one file's outcome)

The definitive outcome for one batch item (FR-004/FR-018). Serde-tagged enum, camelCase.

| Variant | Data | Meaning |
|---------|------|---------|
| `Ok` | `{ path: String }` | Saved successfully; `path` is the final file path. |
| `Failed` | `{ error: String }` | Write failed; human-readable reason (logged too, §VI). |
| `Cancelled` | — | The file had not started when cancellation was observed. |

Every request item produces exactly one `ItemStatus` (SC-002). The command returns
`Vec<ItemStatus>` in input order as the authoritative final result.

## BatchProgress (channel message — incremental)

Streamed over `tauri::ipc::Channel<BatchProgress>`, one per file as it completes (FR-005).

| Field | Type | Notes |
|-------|------|-------|
| `index` | `usize` | Index into the input `Vec<SaveRequest>`; the frontend keys UI by this (order-independent). |
| `status` | `ItemStatus` | The same outcome enum as above. |

Progress derivation (frontend): `total` comes from the request length (or an initial `Started`
message if added); `done = number of BatchProgress messages received`. Do not rely on arrival
order (R1 ordering caveat).

## BatchState (managed runtime state — cancellation)

Tauri-managed singleton holding the current batch's cancel flag, mirroring `FolderState`.

| Field | Type | Notes |
|-------|------|-------|
| `cancel` | `Mutex<Option<Arc<AtomicBool>>>` | The in-flight batch's flag. `save_metadata_batch` installs a fresh flag (setting any previous one to `true`, the `swap_run` idiom); `cancel_batch` stores `true`. |

State transition: idle → (`save_metadata_batch` installs fresh `false` flag) running → workers read
flag per item; `cancel_batch` sets `true` → not-yet-started items resolve `Cancelled` → command
returns, channel closes.

## Event contract payloads (broadcast events — migrated)

Centralized in `events.rs` (names + types); emission stays in the owning domain (FR-013).

| Event name (const) | Payload struct | Emitted by | Replaces |
|--------------------|----------------|------------|----------|
| `FOLDER_CHANGED` = `"folder-changed"` | `FolderChanged { path: String }` | `folder/watch.rs` | bare `String` payload (now typed) |
| `THUMBNAIL_READY` = `"thumbnail-ready"` | `ThumbnailReady { path: String }` | `folder/pipeline.rs` | struct moved out of `pipeline.rs`, unchanged shape |

These remain **global broadcast events** (ambient signals). Batch progress is the per-operation
signal and uses the Channel instead (FR-014).

## Generated TypeScript contract

`src/lib/generated/events.d.ts` (ts-rs output, checked in): the TS interfaces for
`ThumbnailReady`, `FolderChanged`, `BatchProgress`, `ItemStatus`. `src/lib/events.ts` provides the
event-name catalog and a typed `listenEvent` wrapper, plus the `BatchMsg`/progress consumption
types built on the generated interfaces. A `cargo test` drift guard keeps generated ↔ Rust in sync
(FR-016).

## Relationships

```text
Vec<SaveRequest>  ──save_metadata_batch──▶  rayon par_iter ──save_one──▶  Vec<ItemStatus> (return)
                                                  │ per item
                                                  └─ Channel<BatchProgress> ──▶ frontend onmessage
cancel_batch ──▶ BatchState.cancel(AtomicBool=true) ──▶ not-yet-started items ⇒ ItemStatus::Cancelled

events.rs (contract: names + payload types) ──▶ folder/watch.rs, folder/pipeline.rs (emit)
                                              └─▶ ts-rs ──▶ src/lib/generated/events.d.ts (frontend)
```
