# Feature Specification: Configurable Photo Caching

**Feature Branch**: `005-cache-settings`

**Created**: 2026-06-25

**Status**: Draft

**Input**: User description: "Add four caching settings toggles: (1) photo caching on/off (default off) — off shows photos in the UI via direct links, on shows them via thumbnails; (2) small-thumbnail-only caching as a separate toggle (default off, independent of #1), which means small-thumbnail generation must be split from large in the logic, and when both #1 and #2 are on the conversion runs in a single file read; (3) lazy caching (default off) — generate thumbnails when a photo is opened in the viewer rather than when a folder is opened; (4) current-folder-only caching (default on) — cache only photos in the current folder, no recursion into subfolders."

## Clarifications

### Session 2026-06-25

- Q: How do toggles #1 (Photo caching) and #2 (Cache small thumbnails) map onto preview sizes? → A: Orthogonal — "Photo caching" governs the large/viewer (high) preview; "Cache small thumbnails" governs the small/list (low) preview. When only "Cache small thumbnails" is on, only the low thumbnail is cached.
- Q: In lazy mode, are both sizes deferred, or only the large? → A: Both, with per-size triggers — the low (list) thumbnail is generated when its item is shown in the folder hierarchy, and the high (viewer) thumbnail when the photo is opened in the viewer.
- Q: Does "Current folder only" affect explicitly opening a subfolder photo in the viewer? → A: No — an explicit viewer-open always generates the opened photo's thumbnail (if caching is on), even from a subfolder; "Current folder only" limits only bulk/passive generation.

## User Scenarios & Testing *(mandatory)*

> Stories are listed in the user's order (toggle 1–4); priorities reflect impact, not list order.

### User Story 1 - Turn photo caching on or off (Priority: P1)

A user opens the settings menu and finds a "Photo caching" checkbox (off by default). With it
**off**, photos are shown in the UI straight from the original files (no thumbnails are produced or
used). With it **on**, photos are shown via cached thumbnails, generated as needed. This lets users
who don't want the `_thumbnail` cache opt out entirely, and users who want fast previews opt in.

**Why this priority**: This is the master switch the other toggles build around and the headline
control the user asked for; it must exist and behave correctly before the finer controls matter.

**Independent Test**: Toggle "Photo caching" off, open a folder and a photo → no thumbnails are
created and the viewer shows the original. Toggle it on → the viewer shows a cached thumbnail.

**Acceptance Scenarios**:

1. **Given** photo caching is off (default), **When** the user opens a folder and a photo, **Then** the UI displays the photo from the original file and no viewer thumbnails are generated.
2. **Given** photo caching is on, **When** the user views a photo, **Then** the UI displays it via a cached thumbnail, generating it if missing.
3. **Given** the setting is changed, **When** the user continues using the app, **Then** the new behavior takes effect immediately without restarting.

---

### User Story 2 - Cache small thumbnails independently (Priority: P2)

A user finds a separate "Cache small thumbnails" checkbox (off by default), independent of photo
caching. It governs the small (list/tree) preview specifically. Because small and large previews are
now controlled independently, the app must be able to produce each on its own. When **both** photo
caching and small-thumbnail caching are on, producing a photo's previews reads/decodes the source
file only once to make both sizes.

**Why this priority**: Independent control of the small preview is a distinct user need and forces the
small/large split that the rest of the caching logic depends on.

**Independent Test**: With small-thumbnail caching on (photo caching off), the list shows cached small
previews while the viewer still shows originals. With both on, confirm a photo's source is decoded once
to produce both sizes.

**Acceptance Scenarios**:

1. **Given** small-thumbnail caching is off (default), **When** the user browses a folder, **Then** the list shows no generated small previews.
2. **Given** small-thumbnail caching is on, **When** the user browses a folder, **Then** the list shows cached small previews (independently of whether photo caching is on).
3. **Given** both photo caching and small-thumbnail caching are on, **When** a photo's previews are produced, **Then** the source file is decoded a single time to create both the small and the large preview.

---

### User Story 3 - Lazy generation on viewer open (Priority: P3)

A user finds a "Lazy caching" checkbox (off by default). With it **off**, thumbnails are produced when
a folder is opened (eager). With it **on**, a photo's thumbnails are produced only when that photo is
opened in the viewer — opening a folder produces nothing up front. This suits large folders where the
user only views a few photos.

