# Phase 0 Research: SQLite Intermediate Metadata Store

All spec clarifications were resolved in the clarify session (2026-06-29); this document records
the resulting technical decisions and the alternatives weighed.

## 1. Storage engine — SQLite via `rusqlite` (`bundled`)

- **Decision**: Use `rusqlite = { version = "0.x", features = ["bundled"] }`. The `bundled`
  feature compiles the SQLite C amalgamation into the binary (via the `cc` build toolchain), so
  there is no system `libsqlite3` dependency at runtime. One database file `metadata.db` lives in
  the Tauri **app-data dir**, opened in **WAL** journal mode with `synchronous=NORMAL`.
- **Rationale**: Maintainer mandate (clarify). SQLite gives a transactional, single-file,
  externally-inspectable store; `rusqlite` is the de-facto Rust binding; `bundled` removes
  per-OS library setup and keeps the DB self-contained.
- **Constitution note**: This links C code → an explicit, approved exception to Principle I,
  recorded in the plan's Complexity Tracking. Scoped to the `store` module; no FFI in feature code.
- **Alternatives considered**:
  - *redb / sled* (pure Rust): honors Principle I but is not SQLite (different on-disk format, not
    inspectable with SQLite tooling) — rejected per maintainer's explicit SQLite requirement.
  - *Turso/Limbo* (pure-Rust SQLite-compatible): preserves the SQLite format but is too immature
    for the metadata of record — rejected.

## 2. Connection management & threading

- **Decision**: A single `rusqlite::Connection` wrapped in `Mutex`, held in a Tauri-managed
  `DbState`. All store calls run inside `tokio::task::spawn_blocking` (rusqlite is blocking).
  Initialize the schema once on `tauri::Builder::setup` (`CREATE TABLE IF NOT EXISTS`, set WAL).
- **Rationale**: Single-user desktop app with low write concurrency. A mutex-guarded connection
  is the simplest correct model and avoids a pool dependency. `spawn_blocking` keeps the async
  runtime responsive (Principle VIII — heavy/blocking work off the UI/async threads).
- **Alternatives**: `r2d2`/connection pool (unneeded complexity for one writer); connection per
  call (re-open cost, and WAL benefits from a long-lived connection) — rejected.

## 3. Fingerprint & change detection

- **Decision**: Fingerprint = `(size: u64, mtime: i64, hash: u64)` where `hash` is the **xxh3-64**
  of the **whole file**, via `xxhash-rust = { features = ["xxh3"] }`, streamed in fixed chunks
  (e.g. 64 KiB) so large files are not fully buffered. A record matches the file only when **all
  three** values match (clarify Q2 — no size+mtime short-circuit; the hash is always computed).
- **Rationale**: `xxhash-rust` is pure Rust (Principle I-compliant) and multi-GB/s, so hashing
  the whole file on open stays within the 200 ms budget (SC-001) for typical photos. Requiring all
  three to match is the maintainer's explicit rule; it makes any out-of-band touch enter the
  conflict branch deterministically.
- **Storage form**: `mtime` as Unix nanoseconds (`i64`) from `std::fs::metadata().modified()`;
  `hash` as `INTEGER` (i64 reinterpretation of the u64) or TEXT hex — `INTEGER` chosen for compact
  comparison.
- **Alternatives**: size+mtime short-circuit (skip hash when cheap fields match) — rejected by
  clarify; cryptographic hash (blake3) — unnecessary, slower, security not required.

## 4. Read-flow resolution & the conflict prompt (two-step IPC)

- **Decision**: A Tauri command cannot block on a UI dialog, so the read-flow is split:
  1. `open_metadata(path)` computes the fingerprint and applies `docs/SQLite.puml`. It returns a
     **resolution**: either `{ kind: "resolved", metadata, syncState }` (no record → read file +
     insert; all-three match → load store; mismatch **with** app-only changes → store wins) **or**
     `{ kind: "conflict", store, file }` (mismatch **without** app-only changes → external edit).
  2. On `conflict`, the frontend shows a prompt and calls `apply_metadata_source(path, source)`
     with `"store"` or `"file"` to finalize (store → refresh fingerprint, keep store; file → read
     file, overwrite store), returning the resolved metadata.
