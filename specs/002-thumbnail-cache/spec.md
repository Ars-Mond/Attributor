# Feature Specification: Photo Thumbnail Cache

**Feature Branch**: `002-thumbnail-cache`

**Created**: 2026-06-20

**Status**: Draft

**Input**: User description: "Add photo thumbnail caching. A thumbnail is a photo for display in the app, in two size variants: low and high. When a photo is shown, check whether thumbnails exist beside it; if not, create them; if yes, hand their paths to the app. Thumbnails are JPG with strong compression while keeping enough quality for these sizes. Before saving, create a `_thumbnail` folder in the same folder as the photo. The app uses the small thumbnail in the file hierarchy and the large thumbnail for on-screen display. For now, do not store thumbnail/photo paths in a database or files."

## Clarifications

### Session 2026-06-20

- Q: When and at what scope are thumbnails generated? → A: Both variants (low + high) are generated together; the trigger is either showing the photo in the file hierarchy or opening it in the viewer (whichever happens first), when they are not already cached.
- Q: What is the thumbnail geometry and the high target size? → A: Scale by the longest side, preserving aspect ratio (the other side follows proportionally). Low = 360px on the longest side; high = 1920px on the longest side (corrected from the original 1080).
- Q: What does the viewer show while the high thumbnail is still being generated on first open? → A: A loading indicator until the high thumbnail is ready.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Large preview in the viewer (Priority: P1)

When a user opens (selects) a photo, the on-screen viewer shows a high-quality preview
(1920px on the longest side) instead of decoding the full multi-megapixel original. The
first time a photo is opened, both thumbnails are generated and cached beside the photo and
a loading indicator is shown until the high thumbnail is ready; afterwards it is served
from the cache.

**Why this priority**: The viewer is the primary place a user looks at a photo; a fast,
lightweight large preview is the core value and the most visible win over loading originals.

**Independent Test**: Open a photo with no cache → a loading indicator appears, then a
1920px-longest-side preview, and both thumbnail files are created in a sibling `_thumbnail`
folder. Open it again → the high thumbnail appears immediately from the existing file.

**Acceptance Scenarios**:

1. **Given** a photo with no existing thumbnails, **When** it is opened, **Then** both variants are generated and cached, a loading indicator shows until ready, and the high (1920px longest side) thumbnail is displayed.
2. **Given** a photo whose high thumbnail already exists, **When** it is opened, **Then** the existing thumbnail is shown without regenerating it and without a loading delay.
3. **Given** a source whose longest side is under 1920px, **When** its high thumbnail is generated, **Then** it is not upscaled (kept at the source size).

---

### User Story 2 - Small previews in the file hierarchy (Priority: P2)

When a user browses a folder, each photo in the file hierarchy shows a small preview
(360px on the longest side). Showing a photo in the hierarchy also triggers generation of
both variants (if missing) and caches them, so opening that photo later is instant.

**Why this priority**: Visual browsing makes selecting the right photo far faster than a
text-only tree, but it is secondary to viewing the chosen photo.

**Independent Test**: Display a folder of photos → each shows a 360px-longest-side preview
and both thumbnail files appear per photo in the `_thumbnail` folder; re-displaying reuses them.

**Acceptance Scenarios**:

1. **Given** a folder of photos without thumbnails, **When** a photo is shown in the hierarchy, **Then** both variants are generated and cached and the low (360px longest side) preview is displayed for it.
2. **Given** photos whose thumbnails already exist, **When** the folder is displayed again, **Then** the cached thumbnails are reused with no regeneration.
3. **Given** a `_thumbnail` folder inside a browsed folder, **When** the hierarchy is shown, **Then** the `_thumbnail` folder is not listed as a browsable entry.

---

### User Story 3 - Cache reuse and locality (Priority: P3)

Thumbnails live in a `_thumbnail` folder next to each photo and are reused across sessions.
Re-opening or re-browsing never regenerates a valid existing thumbnail, and the cache travels
with the photos if the folder is copied.

**Why this priority**: Persistence and reuse turn one-time generation cost into lasting speed,
but the app is already usable without it (it would just regenerate).

**Independent Test**: Generate thumbnails, close and reopen the app, open the same photo →
no new files are written and the preview appears immediately.

**Acceptance Scenarios**:

1. **Given** previously generated thumbnails, **When** the app is restarted and the photo reopened, **Then** the cached thumbnails are served with no regeneration.
2. **Given** an existing thumbnail file that is unreadable or invalid, **When** it is needed, **Then** it is regenerated.

---

### Edge Cases

