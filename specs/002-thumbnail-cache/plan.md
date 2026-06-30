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
hidden from the tree, and no path index is persisted. Decode/resize/encode run in Rust
**during the folder scan** (`scan_dir`), which writes each image's thumbnail paths into its
`FileNode` (`thumb_low`/`thumb_high`); the tree and viewer read those paths. A thin
`get_thumbnails` command remains as a viewer fallback for files opened outside a scan.

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript 5.6 / Svelte 5 frontend

**Primary Dependencies**: `image` 0.25 (decode + Lanczos resize + JPEG encode — already present); `tokio` (`spawn_blocking` for CPU work — already present); the existing `photo` module (`Photo::decode_image`). No new dependency.

**Storage**: Thumbnail JPG files on disk under each photo's `_thumbnail/` folder

**Testing**: `cargo test` (thumbnail generation/geometry/reuse); `npx svelte-check` for the frontend

**Target Platform**: Windows / Linux / macOS desktop (Tauri 2)

**Project Type**: Desktop application (Rust/Tauri backend + SvelteKit/Svelte 5 frontend)

**Performance Goals**: Cached high preview shown in <200 ms; first-time generation of both variants <1.5 s for a ≤50 MP photo; UI never freezes

**Constraints**: Longest-side fit (low 360 / high 1920), never upscale; JPG strong compression (atomic temp-then-rename writes); `_thumbnail` sibling folder; both variants generated per trigger; no DB/index of paths; generation runs during the folder scan off the UI thread (sequential, no semaphore/rayon — bounding deep-recursion generation is deferred to a future feature)

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
| VIII | Rust Performance First | PASS | Decode/resize/encode in Rust during the scan's `spawn_blocking` (off the UI thread); paths delivered via `FileNode` (no per-row IPC). Deep-recursion bulk generation slows folder open — an accepted, explicitly deferred tradeoff, not a principle violation |
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
│   └── thumbnail.rs     # NEW: longest-side resize + JPEG encode (atomic write) + cache/ensure logic
├── lib.rs               # get_thumbnails command kept as viewer fallback; register in invoke_handler
├── filetree.rs          # scan_dir: generate per image via ensure_thumbnails, fill FileNode.thumb_*, exclude _thumbnail
└── types.rs             # (Thumbnails DTO lives in thumbnail.rs)

src-tauri/tests/
└── thumbnail_test.rs    # NEW: geometry (longest side), no-upscale, JPG, reuse, formats, errors, _thumbnail excluded

src/lib/
├── types.ts                        # FileNode gains thumb_low? / thumb_high?
├── reusable/FileTree.svelte        # Content mode: use node.thumb_low (not the original)
├── panel/ImageViewerPanel.svelte   # Show high thumbnail + loading indicator (FR-014)
└── routes/+page.svelte             # Viewer source from active node.thumb_high; get_thumbnails fallback + loading
```

**Structure Decision**: Thumbnail generation lives in a new `photo/thumbnail.rs` submodule that
reuses the existing decode capability and the `image` crate. The folder scan (`scan_dir`) calls
`ensure_thumbnails` per image and records the resulting paths on each `FileNode`
(`thumb_low`/`thumb_high`), so the tree and viewer read paths directly with no per-row IPC.
Generation is sequential inside the scan's existing `spawn_blocking` (off the UI thread); a
`get_thumbnails` command is retained only as a viewer fallback for files opened outside a scan.
The file tree and viewer are extended in place (Principle V). Bounding generation for deep
recursive trees is deferred (no semaphore/rayon added now). Nothing is persisted to a path index
(FR-007).

## Complexity Tracking

> No Constitution Check violations — section intentionally empty.
