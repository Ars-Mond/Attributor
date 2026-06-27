# Quickstart & Validation: Ollama Vision Auto-Attribution

**Feature**: 007-ollama-attribution | **Date**: 2026-06-26

Runnable validation scenarios. API details in [contracts/](./contracts/), entities in
[data-model.md](./data-model.md).

## Prerequisites

- Ollama installed and running locally, with at least one **vision** model pulled (the offered list /
  default prompts are supplied later; for testing pull any vision model, e.g. via the settings Download
  action or `ollama pull <vision-model>`).
- New crates added to `src-tauri/Cargo.toml`: `reqwest` (rustls-tls/json/stream), `base64`, `futures-util`.
- New commands registered in `src-tauri/src/lib.rs`; new events regenerated
  (`UPDATE_EVENTS_CONTRACT=1 cargo test events_contract`).

## Build / run / check

```sh
npm run check                                   # svelte-check (i18n completeness incl. new keys)
cargo test --manifest-path src-tauri/Cargo.toml # backend + events_contract drift guard
npm run tauri dev                               # run for manual scenarios
```

## Scenario 1 — Configure & detect Ollama (US2 / FR-001..FR-007)

1. Settings → **Ollama**. Click **Check** with the daemon stopped → status shows not reachable.
2. Start Ollama, Check again → reachable + version shown.
3. Pick an offered model, **Download** → the progress overlay shows pull progress; on success the model
   appears in the **installed** dropdown.
4. Select an installed model → it persists across restart (reopen settings, still selected).
5. Open the **JSON response-format** field → its description warns it is debug-only.

## Scenario 2 — Attribute a single photo (US1 / FR-008..FR-012)

1. Open a photo. With Ollama reachable and a model selected, the **Attribute via Ollama** button is enabled.
2. Click it → after inference the **title** and **description** are overwritten, **categories** filled, and
   the returned **keywords** are appended to any existing ones (no duplicates). Nothing is saved yet.
3. Review and **Save** → the file is written. (The editorial/mature/illustration flags are ignored this
   feature.)

## Scenario 3 — Attribute disabled when unavailable (FR-009, SC-004)

1. Stop Ollama (or clear the active model). The **Attribute** button is greyed out; hovering shows the
   "Ollama is not available" tooltip; clicking does nothing / sends no request.

## Scenario 4 — Batch attribute and always save (US3 / FR-013..FR-015)

1. Select several photos → **Attribute via Ollama** (batch). The progress overlay shows per-item progress
   with a **Cancel** button.
2. Each photo is attributed and **saved automatically** (no manual save). A failing item is recorded and
   surfaced; the batch continues; other files are not corrupted.
3. Click **Cancel** mid-batch → processing stops promptly; files already saved remain; the rest are
   untouched (SC-006).

## Scenario 5 — Reusable overlay & batch-save freeze (US4 / FR-016..FR-020)

1. During any long operation (pull, batch attribute, batch save) the overlay sits **above every** dialog/
   panel/menu and offers cancel.
2. Trigger a normal **batch save** → it now runs through the overlay and **freezes** the UI until done.

## Scenario 6 — Manage model profiles (US5 / FR-021..FR-024)

1. Settings → **Ollama Models** → **Create** → the popup opens; set model id, run parameters, and a prompt →
   **Save**. The profile appears in the list and persists across restart.
2. **Edit** it (change the prompt) → **Save**; **Delete** another → removed.
3. Run an attribution with the active model → the matching profile's prompt and parameters drive the request
   (verify via logs / model behaviour).

## Done / acceptance

- `npm run check` and `cargo test` pass (including `events_contract`).
- Scenarios 1–6 behave as described; malformed responses never corrupt metadata (SC-005); cancellation is
  prompt and safe (SC-006).