**Why this priority**: A useful efficiency mode, but secondary to the on/off and scope controls.

**Independent Test**: With lazy on, open a folder → no thumbnails are generated; open a photo in the
viewer → that photo's thumbnail(s) are generated.

**Acceptance Scenarios**:

1. **Given** lazy caching is on, **When** the user opens a folder, **Then** no thumbnails are generated at open time.
2. **Given** lazy caching is on, **When** the user opens a photo in the viewer, **Then** that photo's large (high) thumbnail is generated at that moment; its small (low) thumbnail is generated when its item is shown in the folder hierarchy.
3. **Given** lazy caching is off (default), **When** the user opens a folder, **Then** thumbnails are generated at folder-open time (subject to the other toggles).

---

### User Story 4 - Limit caching to the current folder (Priority: P2)

A user finds a "Current folder only" checkbox (**on** by default). With it **on**, caching covers only
the photos directly in the opened folder, without descending into subfolders. With it **off**, caching
also covers photos in nested subfolders. This keeps caching cheap for deep trees by default.

**Why this priority**: Default-on and it bounds how much work caching does, so it materially shapes the
default behavior of every other toggle.

**Independent Test**: With current-folder-only on (default), open a folder containing subfolders →
only the top-level photos are cached. Turn it off → subfolder photos are cached too.

**Acceptance Scenarios**:

1. **Given** current-folder-only is on (default), **When** the user opens a folder with subfolders, **Then** only photos directly in that folder are cached; subfolder photos are not.
2. **Given** current-folder-only is off, **When** the user opens a folder with subfolders, **Then** photos in subfolders are also cached.

---

### Edge Cases

- Photo caching is on but a thumbnail is not yet ready (lazy mode, or mid-generation) → the UI falls back to showing the original so no broken image appears.
- Lazy on + small-thumbnail caching on → list previews are not generated at folder open; each small thumbnail is generated when its item is shown in the hierarchy (the original/icon is shown until then), and the large thumbnail is generated on viewer-open.
- Current-folder-only on while the user browses into a subfolder in the tree → subfolder list items are not auto-cached (the original/icon is shown), but explicitly opening a subfolder photo in the viewer still generates its large thumbnail (FR-017).
- A toggle is changed while generation is in progress → the change takes effect for subsequent work without crashing the in-flight run.
- A cached thumbnail already exists on disk but its governing toggle is now off → the UI shows the original (does not use the cache); the existing cache file is left intact, not deleted.
- All toggles at defaults → photos display from originals, nothing is cached, and only the current folder would be in scope if caching were enabled.

## Requirements *(mandatory)*

### Functional Requirements

#### Settings & defaults

- **FR-001**: The settings menu MUST present four labeled on/off controls: "Photo caching", "Cache small thumbnails", "Lazy caching", and "Current folder only".
- **FR-002**: Defaults MUST be: Photo caching = off, Cache small thumbnails = off, Lazy caching = off, Current folder only = on.
- **FR-003**: All four settings MUST persist across application restarts and take effect immediately, without requiring a restart.
- **FR-004**: "Cache small thumbnails" MUST be independent of "Photo caching" (either may be on or off regardless of the other).

#### Display behavior

- **FR-005**: When "Photo caching" is off, the UI MUST display the large/viewer photo from the original file (a direct reference), not a thumbnail.
- **FR-006**: When "Photo caching" is on, the UI MUST display the large/viewer photo via a cached thumbnail, generating it if absent.
- **FR-007**: When "Cache small thumbnails" is off, the list/tree MUST display the small preview from the original (or a placeholder/icon), not a generated small thumbnail.
- **FR-008**: When "Cache small thumbnails" is on, the list/tree MUST display the small preview via a cached small thumbnail.
- **FR-009**: When a governing toggle is off, the UI MUST NOT use an existing cached thumbnail for that size, and MUST NOT delete it.

#### Generation behavior