- The `_thumbnail` folder does not exist yet → it is created before the first thumbnail is written.
- The source's longest side is under the target → the thumbnail is not upscaled; the source size is kept.
- The source photo is corrupt or cannot be decoded → generation fails gracefully (logged); the UI shows no preview rather than breaking.
- The `_thumbnail` folder is read-only or the disk is full → generation fails as a handled error; the UI degrades gracefully.
- An existing thumbnail is a leftover 0-byte or invalid file (e.g. from an interrupted write) → it is treated as missing and regenerated.
- A folder contains a `_thumbnail` folder → it is excluded from the photo hierarchy.
- A folder holds many photos → thumbnails are generated on demand, each photo at most once, without freezing the UI.
- The same photo behaves identically on Windows, Linux, and macOS for JPEG, PNG, and WebP sources.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide two thumbnail size variants per photo — low (360px on the longest side) and high (1920px on the longest side).
- **FR-002**: Generation is triggered when a photo is shown in the file hierarchy or opened in the viewer (whichever first). On that trigger, if the thumbnails are not already cached and valid, the system MUST generate BOTH variants, store them in a `_thumbnail` folder beside the photo, and return their paths; if already present and valid, it MUST return the existing paths without regenerating.
- **FR-003**: System MUST create the `_thumbnail` folder in the photo's own directory before writing a thumbnail, when that folder does not exist.
- **FR-004**: Thumbnails MUST be JPG with strong compression while keeping visual quality acceptable for their size.
- **FR-005**: Thumbnails MUST scale by the longest side to the target (360 / 1920), preserving aspect ratio; the system MUST NOT upscale a source whose longest side is smaller than the target.
- **FR-006**: The file hierarchy MUST use the low thumbnail for each photo; the on-screen viewer MUST use the high thumbnail.
- **FR-007**: System MUST hand thumbnail file locations to the application for display, and MUST NOT persist thumbnail or photo paths in a database or index file — paths are derived by convention from the source photo.
- **FR-008**: System MUST exclude `_thumbnail` folders from the photo file hierarchy.
- **FR-009**: Thumbnail generation MUST NOT freeze the user interface.
- **FR-010**: If a source cannot be decoded or a thumbnail cannot be written, the system MUST fail gracefully (logged, no crash) and the UI MUST degrade (no preview) rather than break.
- **FR-011**: If an existing thumbnail is unreadable or invalid, the system MUST regenerate it.
- **FR-012**: A thumbnail's location MUST be derived deterministically from the source photo (name plus size variant), so a photo always maps to the same thumbnail files.
- **FR-013**: System MUST support JPEG, PNG, and WebP sources and behave identically across Windows, Linux, and macOS.
- **FR-014**: On a photo's first open, until its high thumbnail is ready, the viewer MUST show a loading indicator.

### Key Entities *(include if feature involves data)*

- **Source Photo**: The original image file (JPEG/PNG/WebP) in a user folder; the input for thumbnail generation.
- **Thumbnail**: A JPG preview derived from a source photo in one of two variants — low (360px on the longest side) or high (1920px on the longest side). Stored in the photo's `_thumbnail` folder; its name is derived from the source photo and the variant.
- **Thumbnail Folder (`_thumbnail`)**: A folder beside each photo holding that folder's thumbnails; excluded from the browsable hierarchy.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Opening a photo whose high thumbnail is already cached shows the large preview in under 200 ms (no regeneration, no loading indicator).
- **SC-002**: First-time generation of a photo's thumbnails (both variants) for a typical stock photo (≤50 MP) completes in under 1.5 seconds and does not freeze the UI.
- **SC-003**: A low thumbnail file is under 50 KB and a high thumbnail under 500 KB for a typical stock photo, while remaining visually acceptable.
- **SC-004**: A valid cached thumbnail is reused in 100% of reopen/rebrowse cases (zero duplicate generation).
- **SC-005**: Browsing a folder of photos generates each photo's thumbnails at most once.
- **SC-006**: `_thumbnail` folders appear as browsable entries 0% of the time.
- **SC-007**: Thumbnails preserve aspect ratio (no distortion) and are never upscaled, in 100% of cases.
- **SC-008**: JPEG, PNG, and WebP sources all produce valid thumbnails, identically across Windows, Linux, and macOS.

## Assumptions

- **Generation trigger**: Both variants are generated together on first need (per clarification). In practice the backend generates them during the folder scan (each image as the hierarchy is built) and via a viewer fallback for files opened outside a scan. Generation runs off the UI thread (inside the scan); each photo is generated at most once. Bounding generation for deep recursive trees is deferred — folder open is correspondingly slower for now.
- **Resize rule**: Scale by the longest side to the target (low 360, high 1920), preserve aspect ratio, never upscale; the shorter side follows proportionally.
- **Naming**: Deterministic, derived from the source file name (including its extension to avoid collisions between same-stem files) plus the variant, inside `_thumbnail`; this is how "does a thumbnail exist" is checked.
- **Invalidation**: Existence-and-validity based this iteration — a present, readable thumbnail is reused; there is no modification-time or content-hash staleness check. Thumbnails are written atomically (temp file then rename), so a present thumbnail is never half-written. If a source photo is replaced in place, a stale thumbnail may persist (out of scope for now).
- **Compression**: A single strong-compression JPG quality level is used for both variants, chosen to keep files small while acceptable for the size.
- **No path store**: Per the request, no database or index file records thumbnail/photo paths; locations are recomputed by convention each time.
- **Foundation**: Builds on the existing photo abstraction and its image-decode capability; thumbnails are produced from the decoded source. No new external dependency is assumed beyond what the project already provides.
- **Scope**: Generating, caching, locating, and displaying thumbnails. Out of scope: cache eviction/cleanup, a global cache location, bounding/throttling generation for deep recursive folder trees (deferred to a future feature), and persisting any path index.
