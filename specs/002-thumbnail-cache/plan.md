# Implementation Plan: Photo Thumbnail Cache

**Branch**: `002-thumbnail-cache` | **Date**: 2026-06-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/002-thumbnail-cache/spec.md`

## Summary

Generate and cache two JPG thumbnails per photo — low (360px longest side) and high
(1920px longest side) — in a `_thumbnail` folder beside the source. Both variants are
produced together on first need (a photo shown in the file hierarchy or opened in the
viewer) and reused afterward; existing valid thumbnails are never regenerated. The file
tree shows the low thumbnail (replacing today's full-original-in-an-`<img>`), the viewer
shows the high thumbnail with a loading indicator until ready, `_thumbnail` folders are
hidden from the tree, and no path index is persisted. Decode/resize/encode run in Rust; a
single typed command hands the frontend the thumbnail paths.

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript 5.6 / Svelte 5 frontend

**Primary Dependencies**: `image` 0.25 (decode + Lanczos resize + JPEG encode — already present); `tokio` (`spawn_blocking` for CPU work — already present); the existing `photo` module (`Photo::decode_image`). No new dependency.

**Storage**: Thumbnail JPG files on disk under each photo's `_thumbnail/` folder

**Testing**: `cargo test` (thumbnail generation/geometry/reuse); `npx svelte-check` for the frontend

**Target Platform**: Windows / Linux / macOS desktop (Tauri 2)

**Project Type**: Desktop application (Rust/Tauri backend + SvelteKit/Svelte 5 frontend)

**Performance Goals**: Cached high preview shown in <200 ms; first-time generation of both variants <1.5 s for a ≤50 MP photo; UI never freezes

**Constraints**: Longest-side fit (low 360 / high 1920), never upscale; JPG strong compression; `_thumbnail` sibling folder; both variants generated per trigger; no DB/index of paths; generation off the UI thread

**Scale/Scope**: Single-user desktop; per-photo on-demand generation over the current folder

## Constitution Check

*GATE: evaluated against `.specify/memory/constitution.md` v1.0.0. Re-checked after design.*

| # | Principle | Status | Notes |
|---|-----------|--------|-------|
| I | Pure Rust Backend | PASS | `image` is pure Rust; no FFI/native libs added |
| II | Modern Svelte 5 (Runes) | PASS | `FileTree`/`ImageViewerPanel` already use runes; new state uses `$state`/`$derived`/`$effect` only |
| III | Themed SCSS Tokens | PASS | Loading indicator and any styling use `_mixins`/`_themes` tokens; no hardcoded hex |
| IV | Cross-Platform Parity | PASS | `std::path` handling; pure-Rust image ops; no `cfg(target_os)` gating |
| V | Reuse UI Primitives | PASS | Extend existing `ImageViewerPanel` and `FileTree`; no parallel components |
| VI | Mandatory Logging | PASS | Generation failures logged via `log` at error sites (English, concise) |
| VII | Phase-Based Commits | PASS | One commit per phase via `/speckit-git-commit` |
| VIII | Rust Performance First | PASS | Decode/resize/encode in Rust on `spawn_blocking`; one IPC call per photo (no hot-loop IPC); `rayon` available if a batch command is added |
| IX | Typed Tauri IPC | PASS | `get_thumbnails` returns `Result<Thumbnails, String>`, `#[serde(rename_all = "camelCase")]`, never panics |
| X | Fixed Stack | PASS | Reuses already-declared `image`/`tokio`; no new dependency |
| XI | Code Style | PASS | English identifiers/comments; no inner-brace spaces in TS; no field alignment |

**Result**: PASS — no violations, Complexity Tracking not required.

## Project Structure

### Documentation (this feature)

```text
specs/002-thumbnail-cache/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── ipc-commands.md  # get_thumbnails command contract
│   └── thumbnail-api.md # Rust thumbnail module API
└── checklists/
    └── requirements.md  # From /speckit-specify
```

### Source Code (repository root)

```text
src-tauri/src/
├── photo/
│   ├── mod.rs           # Re-export thumbnail API (ensure_thumbnails, Thumbnails)
│   └── thumbnail.rs     # NEW: longest-side resize + JPEG encode + cache/ensure logic
├── lib.rs               # NEW command get_thumbnails; register in invoke_handler
├── filetree.rs          # Exclude `_thumbnail` directories from scan_dir
└── types.rs             # (Thumbnails DTO lives in thumbnail.rs; no change unless shared)

src-tauri/tests/
└── thumbnail_test.rs    # NEW: geometry (longest side), no-upscale, JPG output, reuse, _thumbnail folder

src/lib/
├── reusable/FileTree.svelte        # Content mode: use low thumbnail path (not the original)
├── panel/ImageViewerPanel.svelte   # Show high thumbnail + loading indicator (FR-014)
├── panel/thumbnailCache.svelte.ts  # NEW (optional): in-memory map path→{low,high} to dedupe invokes
└── routes/+page.svelte             # Wire viewer source to the high thumbnail + loading state
```

**Structure Decision**: Thumbnail generation lives in a new `photo/thumbnail.rs` submodule that
reuses the existing decode capability and the `image` crate, exposed through one async Tauri
command `get_thumbnails`. The file tree and viewer are extended in place (Principle V). An
optional small in-memory frontend cache (runes) dedupes repeated command calls during tree
re-renders; it persists nothing to disk (consistent with FR-007).

## Complexity Tracking

> No Constitution Check violations — section intentionally empty.
