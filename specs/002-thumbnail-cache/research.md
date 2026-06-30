# Phase 0 Research: Photo Thumbnail Cache

Decisions are grounded in the current code (`photo` module, `filetree.rs`, `FileTree.svelte`,
`ImageViewerPanel.svelte`, `tauri.conf.json`) and the already-present `image` 0.25 crate.

## 1. Resize geometry — longest-side fit, no upscale

- **Decision**: Decode the source, then `image::DynamicImage::resize(target, target, FilterType::Lanczos3)`,
  which scales the image to fit within a `target × target` box preserving aspect ratio — i.e. the longest
  side becomes `target` (low 360, high 1920). Guard against upscaling: if the source's longest side ≤ the
  target, skip resizing and encode the source as-is.
- **Rationale**: Matches the clarified "scale by the longest side, proportional, never upscale" (FR-005).
  Lanczos3 gives good downscale quality.
- **Alternatives considered**: `DynamicImage::thumbnail()` (faster nearest-ish filter, lower quality) —
  rejected for visible artifacts; fixed-width-only — rejected per clarification; crop-to-square — rejected
  (distortion / data loss).

## 2. JPEG output with strong compression

- **Decision**: Encode with `image::codecs::jpeg::JpegEncoder::new_with_quality(writer, 75)` over the
  resized image converted to `rgb8` (JPEG has no alpha). Quality 75 is the starting "strong compression,
  acceptable quality" point; tune against SC-003 (low <50 KB, high <500 KB).
- **Rationale**: Explicit quality control (FR-004); `to_rgb8` avoids alpha-related encode errors.
- **Alternatives considered**: `DynamicImage::save`/`write_to` with default quality — rejected (no quality
  control); PNG/WebP thumbnails — rejected (request mandates JPG, and JPG is smaller for photos).

## 3. Source decode

- **Decision**: `image::open(source)?` → `DynamicImage` inside `photo/thumbnail.rs`. The `image` crate is
  built with `jpeg`,`png`,`webp` features, covering all supported sources.
- **Rationale**: One decode yielding a `DynamicImage` ready to resize. (`Photo::decode_image` returns
  `RgbaImage` and would also work; `image::open` keeps native channels and is simplest here.)

## 4. Cache location, naming, and validity

- **Decision**: `_thumbnail` folder = `source.parent()/_thumbnail` (created on demand, FR-003). Thumbnail
  file name = `<source_file_name>.<low|high>.jpg`, e.g. `1662451436_803a15.png.low.jpg`. Including the full
  source file name (with extension) makes names deterministic (FR-012) and collision-free between same-stem
  files (`a.jpg` vs `a.png`). A thumbnail is "valid" if it exists and is non-empty; if a later load fails it
  is regenerated (FR-011).
- **Rationale**: Convention-derived paths, no DB/index (FR-007). Existence/validity-based reuse (FR-002/011).
- **Alternatives considered**: stem-only names — rejected (collisions); content-hash names — rejected
  (extra read of the whole source; over-engineered for this iteration).

## 5. Generation trigger, command shape, and non-blocking execution

- **Decision**: One async command `get_thumbnails(path) -> Result<Thumbnails, String>` ensures BOTH variants
  exist (generating any missing/invalid one) and returns their absolute paths. CPU work runs in
  `tokio::task::spawn_blocking` so the UI never freezes (FR-009). The frontend calls it when a tree row is
  shown (uses `low`) and when the viewer opens a photo (uses `high`).
- **Rationale**: Generating both on either trigger matches the clarification; a single typed call per photo
  avoids hot-loop IPC (Principle VIII/IX).
- **Alternatives considered**: separate `get_low`/`get_high` commands — rejected (two round-trips, and the
  trigger generates both anyway); a batch `get_thumbnails_for_folder` with `rayon` — deferred (useful later
  for eager pre-generation, but the spec is per-photo on display); embedding thumbnail paths in `FileNode`
  during scan — rejected (couples scan to generation, eager, and bloats the tree payload).

## 6. Hiding `_thumbnail` from the file hierarchy

- **Decision**: In `filetree.rs::scan_dir`, skip any child directory whose name is `_thumbnail` (FR-008).
- **Rationale**: One-line guard at the existing child-iteration site; keeps cache folders out of the tree.

## 7. Asset-protocol access for thumbnails

- **Decision**: No change needed. `tauri.conf.json` sets `assetProtocol.scope = ["**"]`, so thumbnail files
  (under the photo folders) are already loadable via `convertFileSrc`, exactly as originals are today.
- **Rationale**: Verified in config; the tree already serves originals through `convertFileSrc(node.path)`.

## 8. Frontend integration & in-memory path cache

- **Decision**: `FileTree.svelte` content mode replaces `convertFileSrc(node.path)` (the full original) with
  the `low` thumbnail obtained from `get_thumbnails`; `ImageViewerPanel.svelte` shows the `high` thumbnail and
  a loading indicator until the command resolves (FR-014). A small runes module
  (`thumbnailCache.svelte.ts`) memoizes `path → {low, high}` in memory to avoid re-invoking on every tree
  re-render. It persists nothing (consistent with FR-007).
- **Rationale**: Reuses existing components (Principle V), runes only (Principle II); the in-memory cache is a
  runtime perf aid, not a path store.
