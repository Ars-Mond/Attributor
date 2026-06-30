# Implementation Plan: SQLite Intermediate Metadata Store

**Branch**: `008-sqlite-metadata-store` | **Date**: 2026-06-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/008-sqlite-metadata-store/spec.md`

## Summary

Insert a local SQLite database as an intermediate metadata layer between the editor and the
photo files. Every photo is keyed by its path and carries a fingerprint (`size + mtime +
full-file xxHash`). Opening a photo resolves metadata store-first (per `docs/SQLite.puml`):
fingerprint match → load from store; mismatch with app-only changes → store wins; mismatch
without app-only changes → ask the user (store vs file). Manual edits and Ollama attribution
persist to the store immediately (app-only, file untouched), surfaced by a new "saved in app"
file status. **Save** writes the store's metadata into the file and refreshes the fingerprint;
a new **Cancel** button (between Ollama and Save) reverts the working fields and the store
record back to the file. The store is `rusqlite` with the `bundled` feature — an explicit,
maintainer-approved exception to Constitution Principle I (Pure Rust), justified below.

## Technical Context

**Language/Version**: Rust (edition 2021, backend) + TypeScript / Svelte 5 runes (frontend)

**Primary Dependencies**: Tauri 2; **new**: `rusqlite` (feature `bundled`) for the store,
`xxhash-rust` (feature `xxh3`) for the fast full-file hash; existing `little_exif` (file
metadata), `serde`/`serde_json`, `tokio`, `rayon`, `tauri-plugin-log`.

**Storage**: One SQLite database file (`metadata.db`) in the Tauri app-data dir (WAL mode);
photo files (XMP/EXIF/IPTC) remain the on-disk source of truth that Save writes to.

**Testing**: `cargo test` (store CRUD, fingerprint, read-flow resolution, fallback) +
`npx svelte-check` for the frontend.

**Target Platform**: Windows / Linux / macOS desktop (Tauri 2 webview).

**Project Type**: Desktop application (Tauri 2 backend + SvelteKit/Svelte 5 frontend).

**Performance Goals**: Re-open of an up-to-date photo < 200 ms incl. the change check
(SC-001); full-file xxh3 hashing is multi-GB/s, streamed in chunks.

**Constraints**: Offline, local, single-user. No mass hashing on folder scan — the store
read-flow runs only on photo open / batch selection (spec Assumptions). Store failures must
degrade gracefully to today's direct file read/write (FR-021).

**Scale/Scope**: Libraries up to tens of thousands of photos; metadata rows are tiny (text).
No automatic record cleanup (manual, deferred).

## Constitution Check

*GATE: evaluated against `.specify/memory/constitution.md` v1.1.0.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Pure Rust Backend | **VIOLATION (approved)** | `rusqlite` + `bundled` compiles SQLite **C** into the app. Maintainer-approved exception (clarify session 2026-06-29). See Complexity Tracking. `xxhash-rust` is pure Rust — compliant. |
| II. Modern Svelte 5 (Runes) | PASS | New status + Cancel button + conflict dialog use `$state`/`$derived`/`$effect` only. |
| III. Themed SCSS Tokens | PASS | New status dot/label and Cancel button styled from existing tokens. |
| IV. Cross-Platform Parity | PASS (note) | `bundled` SQLite builds identically on all three OSes via the `cc` toolchain already required for builds; runtime behavior is uniform. No `cfg(target_os)` gating. |
| V. Reuse UI Primitives | PASS | Conflict prompt reuses `ConfirmDialog`; footer reuses existing `.btn-ghost`/`.btn-primary`. |
| VI. Mandatory Logging | PASS | Every store op and every store error logged via `log` (FR-022); fallback paths logged. |
| VII. Phase-Based Commits | PASS | One commit per Spec Kit phase. |
| VIII. Rust Performance First | PASS | Hashing + all DB work in Rust; batch crosses IPC once; no per-file IPC in loops; no hashing during folder scan. |
| IX. Typed Tauri IPC | PASS | New commands return `Result<T, String>`, `#[serde(rename_all = "camelCase")]`, never panic across IPC. |
| X. Fixed Stack | PASS (justified) | Two new deps justified here: `rusqlite/bundled` (the store; exception above) and `xxhash-rust` (the spec-mandated fast hash, pure Rust). |
| XI. Code Style | PASS | English comments/identifiers; no inner brace spaces in TS; no padding alignment. |

**Gate result**: PASS with one documented, maintainer-approved violation (Principle I) recorded
in Complexity Tracking. No unjustified violations.

## Project Structure

### Documentation (this feature)

```text
specs/008-sqlite-metadata-store/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── ipc-commands.md  # Phase 1 output — new/changed Tauri command contracts
└── checklists/
    └── requirements.md  # from /speckit-specify
```

### Source Code (repository root)

```text
src-tauri/
├── Cargo.toml                  # EDIT — add rusqlite (bundled) + xxhash-rust (xxh3)
└── src/
    ├── lib.rs                  # EDIT — manage DbState, init DB on setup, register new commands
    ├── store/                  # NEW module — the intermediate metadata store
    │   ├── mod.rs              #   DbState, open/init (WAL + schema), public store API + commands
    │   ├── schema.rs           #   table definition / migration (CREATE TABLE IF NOT EXISTS)
    │   ├── record.rs           #   StoreRecord, SyncState, IPC payload types (camelCase)
    │   └── fingerprint.rs      #   size + mtime + streamed xxh3 of the whole file
    ├── photo/                  # (read/write unchanged; store calls these for the file side)
    ├── batch/mod.rs            # EDIT — after each file write, refresh the store record
    └── ollama/attribute.rs     # EDIT — batch attribution persists to the store (app-only), not the file

src/ (frontend)
├── lib/
│   ├── store/                  # NEW — typed invoke wrappers for the new commands
│   │   └── metadata.ts         #   openMetadata, applyMetadataSource, storeMetadata, revertToFile
│   ├── panel/MetadataPanel.svelte   # EDIT — store-first load, immediate persist, new status, Cancel button, conflict dialog
│   ├── types.ts                # EDIT — resolution/conflict/status types
│   └── i18n/{en,ru,types}.ts   # EDIT — "saved in app" status + Cancel + conflict-dialog strings
```

**Structure Decision**: Single Tauri desktop project (existing layout). The store is a new,
self-contained Rust module (`src-tauri/src/store/`) that wraps the existing `photo`
read/write as the file side and is fronted by a thin typed IPC layer (`src/lib/store/`). No
new project or process is introduced.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| **Principle I (Pure Rust): `rusqlite` with `bundled` links SQLite C code** | The maintainer explicitly chose SQLite for the intermediate store (familiar, inspectable, transactional) and `rusqlite` + `bundled` to compile it in without a system dependency. Confirmed in the clarify session (2026-06-29). | **Pure-Rust embedded DB (redb/sled)**: rejected — not SQLite (no SQLite file/format, can't inspect with SQLite tooling), changes the requested storage model. **Pure-Rust SQLite-compatible (Turso/Limbo)**: rejected — too immature for the metadata of record. The exception is scoped to this single C dependency, behind the `store` module, and introduces no FFI in feature code. |
