# Feature Specification: Configurable Photo Caching

**Feature Branch**: `005-cache-settings`

**Created**: 2026-06-25

**Status**: Draft

**Input**: User description: "Add four caching settings toggles: (1) photo caching on/off (default off) — off shows photos in the UI via direct links, on shows them via thumbnails; (2) small-thumbnail-only caching as a separate toggle (default off, independent of #1), which means small-thumbnail generation must be split from large in the logic, and when both #1 and #2 are on the conversion runs in a single file read; (3) lazy caching (default off) — generate thumbnails when a photo is opened in the viewer rather than when a folder is opened; (4) current-folder-only caching (default on) — cache only photos in the current folder, no recursion into subfolders."

## Clarifications

### Session 2026-06-25

- Q: How do toggles #1 (Photo caching) and #2 (Cache small thumbnails) map onto preview sizes? → A: Orthogonal — "Photo caching" governs the large/viewer (high) preview; "Cache small thumbnails" governs the small/list (low) preview. When only "Cache small thumbnails" is on, only the low thumbnail is cached.
- Q: In lazy mode, are both sizes deferred, or only the large? → A: Both, with per-size triggers. *(Superseded 2026-06-26 — see below.)*
- Q: Does "Current folder only" affect explicitly opening a subfolder photo in the viewer? → A: No — an explicit viewer-open always generates the opened photo's thumbnail (if caching is on), even from a subfolder; scope limits only bulk/passive generation.

### Session 2026-06-26 (revisions during implementation)

- **Lazy applies only to the large (viewer) thumbnail.** The small (list) thumbnail is always generated eagerly when "Cache small thumbnails" is on; "Lazy caching" only defers the large/viewer thumbnail to viewer-open. (Supersedes the 2026-06-25 "both sizes lazy" answer.)
- **"Current folder only" is replaced by a general "Read nested folders" setting.** Recursion is not only a caching concern: it also controls whether the Content view shows nested folders. So the toggle lives in a general settings section (not under Caching), is inverted in wording ("Read nested folders", default off = top level only), and drives **both** Content-view folder nesting **and** cache recursion. It does not affect the Table view (which always shows the full tree).
- **When "Read nested folders" is off, the Content view shows files only** — folder rows are not shown at all (not even an icon). The Table view is unaffected.
- **Concurrent generation of the same photo is de-duplicated**: if two producers (e.g. the eager folder pass and an explicit viewer-open) request the same photo at once, the source is decoded a single time and the others reuse the result.
- **A cache folder deleted on disk is recovered**: if the `_thumbnail` folder is removed while a folder is open, switching to a thumbnail (Content/icons) view detects the missing cache and regenerates the small thumbnails for the current scope.

## User Scenarios & Testing *(mandatory)*

> Stories are listed in the user's original order; priorities reflect impact, not list order.

### User Story 1 - Turn photo caching on or off (Priority: P1)

A user opens the settings menu and finds a "Photo caching" checkbox (off by default). With it
**off**, photos are shown in the UI straight from the original files (no thumbnails are produced or
used). With it **on**, photos are shown via cached thumbnails, generated as needed. This lets users
who don't want the `_thumbnail` cache opt out entirely, and users who want fast previews opt in.

**Why this priority**: This is the master switch the other toggles build around and the headline
control the user asked for; it must exist and behave correctly before the finer controls matter.

**Independent Test**: Toggle "Photo caching" off, open a folder and a photo → no viewer thumbnails
are created and the viewer shows the original. Toggle it on → the viewer shows a cached thumbnail.

**Acceptance Scenarios**:

1. **Given** photo caching is off (default), **When** the user opens a folder and a photo, **Then** the UI displays the photo from the original file and no viewer thumbnails are generated.
2. **Given** photo caching is on, **When** the user views a photo, **Then** the UI displays it via a cached thumbnail, generating it if missing.
3. **Given** the setting is changed, **When** the user reopens a folder, **Then** the new behavior is in effect (settings persist and are applied without a restart).

