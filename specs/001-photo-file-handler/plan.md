# Implementation Plan: Unified Photo File Handler

**Branch**: `001-photo-file-handler` | **Date**: 2026-06-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/001-photo-file-handler/spec.md`

## Summary

Introduce a single backend `Photo` abstraction that reads a photo's metadata by merging
every block (EXIF → IPTC → XMP) with EXIF-wins precedence, edits the four logical fields
(title, description, keywords, category) in memory, saves them back duplicated into every
format-supported block (removing cleared fields), and decodes the image to in-memory RGBA
as a separate, IPC-free operation reserved for future use. All metadata reads stream via
`Seek + Read` (no whole-file load) using the project's `little_exif` fork path API. The
abstraction then replaces the current XMP-only command path so the app reads and writes
EXIF + IPTC + XMP everywhere.

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript 5.6 frontend (little/no change)

**Primary Dependencies**: Tauri 2; `little_exif` fork (git rev `ba71e6a`, v0.6.23 — EXIF/IPTC/XMP read+write, path-based streaming API); `image` 0.25 (jpeg/png/webp decode, already present); `quick-xml` 0.39 (XMP packet build/parse); `log` + `tauri-plugin-log`. No new dependency required.

**Storage**: Image files on disk — JPEG, PNG, WebP

**Testing**: `cargo test` (integration tests under `src-tauri/tests/`); `npx svelte-check` for any frontend touch

**Target Platform**: Windows / Linux / macOS desktop (Tauri)

**Project Type**: Desktop application (Rust/Tauri backend + SvelteKit/Svelte 5 frontend)

**Performance Goals**: Merge-read of a typical stock photo (≤50 MB) under 500 ms; metadata read/save memory independent of file size (streaming reads)

**Constraints**: Reads MUST use `Seek + Read`, never a whole-file load; image pixels preserved byte-for-byte on save; image decode stays in-process (no IPC); conflict precedence EXIF > IPTC > XMP; cleared fields removed from every block

**Scale/Scope**: Single-user desktop; batch operates per-file over the current selection

## Constitution Check

*GATE: evaluated against `.specify/memory/constitution.md` v1.0.0. Re-checked after design.*

| # | Principle | Status | Notes |
|---|-----------|--------|-------|
| I | Pure Rust Backend | PASS | `little_exif` (deps: brotli, crc, miniz_oxide) and `image` are pure Rust; no FFI, no native libs added |
| II | Modern Svelte 5 (Runes) | PASS | Command signatures unchanged; if any `.svelte`/`.ts` is touched it uses runes only |
| III | Themed SCSS Tokens | PASS | No styling work in this feature |
| IV | Cross-Platform Parity | PASS | Pure-Rust path; no `#[cfg(target_os)]` gating of behavior |
| V | Reuse UI Primitives | PASS | No new UI; existing panels keep their components |
| VI | Mandatory Logging | PASS | All fallible operations log via `log` at error sites (English, concise) |
| VII | Phase-Based Commits | PASS | One commit per Spec Kit phase via `/speckit-git-commit` |
| VIII | Rust Performance First | PASS | All parse/decode/merge/write in Rust; image decode never crosses IPC; batch stays per-file (rayon available if batch moves into Rust) |
| IX | Typed Tauri IPC | PASS | Commands keep `Result<T, String>`, `serde` DTOs, `#[serde(rename_all = "camelCase")]`; never panic across the boundary |
| X | Fixed Stack | PASS | Reuses already-declared `little_exif` + `image`; no new dependency to justify |
| XI | Code Style | PASS | English identifiers/comments; no inner-brace spaces in TS; no field alignment in project code |

**Result**: PASS — no violations, Complexity Tracking not required.

## Project Structure

### Documentation (this feature)

```text
specs/001-photo-file-handler/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── photo-api.md     # Rust Photo module public API
│   └── ipc-commands.md  # Tauri command contracts
└── checklists/
    └── requirements.md  # From /speckit-specify
```

### Source Code (repository root)

```text
src-tauri/src/
├── lib.rs               # Tauri commands — switch read_metadata/save_metadata to the Photo module
├── photo/               # NEW module: the Photo abstraction (evolves the current metadata.rs)
│   ├── mod.rs           # `Photo` struct + `Metadata` + public API (open / metadata / set / save / decode_image)
│   ├── read.rs          # Merge-read: EXIF→IPTC→XMP extraction + EXIF-wins precedence
│   ├── write.rs         # Write-all-blocks + cleared-field removal
│   ├── xmp.rs           # XMP packet build/parse (moved from metadata.rs)
│   └── image.rs         # RGBA decode via the `image` crate (in-process, no IPC)
├── types.rs             # ReadResult / SaveRequest DTOs (unchanged); mapping to photo::Metadata
├── metadata.rs          # REMOVED after migration into photo/
├── xmp.rs               # REMOVED after migration (img-parts XMP path retired)
├── filetree.rs          # Unchanged
├── keywords.rs          # Unchanged
└── events.rs            # Unchanged

src-tauri/tests/
├── metadata_test.rs     # Extend: precedence, write-all-blocks, empty-field removal, PNG/WebP
└── xmp_read.rs          # Update/retire alongside xmp.rs removal
```

**Structure Decision**: A new `photo/` module owns the abstraction, evolved from the existing
`metadata.rs` (which already uses the `little_exif` fork). The legacy `xmp.rs` command path and
the `img-parts` dependency are retired once the `Photo` module is wired into the two Tauri
commands and tests pass. `quick-xml` is retained (XMP packet build/parse moves into `photo/xmp.rs`).

## Complexity Tracking

> No Constitution Check violations — section intentionally empty.
