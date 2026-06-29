# Feature Specification: SQLite Intermediate Metadata Store

**Feature Branch**: `008-sqlite-metadata-store`

**Created**: 2026-06-29

**Status**: Draft

**Input**: User description: "SQLite as intermediate metadata storage. docs/SQLite.puml describes the read-flow. For photo identity store two identifiers: primary key = photo path, fingerprint = size + mtime + fast full-file hash (xxHash). Add a file status meaning 'saved in the app database but not yet written into the photo file'. Add a Cancel button between the Ollama button and the Save button that reverts the values to match the file."

## Clarifications

### Session 2026-06-29

- Q: Where do manual field edits (and autosave) go before an explicit Save to file? → A: Immediately to the store — any value typed into a field is saved to the database at once (app-only); the photo file is untouched until Save.
- Q: How is an unchanged file detected on open, given full-file hashing cost? → A: Always compute the full-file xxHash (no short-circuit). *(Superseded by the analyze-review refinement below: the hash alone is authoritative; an mtime-only difference is silently refreshed, not a change.)*
- Q: Which storage engine, given the Pure-Rust constitution vs SQLite? → A: SQLite via the `rusqlite` crate with the `bundled` feature, accepted as a documented exception to Constitution Principle I (to be justified in the plan).
- Q: What happens to store records for deleted/missing files? → A: No automatic cleanup; records are kept. A manual "clean up database" action may be added later (out of scope now).

### Session 2026-06-29 (analyze review)

- Q: On an mtime-only change with identical content (full-file hash matches), prompt or accept silently? → A: The full-file hash is authoritative — a hash match means the content is unchanged; silently refresh the stored mtime and load from the store, no prompt. Only a hash difference counts as a change. (Refines the earlier "all three must match" answer.)
- Q: `releaseFilename` has no file-side equivalent — clear it when resolving a conflict to "file" or on Cancel, or keep it? → A: Keep the store's `releaseFilename`; the file side never overwrites it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Edits and attribution are kept by the app without rewriting the photo (Priority: P1)

A user opens a photo, runs Ollama attribution (or edits fields by hand), and the
produced metadata is remembered by the application immediately — without modifying the
photo file on disk. When the user re-opens that photo later (even after restarting the
app), the saved metadata is shown instantly, and a clear status indicates the metadata
lives in the app but has not yet been written into the photo file.

**Why this priority**: This is the core value of the feature — an intermediate store
that protects work (especially expensive Ollama attribution) from being lost and avoids
rewriting every photo file on every change. It is independently demonstrable and is the
foundation the other stories build on.

**Independent Test**: Open a photo, run attribution (or edit fields), confirm the photo
file on disk is unchanged, restart the app, re-open the same photo, and confirm the
previously produced metadata is shown with the "saved in app" status.

**Acceptance Scenarios**:

1. **Given** a photo with no app record, **When** the user opens it, **Then** the system
   reads metadata from the file and creates an app record for it.
2. **Given** an opened photo, **When** the user runs Ollama attribution, **Then** the
   results are stored in the app record, the photo file is left unmodified, and the file
   status shows "saved in app".
3. **Given** a photo whose metadata is already in the app store and whose file is
   unchanged, **When** the user re-opens it, **Then** the metadata is loaded from the
   store without re-reading the file's embedded metadata.
4. **Given** attribution results stored only in the app, **When** the app is closed and
   reopened, **Then** the stored metadata is still present for that photo.

---

### User Story 2 - Commit to file, or revert to file (Priority: P2)

When satisfied, the user writes the app-held metadata into the photo file with the Save
button; the app then records that the file and the store are in sync. If instead the user
wants to discard the app-held changes, a Cancel button (placed between the Ollama button
and the Save button) restores the values from the photo file and brings the store back in
line with the file.

**Why this priority**: Turns the intermediate store into a complete round trip — the user
can either promote app changes into the file or abandon them. Without it the store would
accumulate changes with no way to commit or undo them at the file level.

**Independent Test**: With a photo in the "saved in app" state, click Save and confirm the
photo file now contains the metadata and the status becomes synced; on another such photo
click Cancel and confirm the fields and the store both return to the file's current
metadata.

**Acceptance Scenarios**:

1. **Given** a photo in the "saved in app" state, **When** the user clicks Save, **Then**
   the metadata is written into the photo file and the store record is marked in sync with
   the file.
2. **Given** a photo in the "saved in app" state, **When** the user clicks Cancel, **Then**
   the working fields and the store record are restored to the photo file's current
   metadata and the status becomes synced.
3. **Given** a photo with no app-only changes, **When** the user views the footer controls,
   **Then** the Cancel control is unavailable (nothing to revert).
4. **Given** the user has committed metadata to the file, **When** the photo is re-opened,
   **Then** the file's current state and the store agree and no "saved in app" status is
   shown.