---

### User Story 2 - Cache small thumbnails independently (Priority: P2)

A user finds a separate "Cache small thumbnails" checkbox (off by default), independent of photo
caching. It governs the small (list/tree) preview specifically. Because small and large previews are
controlled independently, the app produces each on its own. When **both** photo caching and
small-thumbnail caching produce a photo's previews together (eager), the source file is decoded only
once to make both sizes.

**Why this priority**: Independent control of the small preview is a distinct user need and forces the
small/large split that the rest of the caching logic depends on.

**Independent Test**: With small-thumbnail caching on (photo caching off), the list shows cached small
previews while the viewer still shows originals. With both on, confirm a photo's source is decoded once
to produce both sizes.

**Acceptance Scenarios**:

1. **Given** small-thumbnail caching is off (default), **When** the user browses a folder, **Then** the list shows no generated small previews (originals/icons only).
2. **Given** small-thumbnail caching is on, **When** the user opens a folder, **Then** the list's small previews are generated eagerly and shown from the cache (independently of whether photo caching is on).
3. **Given** both photo caching and small-thumbnail caching are on (lazy off), **When** a photo's previews are produced, **Then** the source file is decoded a single time to create both the small and the large preview.

---

### User Story 3 - Lazy generation of the viewer thumbnail (Priority: P3)

A user finds a "Lazy caching" checkbox (off by default). It controls **the large/viewer thumbnail
only**: with it **off**, the large thumbnail is produced eagerly when a folder is opened; with it
**on**, the large thumbnail is produced only when the photo is opened in the viewer. The small (list)
thumbnail is always produced eagerly at folder open and is not affected by this setting. This suits
large folders where the user wants list previews up front but doesn't want every full-size viewer
thumbnail generated in advance.

**Why this priority**: A useful efficiency mode for the expensive large thumbnail, but secondary to
the on/off controls.

**Independent Test**: With lazy on and photo caching on, open a folder → small list thumbnails are
generated but no large/viewer thumbnails; open a photo in the viewer → its large thumbnail is
generated at that moment.

**Acceptance Scenarios**:

1. **Given** lazy caching is on and photo caching is on, **When** the user opens a folder, **Then** no large (viewer) thumbnails are generated at open time.
2. **Given** lazy caching is on, **When** the user opens a photo in the viewer, **Then** that photo's large (high) thumbnail is generated at that moment.
3. **Given** lazy caching is off (default) and photo caching is on, **When** the user opens a folder, **Then** the large thumbnails are generated at folder-open time (subject to the nesting scope).

---

### User Story 4 - Read nested folders (Priority: P2)

A user finds a "Read nested folders" checkbox in the general settings (not under Caching, since it is
not only about caching), **off** by default. With it **off**, the Content view shows only the opened
folder's top level — nested folders are not shown — and caching covers only the top-level photos.
With it **on**, the Content view shows nested folders and caching also covers photos in subfolders.
This setting does not affect the Table view, which always shows the full folder tree.

**Why this priority**: It shapes both what the Content view displays and how deep caching goes, so it
materially affects the default behavior; default-off keeps both cheap for deep trees.

**Independent Test**: With "Read nested folders" off (default), open a folder containing subfolders →
the Content view shows top-level files only (no folder rows) and only top-level photos are cached; the
Table view still shows the full tree. Turn it on → nested folders appear in the Content view and
subfolder photos are cached too.

**Acceptance Scenarios**:

1. **Given** "Read nested folders" is off (default), **When** the user views a folder with subfolders in the Content view, **Then** only top-level files are shown (no folder rows) and only top-level photos are cached.
2. **Given** "Read nested folders" is on, **When** the user views a folder with subfolders in the Content view, **Then** nested folders are shown and photos in subfolders are cached too.
3. **Given** any value of "Read nested folders", **When** the user uses the Table view, **Then** the full folder tree is shown regardless of the setting.

---

### Edge Cases

