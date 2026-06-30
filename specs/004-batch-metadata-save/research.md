# Research: Batch Metadata Save & Unified Event Contract

Phase 0 decisions. Each entry: Decision / Rationale / Alternatives considered.

## R1 — Per-batch progress transport: `tauri::ipc::Channel<T>`

**Decision**: Stream progress and per-file results over a single `tauri::ipc::Channel<BatchProgress>`
parameter on the async `save_metadata_batch` command. The frontend constructs
`new Channel<BatchMsg>()` (`@tauri-apps/api/core`), sets `onmessage`, and passes it as the
camelCase invoke arg (`onProgress`). The channel handle is `Clone + Send + Sync`, so it is cloned
into the `rayon` workers and `send()` is called concurrently as each file completes.

**Rationale**: `Channel<T>` is purpose-built for backend→frontend streaming scoped to ONE command
call: concurrent batches never collide, no global event name is needed, and the channel is torn
down automatically when the call's promise resolves. `T` only needs `Serialize` (blanket
`IpcResponse`). This is the concrete application of spec FR-014 ("mechanism by signal nature":
per-operation progress → channel; ambient broadcasts → global events).

**Ordering caveat**: With many `rayon` senders, message arrival order is not guaranteed. The
protocol therefore emits `Started{total}` once, then one `FileResult{index, …}` per file; the
frontend keys results by `index` and derives `done = count(received FileResult)` rather than
trusting any in-message running counter.

**Alternatives considered**:
- Global `app.emit("batch-progress", …)` — broadcasts to all windows/listeners, needs a string
  name + manual unlisten, cannot separate two concurrent batches. Rejected.
- Single aggregate `Vec<FileResult>` return (no streaming) — no incremental progress, no live
  cancel feedback (violates FR-005). Rejected.
- `tokio::mpsc` + a forwarding task re-emitting events — reinvents `Channel` with extra hops.
  Rejected.

## R2 — Concurrency + cancellation: `rayon` inside `spawn_blocking`, shared `AtomicBool`

**Decision**: Add `rayon`. The command resolves a fresh `Arc<AtomicBool>` cancel flag into
managed `BatchState` (swap-out-old idiom, mirroring `folder::swap_run`), then runs
`tokio::task::spawn_blocking(move || items.into_par_iter().enumerate().map(...).collect())`.
Each closure checks `cancel.load(Relaxed)` at the start: if set → `ItemStatus::Cancelled` (no disk
touch); else it calls the shared `save_one(item)` and maps the result. It `send()`s a
`BatchProgress{index, status}` per item and returns the ordered `Vec<ItemStatus>` as the
authoritative result. A separate sync command `cancel_batch(state)` flips the flag.

**Rationale**:
- `spawn_blocking` is required: `rayon::par_iter` blocks the calling thread for the whole job and
  `save_one` does sync file I/O + EXIF encode; running it directly in the async command would
  starve a tokio worker and could even delay the `cancel_batch` command. The existing
  `get_thumbnails` command already uses `spawn_blocking` — same precedent.
- Per-item start-check cancellation gives exactly the spec semantics: not-yet-started files become
  `Cancelled`; an in-flight write finishes (FR-017/FR-018).
- The cancel flag lives in `State`, not the channel, because `Channel` is backend→frontend only;
  the frontend signals cancel by invoking `cancel_batch`. This is the same `Arc<AtomicBool>`
  pattern the thumbnail pipeline already uses (`folder/pipeline.rs`).
- The global `rayon` pool sizes to ~all cores (matches §VIII and clarify Q3); no custom pool today
  (thumbnails use `std::thread`, so no `rayon` contention).

**Alternatives considered**:
- `par_iter` directly in the async fn (no `spawn_blocking`) — blocks the runtime. Rejected.
- Per-file `spawn_blocking` + `FuturesUnordered` — ignores §VIII's mandate for `rayon`; loses
  ordered collect. Rejected.
- Custom per-batch `ThreadPool` — extra construction cost/complexity for no current benefit; keep
  as a future option only if `rayon` contention ever appears. Rejected for now.
- Cancellation by closing the channel — impossible (channel is one-directional). Rejected.

