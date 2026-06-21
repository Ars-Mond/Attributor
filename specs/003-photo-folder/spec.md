# Feature Specification: Photo Folder Handler

**Feature Branch**: `003-photo-folder`

**Created**: 2026-06-21

**Status**: Draft

**Input**: User description: "Create a dedicated class for working with 'photo folders' in the project. It must implement reading a folder, reading subfolders, assembling photo paths, searching photo paths, searching thumbnails, creating thumbnails, and more. Thumbnail creation must use a producer–consumer approach over a thread pool (multithreaded). This class owns opening the folder the user selected and all folder operations in the project. It must not take on other responsibilities, e.g. those of the photo class."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Open a folder and browse immediately (Priority: P1)

When the user selects a folder, the application reads it and its subfolders and shows the
structure (folders and photos) right away — without waiting for thumbnails. Thumbnails fill
in progressively as they are produced in the background, so the user can browse and select
photos immediately even in large folders.

**Why this priority**: Folder open is the entry point to everything; a fast, non-blocking
open is the core value and removes today's wait where the whole folder must finish generating
thumbnails before anything appears.

**Independent Test**: Open a folder with many photos and no cached thumbnails → the folder
structure appears quickly and is navigable while thumbnails keep appearing; the UI never freezes.

**Acceptance Scenarios**:

1. **Given** a folder with many photos and no thumbnails, **When** it is opened, **Then** the folder structure is shown promptly and is navigable before all thumbnails exist.
2. **Given** thumbnails being produced in the background, **When** the user browses or selects photos, **Then** the UI stays responsive and previews appear as they complete.
3. **Given** a folder with subfolders, **When** it is opened, **Then** photos in subfolders are included and `_thumbnail` cache folders are excluded from the structure.

---

### User Story 2 - Concurrent thumbnail creation (Priority: P2)

For the photos in an opened folder, the application creates thumbnails concurrently using a
producer–consumer model over a bounded thread pool: a producer enumerates photos that need
thumbnails, and a fixed set of worker threads generate them in parallel. Existing valid
thumbnails are reused; only missing ones are generated.

**Why this priority**: Concurrency makes generating a whole folder's thumbnails much faster
than one-at-a-time while keeping the machine responsive — the multithreading requirement.

**Independent Test**: Open a folder of N photos without thumbnails on a multi-core machine →
generation uses several workers in parallel and finishes meaningfully faster than sequential;
the machine stays responsive throughout.

**Acceptance Scenarios**:

1. **Given** many photos needing thumbnails, **When** the folder is opened, **Then** thumbnails are produced by multiple workers in parallel (bounded), not one at a time.
2. **Given** photos that already have valid thumbnails, **When** the folder is opened, **Then** those are reused and only missing thumbnails are generated.
3. **Given** the user switches to another folder mid-generation, **When** the switch happens, **Then** pending work for the previous folder is stopped/superseded and no longer consumes resources.

---

### User Story 3 - Folder operations and queries (Priority: P3)

The folder handler is the single place that performs folder operations: enumerating all photo
paths in the open folder, searching for photos and their thumbnails, and watching the folder
for changes (added/removed photos) so the view and thumbnails stay current.

**Why this priority**: These operations back navigation, batch actions, and live updates, but
they build on the open/scan capability from US1.

**Independent Test**: With a folder open, request the list of all photo paths → it returns every
supported photo (excluding `_thumbnail`); add a photo on disk → it appears and gets a thumbnail.

**Acceptance Scenarios**:

1. **Given** an open folder, **When** all photo paths are requested, **Then** every supported photo across the tree is returned and `_thumbnail` contents are excluded.
2. **Given** an open folder, **When** a photo's thumbnail is requested, **Then** its cached thumbnail is located (or created if missing).
3. **Given** an open folder, **When** a photo is added or removed on disk, **Then** the structure updates and a new photo receives a thumbnail.

---

### Edge Cases