- **Rationale**: Keeps commands non-blocking (Principle IX) and the decision logic in Rust
  (Principle VIII). The two-step shape cleanly maps the diagram's single "ask the user" node.
- **Batch**: `open_metadata` is called per opened/selected photo. When several photos return
  `conflict`, the frontend collects them and offers **one** apply-to-all choice (FR-020), then
  calls `apply_metadata_source` for each — no per-file prompt storm.
- **Alternatives**: a single blocking command that emits an event and awaits a response — more
  fragile (correlating requests/responses) than an explicit two-call contract; rejected.

## 5. Immediate app-only persistence (manual edits + attribution)

- **Decision**: The working fields are the live copy of the store record. A debounced
  `store_metadata(path, fields)` upserts them as **app-only** (`syncState = appOnly`) on every
  change, without touching the file (clarify Q1, FR-013). Single Ollama attribution fills the
  fields → the same debounced upsert persists them. **Batch** attribution has no form, so
  `ollama/attribute.rs` persists each result directly to the store as app-only (replacing today's
  write-to-file in `attribute_and_save`).
- **Rationale**: One persistence mechanism for all edits; matches the maintainer's "any value
  typed into a field is saved to the database at once". Debounce (reuse the autosave-delay idea)
  avoids a write per keystroke while keeping the store authoritative.
- **Integration note**: This **changes existing 007 batch-attribution behavior** (it currently
  attributes **and saves to file**). Under this feature batch attribution writes to the store only;
  the user commits to files via Save / batch Save. Flagged for `/speckit-tasks`.

## 6. Save & Cancel (file side)

- **Decision**: `save_metadata` / `save_metadata_batch` keep writing files via the existing
  `batch::save_one`, then update the store record: refresh fingerprint from the freshly-written
  file and set `syncState = synced`. On rename, the store row key moves from the old path to the
  new final path. **Cancel** = `revert_to_file(path)`: read file metadata, overwrite the store
  record to mirror it, set `synced`, and return the metadata for the frontend to reload.
- **Rationale**: Reuses the proven write path; the store update is the only addition. Cancel gives
  the diagram's "accept from photo" outcome on demand.
- **Edge — `releaseFilename`**: today the file pipeline neither reads nor writes it
  (`read_metadata` returns `""`, `save_one` ignores it). The **store** can persist it, so the store
  becomes its source of truth; a conflict resolved to "file" or a Cancel will clear it (the file
  has none). Documented in data-model; acceptable for this feature.

## 7. New file status "saved in app"

- **Decision**: Add status id `app` (label EN "in app", RU "в приложении"), shown when the open
  record's `syncState = appOnly`. Single-mode precedence: `none` (no file) → `edit` (in-memory,
  pre-persist, transient) → `app` (store has unsaved-to-file changes) → `open` (synced). Existing
  `none/open/edit/batch` keep their meaning (FR-015); reuse the `status-dot`/`status-label`
  pattern with a new modifier color from theme tokens.
- **Rationale**: Minimal, additive change to the existing status component; one new i18n key trio.

## 8. Graceful degradation

- **Decision**: Wrap store access so any failure (cannot open DB, query error) is logged and the
  command falls back to today's direct file read/write (FR-021). `open_metadata` falls back to a
  plain file read with `syncState = synced`; `store_metadata` becomes a no-op-with-warning; Save
  still writes the file.
- **Rationale**: The store is an accelerator/safety-net, never a single point of failure for core
  editing.

## 9. No automatic cleanup

- **Decision**: Records for missing/deleted files are retained; no eviction. A manual "clean up
  database" action is deferred to a later feature (clarify Q4) — not built here.