## R3 — Typed event contract Rust↔TS: `ts-rs` (dev-dependency) + checked-in types + drift test

**Decision**: Create `src-tauri/src/events.rs` as the single contract: event-name constants
(`FOLDER_CHANGED`, `THUMBNAIL_READY`) plus payload structs (`ThumbnailReady`, `FolderChanged`)
and the batch channel types (`BatchProgress`, `ItemStatus`), all `#[serde(rename_all = "camelCase")]`.
Add `ts-rs` as a **dev-dependency**; derive/export are gated with `#[cfg_attr(test, derive(ts_rs::TS))]`
+ `#[cfg_attr(test, ts(export, export_to = "…/src/lib/generated/events.d.ts"))]`. The generated
`src/lib/generated/events.d.ts` is committed; a `cargo test` guard (`events_contract_test.rs`)
regenerates to a temp dir and asserts byte-equality (line-endings normalized). Domain modules
import the constants/structs and keep emitting in place (FR-013). Frontend adds `src/lib/events.ts`
(name catalog + a typed `listenEvent` wrapper over `@tauri-apps/api/event`) consuming the generated
types; `FilesPanel.svelte` switches to it. `folder-changed`'s payload is promoted from a bare
`String` to `FolderChanged{path}` so no untyped crossing remains (its only listener ignores the
payload today → safe).

**Rationale**: `ts-rs` is pure Rust (proc-macro, §I-clean), dev-only (zero shipped weight, the
narrowest §X justification), and gives real derive-level linkage: a Rust field/name change without
regen fails the guard test; a stale frontend type fails `svelte-check`. That is exactly the
FR-012/FR-016 tripwire on both sides, while leaving the existing invoke/listen mechanism untouched.

**Alternatives considered**:
- `tauri-specta` + `specta` — also pure Rust and types commands+events together, but a much heavier
  tree and steers toward replacing the whole IPC surface (more change/risk than needed). Rejected.
- Manual mirror + text-equality test — no derive linkage, brittle to formatting, a rename slips
  through unless someone also edits the expected string. Rejected.
- Status quo (duplicated string literals + hand-mirrored types) — drift surfaces only as a silent
  runtime no-op, exactly what FR-012/FR-016 forbid. Rejected.

**Notes / risks captured for the plan**:
- Gate `ts-rs` derive/export to `cfg(test)` so normal builds write nothing.
- Pin the `ts-rs` major version so a bump can't reformat the committed `.d.ts` and falsely fail.
- Normalize CRLF/LF in the guard test (§IV cross-platform).
- Keep scope to events + channel payloads; do NOT codegen all command DTOs (keeps §X narrow).
- Re-introducing `events.rs` is intentional — unlike the previously-removed dead `AppEvent`, this
  module has real consumers (the migrated broadcasts + the batch channel types).

## R4 — Reuse the existing single-file write path

**Decision**: Extract the rename-if-changed + `photo::write_metadata` body currently inline in
`lib.rs::save_metadata` into a shared `batch::save_one(item: SaveRequest) -> Result<String, String>`.
Both the single-file `save_metadata` command and each `rayon` closure call it. Reuse the existing
`SaveRequest` (`types.rs`) verbatim as the per-item type (`Vec<SaveRequest>` is the batch payload).

**Rationale**: Guarantees FR-010 (batch result identical to single-file save) by construction and
avoids duplicating the atomic create-new/copy/splice/delete rename logic. No behavior change to the
per-file write itself (the `little_exif` write-path micro-optimization remains out of scope per the
spec).

## R5 — Frontend resolves per-file metadata from already-loaded batch data (FR-006/FR-007)

**Decision**: `handleBatchSave` builds the `Vec<SaveRequest>` from data already loaded by
`loadBatchData` (cache each file's current keywords when the batch selection loads) instead of
re-reading each file at save time. The backend reads each file at most once (only to preserve
unrelated EXIF/IPTC tags inside `save_one`).

**Rationale**: Honors the clarify decision (frontend sends fully-resolved items) while eliminating
the redundant per-file re-read identified in the earlier batch perf analysis. Keyword-merge logic
stays on the frontend (trivial CPU, not "heavy work", so no §VIII conflict).