- Photo caching is on but the large thumbnail is not yet ready (lazy mode, or mid-generation) → the viewer falls back to showing the original so no broken image appears.
- "Read nested folders" off, Content view → nested folders (and their contents) are not shown at all; subfolder photos are not cached. To open a subfolder photo, the user can use the Table view (which shows the full tree); an explicit viewer-open still generates its large thumbnail (FR-017).
- A toggle is changed while generation is in progress → the change takes effect for subsequent work without crashing the in-flight run.
- A cached thumbnail already exists on disk but its governing toggle is now off → the UI shows the original (does not use the cache); the existing cache file is left intact, not deleted.
- The `_thumbnail` cache folder is deleted on disk while a folder is open → switching to a Content/icons view detects the missing cache (for the current nesting scope) and regenerates the small thumbnails; stale "ready" markers are cleared so no broken images linger.
- Two producers request the same photo's thumbnail at the same time (e.g. the eager folder pass and an explicit viewer-open) → the source is decoded once and the second producer reuses the result.
- All settings at defaults → photos display from originals, nothing is cached, and the Content view shows top-level files only.

## Requirements *(mandatory)*

### Functional Requirements

#### Settings & defaults

- **FR-001**: The settings menu MUST present, under a "Caching" section, three on/off controls — "Photo caching", "Cache small thumbnails", "Lazy caching" — and, under a general (non-Caching) section, a "Read nested folders" control.
- **FR-002**: Defaults MUST be: Photo caching = off, Cache small thumbnails = off, Lazy caching = off, Read nested folders = off.
- **FR-003**: All settings MUST persist across application restarts and take effect on subsequent folder opens without requiring a restart.
- **FR-004**: "Cache small thumbnails" MUST be independent of "Photo caching" (either may be on or off regardless of the other).

#### Display behavior

- **FR-005**: When "Photo caching" is off, the viewer MUST display the photo from the original file (a direct reference), not a thumbnail.
- **FR-006**: When "Photo caching" is on, the viewer MUST display the photo via a cached large thumbnail, generating it if absent and falling back to the original until it is ready.
- **FR-007**: When "Cache small thumbnails" is off, the list/tree MUST display the small preview from the original (or a placeholder/icon), not a generated small thumbnail.
- **FR-008**: When "Cache small thumbnails" is on, the list/tree MUST display the small preview via a cached small thumbnail (placeholder/original until ready).
- **FR-009**: When a governing toggle is off, the UI MUST NOT use an existing cached thumbnail for that size, and MUST NOT delete it.
- **FR-010**: When "Read nested folders" is off, the Content view MUST show only the opened folder's top level (files only; nested folder rows are not shown). The Table view MUST be unaffected (it always shows the full tree). Out-of-scope (subfolder) photos shown anywhere MUST display the original, not a cached/placeholder tile.

#### Generation behavior

- **FR-011**: Small (list) thumbnail generation MUST be separable from large (viewer) thumbnail generation, so each size can be produced and cached independently according to its toggle.
- **FR-012**: When both sizes are produced in the same operation, the source MUST be decoded a single time for both sizes. Additionally, concurrent producers of the same photo (e.g. the eager folder pass and an explicit viewer-open) MUST be de-duplicated so the source is decoded only once.
- **FR-013**: The small (low) thumbnail MUST be generated eagerly at folder open when "Cache small thumbnails" is on, regardless of "Lazy caching".
- **FR-014**: "Lazy caching" MUST affect only the large (viewer) thumbnail: when off, the large thumbnail is generated at folder open (when "Photo caching" is on); when on, the large thumbnail is generated when the photo is opened in the viewer.
- **FR-015**: When "Read nested folders" is off, automatic generation MUST cover only photos directly in the opened folder and MUST NOT descend into subfolders; when on, it MUST also cover photos in nested subfolders.
- **FR-016**: Existing valid cached thumbnails MUST be reused (not regenerated) when their governing toggle is on.
- **FR-017**: Explicitly opening a photo in the viewer MUST generate its large (high) thumbnail (when "Photo caching" is on) regardless of "Read nested folders" — the nesting scope restricts only bulk/passive generation, not an explicit open.
- **FR-018**: If the `_thumbnail` cache folder is deleted on disk while a folder is open and "Cache small thumbnails" is on, switching to a thumbnail (Content/icons) view MUST detect the missing cache for the current nesting scope and regenerate the small thumbnails.

