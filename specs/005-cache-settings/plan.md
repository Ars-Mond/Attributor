# Implementation Plan: Configurable Photo Caching

**Branch**: `005-cache-settings` | **Date**: 2026-06-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/005-cache-settings/spec.md`

## Summary

Make the existing thumbnail cache configurable through four persisted settings toggles. The frontend
reads the settings and (a) chooses the display source per size — original (direct) vs cached
thumbnail — and (b) tells the backend which sizes to generate, how deep, and whether to generate at
all up front. The backend's per-photo generation is split so the small (low) and large (high)
thumbnails can be produced independently, while still decoding the source once when both are produced
together. No new dependencies. Settings are passed to the backend as typed command arguments (the
frontend store stays the single source of truth); the backend stays stateless about settings.

Toggle → behavior (per clarifications):
- **Photo caching** (default off) → large/viewer (high): off shows the original directly, on shows a cached high thumbnail.
- **Cache small thumbnails** (default off) → small/list (low): off shows the original directly, on shows a cached low thumbnail.
- **Lazy caching** (default off) → off generates at folder open; on generates low when its list item is shown in the hierarchy and high on viewer-open.
- **Current folder only** (default on) → automatic generation covers only the opened folder's top level; an explicit viewer-open is exempt (always generates high).

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript + Svelte 5 (runes) frontend.

**Primary Dependencies**: existing only — Tauri 2, `tauri-plugin-store` (settings persistence), the
registry-driven settings system (`src/lib/settings/`), `image` (decode/resize), the `photo`/`folder`
modules. **No new dependencies.**

**Storage**: Settings persisted via the existing `settings.json` store; thumbnails in the existing
sibling `_thumbnail` folders (unchanged location/format).

**Testing**: `cargo test` (split-generation + scoped-pipeline unit/integration tests); `npx svelte-check`.

**Target Platform**: Windows, Linux, macOS desktop.

**Project Type**: Desktop application (Rust backend + SvelteKit/Svelte 5 frontend).

**Performance Goals**: Defaults (all caching off except current-folder-only) generate nothing at
folder open (SC-002/003); with both size toggles on and lazy off, eager generation decodes each
source once for both sizes (SC-004); lazy on generates 0 thumbnails at folder open (SC-005).

**Constraints**: Generation stays in Rust (§VIII); settings cross IPC as typed args, not read from a
store inside hot loops; never panic across IPC (§IX).

**Scale/Scope**: Folders from a handful to thousands of photos; current-folder-only (default) bounds
default work to one level.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Pure Rust Backend | PASS | No new deps; pure-Rust refactor of existing generation. |
| II. Modern Svelte 5 (Runes) | PASS | Settings reads via `settings.subscribe` in `$derived`; FileTree lazy trigger via `$effect`. |
| III. Themed SCSS Tokens | PASS | Any new list placeholder uses tokens; settings UI is registry-rendered. |
| IV. Cross-Platform Parity | PASS | No platform-specific code. |
| V. Reuse UI Primitives | PASS | Reuse the settings registry (4 boolean descriptors auto-render), FileTree, and the viewer. |
| VI. Mandatory Logging | PASS | Existing generation error logging retained; new command errors logged. |
| VII. Phase-Based Commits | PASS | One commit per phase. |
| VIII. Rust Performance First | PASS | Generation stays in Rust; settings cross IPC once per folder open (config) or per explicit user action (lazy), not inside loops. |
| IX. Typed Tauri IPC | PASS | Commands take typed booleans and return `Result<T, String>`, camelCase. |
| X. Fixed Stack | PASS | No new dependencies. |
| XI. Code Style | PASS | English; no inner brace spaces / alignment padding. |

**Gate result**: PASS. No violations; Complexity Tracking not required.

## Project Structure

### Documentation (this feature)

```text
specs/005-cache-settings/
├── plan.md            # This file
├── research.md        # Phase 0 decisions
├── data-model.md      # Phase 1 entities
├── quickstart.md      # Phase 1 validation guide
├── contracts/         # Phase 1
│   ├── settings.md        # the four setting keys, types, defaults
│   └── ipc-commands.md    # parameterized open/scan/cache-thumbnail commands + pipeline config
└── tasks.md           # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
src-tauri/src/
├── photo/thumbnail.rs   # split: ensure(source, low, high) — decode once if either needed,
│                        #   generate only the requested sizes; ensure_thumbnails = ensure(true,true)
├── folder/
│   ├── pipeline.rs      # start(app, root, cancel, want_low, want_high, recursive);
│   │                    #   collect_jobs honors `recursive` (top-level only vs all)
│   └── mod.rs           # open/rescan take a GenConfig and skip the pipeline when nothing is eager
└── lib.rs               # open_folder/open_folder_path/scan_folder take GenConfig;
                         #   cache_thumbnail(path, low, high) for on-demand (viewer/lazy) generation

src/lib/
├── settings/index.ts    # register a "Caching" section + 4 boolean settings (keys + defaults)
├── reusable/FileTree.svelte   # display source per #2; lazy low trigger when an image item is shown
├── panel/FilesPanel.svelte    # icons-mode display per #2 + lazy trigger; pass GenConfig on open/scan
└── routes/+page.svelte        # viewer: high thumbnail (#1 on) vs direct original (#1 off)

src-tauri/tests/
├── thumbnail_test.rs    # ensure(low-only), ensure(high-only), ensure(both → single decode), reuse
└── folder_test.rs       # pipeline with recursive=false (top level only) and with one size only
```

**Structure Decision**: No new modules. The change is a per-size split of the existing generation
(`thumbnail.rs`), a parameterized pipeline + folder open (sizes, recursion, eager-or-not), one new
on-demand command, and frontend wiring that reads the four settings to choose display source and
generation triggers. The folder **tree** is still scanned in full (the user can browse subfolders);
"current folder only" limits only which photos are auto-*generated*, not what is shown.

## Complexity Tracking

No constitution violations — no new dependencies, no new modules, no added concurrency machinery.
Table intentionally empty.