- The folder is empty or contains no supported photos → an empty structure is shown without error.
- A subfolder is unreadable (permissions) or an entry is broken → it is skipped and logged; the scan continues.
- A folder holds thousands of photos / deep subfolder nesting → generation stays bounded and the UI stays responsive; the structure still appears promptly.
- The folder already has cached thumbnails → they are reused; only missing thumbnails are created.
- The user switches folders while generation is in progress → previous-folder work is stopped/superseded.
- The folder changes during generation (files added/removed) → the structure updates and new photos get thumbnails.
- `_thumbnail` cache folders are never shown as browsable entries.
- The same folder behaves identically on Windows, Linux, and macOS.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a single folder abstraction that owns all folder operations — open a user-selected folder, read the folder and its subfolders, enumerate photo paths, search photos and thumbnails, create thumbnails, and watch for changes.
- **FR-002**: Opening a folder MUST read it and its subfolders and present the structure (folders and photos) without waiting for thumbnails to be created.
- **FR-003**: Thumbnail creation for a folder's photos MUST use a producer–consumer model over a bounded thread pool — a producer feeds photos needing thumbnails to multiple worker threads that generate them in parallel.
- **FR-004**: Concurrent generation MUST be bounded (a limited number of workers) so the machine and UI stay responsive, and it MUST NOT block folder browsing.
- **FR-005**: System MUST reuse existing valid thumbnails and generate only the missing ones.
- **FR-006**: The folder abstraction MUST delegate per-photo work (creating a single photo's thumbnails, reading a single photo's metadata, decoding) to the photo abstraction and MUST NOT reimplement those single-photo responsibilities.
- **FR-007**: System MUST enumerate (assemble) all supported photo paths within the open folder tree.
- **FR-008**: System MUST support locating a photo's thumbnails and searching for photos within the open folder.
- **FR-009**: System MUST watch the open folder for changes and reflect added/removed photos, creating thumbnails for newly added photos.
- **FR-010**: `_thumbnail` cache folders MUST be excluded from the folder structure.
- **FR-011**: Unreadable or broken entries MUST be skipped gracefully (logged); a single failure MUST NOT abort the scan or generation.
- **FR-012**: Switching to a different folder MUST stop or supersede pending thumbnail work for the previous folder.
- **FR-013**: Thumbnails MUST become visible progressively as they are produced, without freezing the UI.
- **FR-014**: All folder operations MUST behave identically across Windows, Linux, and macOS.

### Key Entities *(include if feature involves data)*

- **Photo Folder**: The opened user-selected folder. Owns the folder structure, the list of photos, the thumbnail-generation pipeline, and the change watcher. The single entry point for folder operations.
- **Folder Structure**: The tree of folders and photos under the open folder, excluding `_thumbnail` cache folders.
- **Thumbnail Pipeline**: The producer–consumer work pipeline — a producer enumerating photos that need thumbnails and a bounded pool of workers generating them in parallel (each via the photo abstraction).
- **Photo (delegated)**: The existing per-photo abstraction the folder uses for single-photo work; not reimplemented here.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A folder's structure appears and is navigable within 1 second of selection for typical folders, regardless of how many thumbnails still need generating.
- **SC-002**: Generating a folder's thumbnails uses multiple workers in parallel and is at least 2× faster than one-at-a-time generation on a 4-core machine.
- **SC-003**: While a large folder's thumbnails are being generated, the user can browse and select photos with no perceptible freeze (UI stays responsive 100% of the time).
- **SC-004**: Valid existing thumbnails are reused in 100% of cases; only missing thumbnails are generated.
- **SC-005**: After switching folders, work for the previous folder stops within 1 second and no longer consumes CPU.
- **SC-006**: Enumerating photos returns 100% of supported photos in the tree and 0% of `_thumbnail` entries.
- **SC-007**: Behavior is identical across Windows, Linux, and macOS.

## Assumptions

- **Tree first, thumbnails async**: Opening a folder returns the structure immediately; thumbnails are generated in the background and surface progressively. (Alternative — block the open until all thumbnails exist — is rejected; it is the slow behavior this feature removes.)
- **Eager folder-wide generation**: When a folder opens, thumbnails for all its photos are scheduled for generation (missing ones), processed concurrently by the pool. (On-demand-only generation is out of scope here.)
- **Bounded concurrency**: The worker count is bounded relative to available CPU cores so the machine stays responsive; the exact size is an implementation detail.
- **Reuse the photo abstraction**: Per-photo thumbnail creation and metadata are delegated to the existing photo component; this folder component performs no single-photo decode/metadata logic itself (single responsibility, per the request).
- **Consolidation**: This becomes the single owner of folder operations in the project; existing folder behaviors (scan, hierarchy, watching, scan-time thumbnail generation) are routed through it, replacing the scattered/inline logic and the previous single-threaded folder-open generation.
- **Thumbnails definition**: Low/high thumbnail sizes, format, naming, and the `_thumbnail` location are as already established for photos; this feature reuses them.
- **Search scope**: "Searching" photos/thumbnails means locating them within the currently open folder tree (by path/name/existence), not a global or content-based search.
- **No new external dependency** beyond what the project already provides is assumed; the concurrency uses the project's existing capabilities.
