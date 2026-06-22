# Implementation Plan: Photo Folder Handler

**Branch**: `003-photo-folder` | **Date**: 2026-06-22 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/003-photo-folder/spec.md`

## Summary

Consolidate all folder operations into a single backend `folder` module (PhotoFolder): open and
scan a folder + subfolders into the tree (fast, no inline thumbnail work), enumerate/search photo
paths, locate thumbnails, watch for changes, and generate thumbnails through a hand-rolled
producer–consumer thread pool (std threads + a channel, no new dependency). The scan fills each
photo node with its deterministic thumbnail paths immediately; a bounded worker pool then generates
missing thumbnails — visible folder level first, deeper subfolders after — emitting a per-photo
`thumbnail-ready` event so the UI updates each preview as it completes. Switching folders cancels
the previous run. Per-photo work (decode, metadata, single-photo thumbnail creation) is delegated
to the existing `photo` module — this module owns folders only (single responsibility). This
replaces the inline single-threaded generation from feature 002.

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript 5.6 / Svelte 5 frontend

**Primary Dependencies**: existing only — `tauri` (Emitter for events), `notify` (watcher), `tokio` (async commands), the `photo` module (`ensure_thumbnails`, `thumbnail_paths`). The pool uses `std::thread` + `std::sync::mpsc`. **No new dependency.**

**Storage**: thumbnail JPGs in `_thumbnail` folders on disk (as established in feature 002)

**Testing**: `cargo test` (scan, enumeration, `_thumbnail` exclusion, pipeline generates+reuses, cancellation, deterministic paths); `npx svelte-check`

**Target Platform**: Windows / Linux / macOS desktop (Tauri 2)

**Project Type**: Desktop application (Rust/Tauri backend + SvelteKit/Svelte 5 frontend)

**Performance Goals**: folder structure shown <1 s regardless of photo count; thumbnail generation ≥2× faster than sequential on 4 cores; UI responsive throughout; folder-switch cancellation <1 s

**Constraints**: producer–consumer over a bounded, hand-rolled thread pool (std threads + channel, no new dep); visible-level-first ordering; per-photo `thumbnail-ready` events (no polling); single responsibility (folder ops only; per-photo work delegated to `photo`)

**Scale/Scope**: Single-user desktop; folder trees up to thousands of photos

## Constitution Check

*GATE: evaluated against `.specify/memory/constitution.md` v1.0.0. Re-checked after design.*

| # | Principle | Status | Notes |
|---|-----------|--------|-------|
| I | Pure Rust Backend | PASS | `std::thread` + `std::sync::mpsc`; no FFI/native libs |
| II | Modern Svelte 5 (Runes) | PASS | Event listener + a reactive "ready" set in the runes store; `$state`/`$derived`/`$effect` only |
| III | Themed SCSS Tokens | PASS | Minimal styling (placeholder state); tokens only |
| IV | Cross-Platform Parity | PASS | `std` threads, `notify`, `std::path`; no `cfg(target_os)` gating |
| V | Reuse UI Primitives | PASS | Reuses `photo`, `FileNode`, `FileTree`, the watcher/event pattern; **consolidates** scattered folder logic rather than duplicating |
| VI | Mandatory Logging | PASS | Worker/scan/watch failures logged at the error site |
| VII | Phase-Based Commits | PASS | One commit per phase |
| VIII | Rust Performance First | PASS | Heavy batch (decode/resize/encode) parallelized in Rust off the UI thread; coarse per-photo events, no hot-loop IPC. Constitution v1.1.0 §VIII permits an equivalent thread pool for such workloads, so the hand-rolled producer–consumer pool is compliant |
| IX | Typed Tauri IPC | PASS | Commands keep `Result<T, String>`; the `thumbnail-ready` event payload is a `serde` type with `#[serde(rename_all = "camelCase")]` |
| X | Fixed Stack | PASS | No new dependency (standard library concurrency) |
| XI | Code Style | PASS | English identifiers/comments; no inner-brace spaces in TS; no field alignment |

**Result**: PASS — no violations (the thread-pool choice is permitted by constitution v1.1.0 §VIII).

## Project Structure

### Documentation (this feature)

```text
specs/003-photo-folder/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── ipc-commands.md  # open_folder / scan_folder / queries
│   ├── events.md        # thumbnail-ready event payload
│   └── folder-api.md    # Rust folder module API
└── checklists/
    └── requirements.md  # From /speckit-specify
```

### Source Code (repository root)

```text
src-tauri/src/
├── folder/                 # NEW module (replaces filetree.rs): owns all folder operations
│   ├── mod.rs              # PhotoFolder struct (the class) + FileNode + FolderState (managed); open/rescan/query methods
│   ├── scan.rs             # scan_dir → tree + deterministic thumb paths; excludes _thumbnail (no inline generation)
│   ├── pipeline.rs         # producer–consumer pool (std threads + mpsc), visible-first, cancellation, emits thumbnail-ready
│   └── watch.rs            # notify watcher (moved from filetree.rs)
├── filetree.rs             # REMOVED (content moved into folder/)
├── lib.rs                  # Commands delegate to folder; open_folder passes AppHandle to start pipeline + watcher
├── photo/
│   ├── mod.rs              # re-export thumbnail_paths
│   └── thumbnail.rs        # NEW pub `thumbnail_paths(&Path) -> Thumbnails` (paths only, no I/O); ensure_thumbnails reuses it

src-tauri/tests/
├── folder_test.rs          # NEW: scan tree, enumerate paths, _thumbnail excluded, pipeline generate+reuse, deterministic paths
└── (filetree exclusion test moves into folder/scan.rs cfg(test) or folder_test.rs)

src/lib/
├── panel/filesPanelStore.svelte.ts   # add reactive `readyThumbs` set (paths whose thumbnail is ready)
├── panel/FilesPanel.svelte           # listen "thumbnail-ready" → mark path ready
├── reusable/FileTree.svelte          # show low thumbnail once the path is ready (placeholder before)
└── types.ts                          # ThumbnailReady payload type (if needed)
```

**Structure Decision**: A new `folder/` module becomes the single owner of folder operations,
replacing `filetree.rs` (scan + watcher move in), exposing a `PhotoFolder` struct (the class,
mirroring `Photo`) as the entry point. The scan builds the tree quickly and records
deterministic thumbnail paths (via a new paths-only `photo::thumbnail::thumbnail_paths`); a
`pipeline` submodule runs the producer–consumer pool that calls `photo::ensure_thumbnails`
per photo (visible-first), emits `thumbnail-ready`, and is cancelled on folder switch. The
frontend marks photos ready from the event and renders their low thumbnail then. `get_thumbnails`
(feature 002) remains as the viewer fallback.

## Complexity Tracking

> No Constitution Check violations. The hand-rolled producer–consumer pool (`std::thread` + `mpsc`)
> was a deviation from the original §VIII "use rayon" wording; constitution **v1.1.0** amended §VIII
> to permit an equivalent thread pool for concurrent workloads like thumbnail generation. It is now
> compliant and adds no new dependency (Principle X).