---

### User Story 3 - Detect and resolve external file changes (Priority: P3)

If a photo file is modified outside the application between sessions, the app detects that
the file no longer matches what the store last saw. When the app cannot safely decide which
version to keep, it asks the user whether to keep the metadata from the app store or the
metadata from the photo file, and applies the choice consistently.

**Why this priority**: Protects against silent data loss when files are edited by other
tools. It is valuable but only relevant in the less-common case of out-of-band edits, so it
ships after the core store and the save/cancel round trip.

**Independent Test**: Put a photo into the store, modify the same file with an external tool
so its contents differ, re-open it in the app, and confirm the app detects the mismatch and
prompts (or auto-resolves per the defined rule), then verify the chosen source is loaded and
the store fingerprint is updated.

**Acceptance Scenarios**:

1. **Given** a stored photo whose file is unchanged since the last sync, **When** it is
   opened, **Then** the store metadata is used and no prompt appears.
2. **Given** a stored photo with pending app-only changes whose file was also changed
   externally, **When** it is opened, **Then** the app store version is used (app data is
   treated as newer) per the read-flow.
3. **Given** a stored photo that was in sync but whose file was changed externally, **When**
   it is opened, **Then** the user is asked to choose the source; choosing "file" reads the
   file and updates the store, choosing "store" keeps the store metadata and refreshes the
   fingerprint.

---

### Edge Cases

- **File content identical but mtime changed** (e.g., touched or copied): the full-file hash
  still matches, so the photo is treated as unchanged — the stored mtime is silently refreshed,
  the store metadata loads, and no prompt is shown (FR-009).
- **Photo file deleted** while a store record exists: the store record is retained but the
  photo is unavailable to open; the store is not silently purged (no automatic cleanup — see
  Assumptions).
- **Photo moved or renamed** (path changes, content identical): a new record is keyed by the
  new path; recognizing it as the same content via fingerprint to re-link the old record is
  out of scope for this feature (see Assumptions).
- **Store unavailable or corrupt**: the application MUST keep working by reading and writing
  the photo files directly, and MUST log the failure.
- **Batch read with conflicts**: when several selected photos have external-change conflicts,
  the user MUST be able to resolve them with a single apply-to-all choice rather than one
  prompt per file.
- **Two different photos with identical content** (same hash, different paths): each keeps
  its own record keyed by path; the shared hash causes no collision.
- **Very large photo files**: computing the full-file fast hash on open must remain
  responsive (the fast-hash algorithm is chosen for this).

## Requirements *(mandatory)*

### Functional Requirements

**Store & identity**

- **FR-001**: The system MUST maintain an application-level metadata store, separate from the
  photo files, that persists across application restarts.
- **FR-002**: Each photo MUST be identified in the store primarily by its file path.
- **FR-003**: Each record MUST carry a fingerprint composed of the file size, the file
  modification time, and a fast hash of the whole file content.
- **FR-004**: Each record MUST store the editable metadata fields (title, description, keywords,
  categories) plus the store-only fields that have no file-side equivalent: release filename and
  the attribution flags (editorial, mature content, illustration). The store-only fields are kept
  in the store and never written to the file.
- **FR-005**: Each record MUST track whether its metadata is currently in sync with the photo
  file or has app-only changes not yet written to the file.

**Read flow (per docs/SQLite.puml)**

- **FR-006**: On opening a photo, the system MUST look up a store record by path.
- **FR-007**: When no record exists, the system MUST read metadata from the file and create a
  store record with the file's current fingerprint.
- **FR-008**: When a record exists, the system MUST compute the file's current fingerprint —
  always including a full-file xxHash — and compare it against the stored fingerprint, treating
  the **full-file hash as authoritative** for content identity.
- **FR-009**: The file is considered unchanged when the **full-file hash matches** the stored
  hash (content identical); a differing modification time alone is NOT a change — the system MUST
  silently refresh the stored mtime and load metadata from the store, MUST NOT parse the file's
  embedded metadata, and MUST NOT prompt. Only a **hash difference** enters the read-flow's
  mismatch branch (FR-010/FR-011).
- **FR-010**: When the fingerprints differ but the store holds app-only changes (store treated
  as newer), the system MUST load metadata from the store.
- **FR-011**: When the fingerprints differ and the store does not hold app-only changes
  (external modification), the system MUST ask the user whether to keep the store metadata or
  the file metadata.
- **FR-012**: When the user chooses the store, the system MUST refresh the stored fingerprint
  and load metadata from the store; when the user chooses the file, the system MUST read
  metadata from the file and update the store record to match, EXCEPT it MUST retain the store's
  `releaseFilename` (which has no file-side equivalent).

**Attribution & status**

