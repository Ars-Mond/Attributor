# Quickstart & Validation: Batch Metadata Save & Unified Event Contract

How to build, regenerate the event types, and validate the feature end-to-end. Details live in
[contracts/](./contracts/) and [data-model.md](./data-model.md).

## Prerequisites

- Rust toolchain + Node deps installed (`npm install`).
- New backend deps present after implementation: `rayon` (`[dependencies]`) and `ts-rs`
  (`[dev-dependencies]`) in `src-tauri/Cargo.toml`.

## Build & checks

```bash
# Backend
cargo build --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml

# Frontend (Constitution workflow gate)
npx svelte-check --tsconfig ./tsconfig.json
```

## Regenerate the event types (ts-rs)

The generated TS file is committed; regenerate it whenever an event/channel payload changes:

```bash
# ts(export) runs under cfg(test); running the contract test (re)writes the generated types
cargo test --manifest-path src-tauri/Cargo.toml events_contract
# then review/commit src/lib/generated/events.d.ts
```

If `events_contract_test` fails, the committed `src/lib/generated/events.d.ts` is stale — rerun the
command above and commit the regenerated file.

## Validation scenarios

### S1 — Concurrent batch save is correct and faster (US1, SC-001/SC-006)

1. Open a folder, multi-select ≥50 photos, edit a shared title + add a keyword, Save.
2. Expect: all files written with the new metadata; per-file result identical to saving one-by-one;
   wall-clock ≥3× faster than the old sequential flow on a 4-core machine.
3. Backend test: `batch_test.rs` writes N generated images concurrently and asserts each file's
   metadata reads back correctly.

### S2 — Per-file status + incremental progress, best-effort (US2, SC-002/SC-003/SC-004)

1. Build a batch mixing writable photos with one read-only/locked file; Save.
2. Expect: progress advances during the run (not a single end jump); the bad file reports `failed`
   with a reason; every other file is saved; all N items have a definitive outcome.
3. Backend test: `batch_test.rs` includes an unwritable/corrupt item and asserts the others succeed
   and the failure is reported (no panic).

### S3 — Cancellation (US2, FR-017/FR-018, SC-009)

1. Start a large batch, click Cancel partway.
2. Expect: not-yet-started files reported `cancelled`; an in-flight write may complete; the batch
   stops promptly rather than running everything; every item still has a definitive outcome.
3. Backend test: set the cancel flag mid-run and assert a mix of `ok`/`cancelled` with no item left
   ambiguous.

### S4 — No watcher churn from a metadata-only batch (FR-008, SC-005)

1. With a folder open (thumbnails generated), run a batch metadata save.
2. Expect: 0 thumbnail regenerations (existing thumbnails stay valid); the folder tree is not
   disruptively reloaded for the unchanged images.

### S5 — Event contract drift guard (US3, FR-016)

1. Change a payload field in `events.rs` without regenerating.
2. Expect: `cargo test events_contract` FAILS (drift detected). After regeneration + commit it
   passes; a stale frontend type is caught by `svelte-check`.
3. Sanity: `folder-changed` and `thumbnail-ready` still drive the file tree exactly as before
   (FR-015).

## UI responsiveness (SC-008)

During S1/S3 the window stays responsive (input usable, no frozen frames) because the batch runs in
`spawn_blocking` + `rayon`, off the async runtime and off the webview thread.