### Key Entities *(include if feature involves data)*

- **Cache Settings** (Caching section): the persisted booleans `photoCaching` (default off), `smallThumbnailCaching` (default off), `lazyCaching` (default off) — read by both the display and generation layers.
- **Folder Setting** (general section): `nestedFolders` (default off) — drives both Content-view folder nesting and cache recursion; does not affect the Table view.
- **Thumbnail size**: the two independently-governed preview sizes — small (list/tree) and large (viewer) — each produced/used per its own rules (small always eager; large eager-or-lazy).
- **Display source (per photo, per size)**: the resolved choice of what the UI shows for a size — the original file (direct) or a cached thumbnail — derived from the settings, nesting scope, and cache availability.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The settings show the three Caching toggles plus the general "Read nested folders" toggle, all defaulting off.
- **SC-002**: With Photo caching off, opening a folder and a photo generates 0 viewer thumbnails and the viewer shows the original.
- **SC-003**: With Cache small thumbnails off, the list shows 0 generated small thumbnails.
- **SC-004**: With both Photo caching and Cache small thumbnails on and lazy off, generating a photo's previews decodes its source exactly once for both sizes; and two concurrent producers of the same photo decode it exactly once.
- **SC-005**: With Lazy caching on and Photo caching on, opening a folder generates the small (list) thumbnails but 0 large (viewer) thumbnails; opening a photo in the viewer generates its large thumbnail.
- **SC-006**: With "Read nested folders" off (default), opening a folder with subfolders shows top-level files only in the Content view and auto-caches 0 subfolder photos; with it on, nested folders are shown and subfolder photos are auto-cached. The Table view shows the full tree either way.
- **SC-007**: All settings survive an app restart and are applied on the next folder open without a restart.
- **SC-008**: Toggling any setting deletes or corrupts 0 existing valid cached thumbnails.

## Assumptions

- **Toggle-to-size mapping** (confirmed): "Photo caching" governs the large/viewer (high) preview; "Cache small thumbnails" governs the small/list (low) preview, orthogonally.
- **Lazy is large-only** (revised 2026-06-26): "Lazy caching" defers only the large/viewer thumbnail to viewer-open; the small thumbnail is always generated eagerly at folder open when its toggle is on.
- **"Read nested folders" is a general setting** (revised 2026-06-26): it controls both Content-view folder nesting and cache recursion; default off (top level only); it does not affect the Table view. "Current folder" = the top level of the opened folder.
- **Concurrent de-duplication**: producers of the same photo's thumbnails are serialized so the source is decoded once (an in-flight registry plus atomic temp-then-rename writes); this is safe and avoids redundant decodes.
- **Deleted-cache recovery is best-effort**: it is triggered by switching to a thumbnail view and checks the cache folder for the current nesting scope; an in-place deletion without a view switch is not auto-detected (the folder watcher ignores changes inside `_thumbnail`).
- **Cache location/format unchanged**: the existing sibling `_thumbnail` cache (location, JPG format, two size targets) is unchanged; this feature changes only *whether*, *which size*, *when*, and *how deep* thumbnails are generated, plus whether they are used for display and whether nested folders are shown.
- **Fallback on miss**: when a size's toggle is on but its cached thumbnail isn't ready yet, the UI shows the original for that size until the thumbnail exists.
- **Out of scope**: cache eviction/cleanup/size limits; a manual "regenerate cache" action; changing thumbnail dimensions or the cache folder; per-folder (rather than global) settings.
- Settings live in the application's existing settings store and settings menu alongside the current options.