- **FR-013**: Editing a metadata field by hand or running Ollama attribution (single or batch)
  MUST persist the change to the store immediately (app-only) and MUST NOT modify the photo
  file.
- **FR-014**: The system MUST present a distinct file status indicating "metadata saved in the
  app but not yet written into the photo file", shown when the record has app-only changes.
- **FR-015**: The new status MUST coexist with the existing file statuses (no file / viewing /
  edited / batch) without changing their meaning.

**Save & Cancel**

- **FR-016**: A Save action MUST write the current metadata into the photo file and then update
  the store record (refresh its fingerprint and mark it in sync with the file).
- **FR-017**: A Cancel control MUST be presented between the Ollama (attribute) button and the
  Save button in single-photo editing.
- **FR-018**: The Cancel action MUST restore the working fields from the photo file's current
  metadata and update the store record to mirror the file (discarding app-only changes), EXCEPT
  the store's `releaseFilename` MUST be retained (it has no file-side equivalent).
- **FR-019**: The Cancel control MUST be unavailable when there is nothing to revert (the record
  is already in sync with the file and there are no in-memory edits).

**Robustness, batch, logging**

- **FR-020**: Batch reads MUST use the same store-first read flow; when multiple opened photos
  have external-change conflicts, the system MUST allow resolving them with one apply-to-all
  choice.
- **FR-021**: If the store cannot be opened or a store operation fails, the system MUST fall
  back to reading/writing the photo files directly so the app remains usable.
- **FR-022**: All store operations and every store error MUST be logged.

### Key Entities *(include if feature involves data)*

- **Photo metadata record**: the app's stored copy of one photo's metadata. Identified
  primarily by file path. Holds the editable metadata fields, the fingerprint, a sync state
  (in sync with file vs. app-only changes), and timestamps for creation and last update.
- **Fingerprint**: the identity-of-content for a photo file at a point in time — file size,
  file modification time, and a fast full-file content hash. Used to decide whether a stored
  record still matches the file on disk.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Re-opening a photo whose stored metadata matches its unchanged file displays the
  metadata in under 200 ms; the file is hashed for the change check, but its embedded metadata
  is not re-parsed.
- **SC-002**: After running attribution on any number of photos and then closing and reopening
  the app without saving to file, 100% of the produced metadata is still present.
- **SC-003**: Running attribution writes zero bytes to the photo files until the user explicitly
  saves; verified by the files' content being byte-identical before and after attribution.
- **SC-004**: When a stored photo's file is modified outside the app, the app detects the change
  on open in 100% of cases (never silently shows stale metadata as if in sync).
- **SC-005**: The user can tell, at a glance, whether the currently open photo's metadata is
  only in the app or already written to the file, in 100% of states.
- **SC-006**: Cancel returns the open photo's fields and its stored record to exactly the file's
  current metadata in 100% of cases.

## Assumptions

- **Read triggers on open, not on folder scan**: the store-first read flow runs when a photo is
  opened in the editor (single) or selected for batch editing, not for every file during a
  folder/thumbnail scan, so conflict prompts cannot flood the user on folder open.
- **"Store is newer" definition**: the store is treated as newer than the file when the record
  has app-only changes (it was modified by the app after the last successful file write). In
  that state a fingerprint mismatch is resolved in favor of the store without prompting, per the
  read-flow diagram — including the rare case where the file was also changed externally.
- **Metadata field set**: the store holds the file-backed fields (title, description, keywords,
  categories) plus store-only fields with no file equivalent — release filename and the three
  attribution flags (editorial, mature content, illustration). The store-only fields are retained
  when resolving from the file and on Cancel/revert; the file pipeline never reads or writes them.
- **Fast hash**: the full-file content hash is a non-cryptographic fast hash (xxHash), computed
  over the whole file on every open and used only for change detection, not for security.
- **Storage engine (constitution exception)**: the store is SQLite accessed via the `rusqlite`
  crate with the `bundled` feature (SQLite C code compiled into the application). This is an
  explicit, documented exception to Constitution Principle I (Pure Rust Backend) and MUST be
  justified in the plan's Complexity Tracking.
- **No automatic record cleanup**: store records for deleted or missing files are kept; a manual
  "clean up database" action may be added in a later feature and is out of scope here.
- **Cancel scope**: the Cancel ("revert to file") control applies to single-photo editing; batch
  editing keeps its existing controls, and batch conflicts are handled by the apply-to-all
  resolution (FR-020).
- **Move/rename re-linking is out of scope**: recognizing a moved/renamed file as the same
  content (via fingerprint) and transferring its old record to the new path is deferred; a new
  path produces a new record.
- **Single-user, local store**: the store is a local, single-application database; concurrent
  access from multiple app instances is not a target scenario.
- **Existing read/write paths are reused**: the feature reuses the app's current file metadata
  read and write capabilities as the file-side source of truth, layering the store in front of
  them.