- **FR-010**: Small (list) thumbnail generation MUST be separable from large (viewer) thumbnail generation, so each size can be produced and cached independently according to its toggle.
- **FR-011**: When both sizes are produced in the same operation (eager generation with both "Photo caching" and "Cache small thumbnails" on), the source file MUST be decoded a single time to create both sizes (no double decode). In lazy mode the two sizes are produced at different moments and are decoded independently.
- **FR-012**: When "Lazy caching" is off, thumbnails (for the sizes whose toggles are on) MUST be generated when a folder is opened.
- **FR-013**: When "Lazy caching" is on, thumbnails MUST NOT be generated at folder open; instead each size is generated when first needed for display — the small (low) thumbnail when its item is shown in the folder hierarchy, and the large (high) thumbnail when the photo is opened in the viewer.
- **FR-014**: When "Current folder only" is on, automatic generation (eager at folder open, and lazy on-hierarchy-display) MUST cover only photos directly in the opened folder and MUST NOT descend into subfolders (an explicit viewer-open is exempt — see FR-017).
- **FR-015**: When "Current folder only" is off, generation MUST also cover photos in nested subfolders.
- **FR-016**: Existing valid cached thumbnails MUST be reused (not regenerated) when their governing toggle is on.
- **FR-017**: Explicitly opening a photo in the viewer MUST generate its large (high) thumbnail (when "Photo caching" is on) regardless of "Current folder only" — scope restricts only bulk/passive generation, not an explicit open.

### Key Entities *(include if feature involves data)*

- **Cache Settings**: The four persisted booleans — `photoCaching` (default off), `smallThumbnailCaching` (default off), `lazyCaching` (default off), `currentFolderOnly` (default on) — read by both the display layer and the generation layer.
- **Thumbnail size**: The two independently-governed preview sizes — small (list/tree) and large (viewer) — each produced/used per its own toggle.
- **Display source (per photo, per size)**: The resolved choice of what the UI shows for a size — the original file (direct) or a cached thumbnail — derived from the settings and cache availability.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The settings menu shows all four toggles with the specified defaults (Photo caching off, Cache small thumbnails off, Lazy caching off, Current folder only on).
- **SC-002**: With Photo caching off, opening a folder and a photo generates 0 viewer thumbnails and the viewer shows the original.
- **SC-003**: With Cache small thumbnails off, the list shows 0 generated small thumbnails.
- **SC-004**: With both Photo caching and Cache small thumbnails on and lazy off, eager generation of a photo's previews decodes its source file exactly once to produce both sizes.
- **SC-005**: With Lazy caching on, opening a folder generates 0 thumbnails; showing a list item in the hierarchy generates its small thumbnail, and opening a photo in the viewer generates its large thumbnail.
- **SC-006**: With Current folder only on (default), opening a folder with subfolders auto-caches 0 subfolder photos (an explicit viewer-open of a subfolder photo still caches its large thumbnail); with it off, subfolder photos are auto-cached.
- **SC-007**: All four settings survive an app restart and change behavior without a restart.
- **SC-008**: Toggling any setting deletes or corrupts 0 existing valid cached thumbnails.

## Assumptions

- **Toggle-to-size mapping** (confirmed): "Photo caching" governs the large/viewer preview (high thumbnail); "Cache small thumbnails" governs the small/list preview (low thumbnail), orthogonally. With only "Cache small thumbnails" on, only the low thumbnail is cached.
- **Lazy triggers per size** (confirmed): in lazy mode the low thumbnail is generated when its item is shown in the folder hierarchy, and the high thumbnail when the photo is opened in the viewer.
- **"Current folder"** = the top level of the folder the user opened (not a subfolder navigated to within the tree).
- **Cache location/format unchanged**: The existing sibling `_thumbnail` cache (its location, JPG format, and the two size targets) is unchanged; this feature changes only *whether*, *which size*, *when*, and *how deep* thumbnails are generated, and whether they are used for display.
- **Scope vs explicit open** (confirmed): "Current folder only" restricts automatic generation (eager + lazy-on-hierarchy) to the opened folder's top level; explicitly opening any photo in the viewer always generates its high thumbnail regardless of scope.
- **Fallback on miss**: When a size's toggle is on but its cached thumbnail isn't ready yet, the UI shows the original for that size until the thumbnail exists.
- **Out of scope**: cache eviction/cleanup/size limits; a "regenerate cache" action; changing thumbnail dimensions or the cache folder; per-folder (rather than global) settings.
- Settings live in the application's existing settings store and settings menu alongside the current options.
