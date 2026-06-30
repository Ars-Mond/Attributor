# Feature Specification: Batch Metadata Save & Unified Event Contract

**Feature Branch**: `004-batch-metadata-save`

**Created**: 2026-06-23

**Status**: Draft

**Input**: User description: "Replace the frontend per-file `handleBatchSave` loop with a single list sent to the backend; save the batch multithreaded in the backend; emit status events back to the frontend (saved / not saved / …). Fix the event design: unify the event *contract* (names + payload types), not the emission, into one place — emission stays in the owning domain; the delivery mechanism is chosen by signal nature (broadcast vs per-call), not 'one for all'. Also determine whether batch mode should use `tauri::ipc::Channel<T>`."

## Clarifications

### Session 2026-06-23

- Q: What does the frontend send to the batch command — fully-resolved per-file metadata, or an "intent" (shared fields + keyword edits) resolved by the backend? → A: Fully-resolved per-file metadata. The frontend resolves each file's final metadata from data already loaded for the batch and sends a list of resolved items; the backend writes and reads each file at most once only to preserve unrelated tags. Keyword-merge logic stays on the frontend.
- Q: Should an in-progress batch be cancellable in this version? → A: Yes. The user can cancel a running batch; not-yet-started photos are skipped, and each item's outcome distinguishes saved / failed / cancelled.
- Q: What degree of write parallelism? → A: `rayon` default (~all cores); the `rayon` pool bounds the thread count.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Save many photos at once, fast (Priority: P1)

A user selects multiple photos in the files panel, edits the shared metadata fields
(title, description, keywords, category), and saves the whole selection in one action.
Instead of the app writing the files one after another, it hands the entire list to the
backend, which writes them concurrently across CPU cores. For a large selection the user
waits a fraction of the time the current one-by-one flow takes.

**Why this priority**: This is the core value — batch editing today is serialized and slow;
making it concurrent is the headline improvement the user asked for and the MVP of the feature.

**Independent Test**: Select N photos, apply a common title/keywords, save once; verify all N
files contain the new metadata and that wall-clock time is substantially lower than the current
sequential flow on a multi-core machine.

**Acceptance Scenarios**:

1. **Given** a selection of many photos and edited shared fields, **When** the user saves, **Then** every selected photo is written with the resolved metadata and the operation completes in one backend round-trip.
2. **Given** a multi-core machine, **When** a sizable batch is saved, **Then** the writes run concurrently and total time scales down with available cores rather than growing linearly per file.
3. **Given** the same inputs, **When** a photo is saved via batch versus saved individually, **Then** the resulting metadata is identical (same multi-block write, same cleared-field removal).

---

### User Story 2 - See per-file outcome and live progress (Priority: P2)

While the batch runs, the user sees progress advance (e.g. "saved 12 of 40") and, at the end,
which files succeeded and which failed and why. A single problem file (locked, read-only, missing)
does not abort the rest — the batch continues and the failure is reported distinctly.

**Why this priority**: Without per-file feedback a long batch is an opaque freeze and a single bad
file silently breaks the run. Status reporting makes the concurrent batch trustworthy and debuggable.

**Independent Test**: Run a batch that mixes writable photos with one unwritable file; verify the
writable photos are all saved, the bad file is reported as failed with a reason, and progress
updates are observed incrementally rather than only at completion.

**Acceptance Scenarios**:

1. **Given** a running batch, **When** each photo finishes, **Then** the frontend receives an incremental progress signal and updates the UI without waiting for the whole batch.
2. **Given** a batch where one file cannot be written, **When** the batch runs, **Then** that file is reported as failed (with a reason) and all other files are still saved.
3. **Given** a finished batch, **When** the user inspects the result, **Then** every item in the batch has a definitive outcome (saved, failed, or cancelled) and none are silently dropped.
4. **Given** a running batch, **When** the user cancels it, **Then** photos not yet started are not written and the result distinguishes saved, failed, and cancelled items.

---

### User Story 3 - Reliable, drift-proof event contract (Priority: P3)

As a maintainer, every backend→frontend event has its name and payload defined once, in a single
shared contract that both sides consume. Emission still lives in the module that owns the signal,
but the *types* are no longer hand-duplicated across Rust and TypeScript, so an event can't silently
drift out of sync. The delivery mechanism is chosen per signal: ambient broadcasts stay broadcasts;
progress tied to one operation flows over a channel scoped to that operation.

**Why this priority**: It is the supporting infrastructure that makes US2 sound and removes a class
of integration bugs across the app's existing events; valuable but not user-visible on its own.

**Independent Test**: Verify the existing `folder-changed` and `thumbnail-ready` events still work
after migrating onto the unified contract, and that changing an event's payload surfaces as a
build/type error on the side left out of sync rather than a silent runtime mismatch.

**Acceptance Scenarios**:

1. **Given** the unified contract, **When** an event's payload shape changes, **Then** the mismatched side fails to build/type-check instead of failing silently at runtime.
2. **Given** the migrated events, **When** the app runs, **Then** `folder-changed` and `thumbnail-ready` behave exactly as before.
3. **Given** a new event is added, **When** a developer defines it, **Then** its name and payload exist in exactly one shared place reused by both sides.

---

### Edge Cases

- A file in the batch is read-only, locked by another process, or deleted between selection and save → reported as failed with a reason; the rest of the batch still completes.
- Shared fields are cleared (emptied) and applied across the batch → each file has those fields removed (consistent with single-file clearing).
- A very large batch (hundreds–thousands of files) → concurrency stays bounded (no thread explosion), memory stays bounded, and progress keeps flowing.
- Mixed formats (JPEG/PNG/WebP) in one batch → each is written through its correct path.
- Duplicate paths appear in the list → handled without double-writing or corrupting the file.
- A batch is started while the thumbnail pipeline from a recent folder-open is still running → both complete; the batch must not deadlock or be starved indefinitely.
- The user navigates away, deselects, or closes the window mid-batch → the in-flight backend work resolves safely and stale progress is not applied to unrelated UI state.
- The user cancels mid-batch → photos not yet started are skipped and reported as cancelled; a photo already being written either completes or fails but is reported accurately; the operation ends promptly rather than running to completion.

## Requirements *(mandatory)*

### Functional Requirements

#### Batch save

- **FR-001**: The system MUST accept a batch save as a single backend request carrying every photo and its resolved target metadata, crossing the IPC boundary once for the whole batch (not once per file).
- **FR-002**: The system MUST perform the batch's metadata writes concurrently across multiple CPU threads.
- **FR-003**: The system MUST process the batch best-effort: a failure writing one photo MUST NOT abort the remaining photos.
- **FR-004**: The system MUST report a definitive per-photo outcome (succeeded, or failed with a human-readable reason) for every item in the batch.
- **FR-005**: The system MUST report incremental progress as the batch proceeds (each completion observable before the whole batch finishes), not only a single result at the end.
- **FR-006**: The frontend MUST resolve each photo's final metadata (shared fields merged with that file's own keywords) from data already loaded for the batch, and carry the resolved per-photo metadata in the request, so no extra per-file round-trip to the frontend is needed during the save.
- **FR-007**: The backend MUST read each file's existing metadata at most once during a batch save (only to preserve unrelated tags); the frontend MUST NOT re-read files it already loaded for the batch when building the request.
- **FR-008**: A metadata-only batch save MUST NOT trigger a full-folder rescan or a thumbnail pipeline restart for the files it writes (no watcher-induced churn during or immediately after the batch).
- **FR-009**: The frontend MUST replace the existing sequential per-file save loop (`handleBatchSave`) with the single batched backend call.
- **FR-010**: For each photo, the batch write result MUST be identical to saving that photo individually today (same multi-block write across EXIF/IPTC/XMP, same removal of cleared fields).
- **FR-011**: While a batch runs, the UI MUST remain responsive (input usable, no frozen frames) and reflect progress.

#### Event contract

- **FR-012**: Event names and their payload shapes MUST be defined in a single shared contract that is the source of truth for both backend and frontend; ad-hoc duplicated string literals and hand-mirrored payload types MUST be eliminated.
- **FR-013**: Event *emission* MUST remain in the domain module that owns the signal; the shared contract centralizes names and types only, not the act of emitting.
- **FR-014**: The delivery mechanism MUST be chosen by signal nature: ambient/broadcast signals are delivered as broadcast events to listeners; progress tied to a single operation is delivered over a channel scoped to that one operation.
- **FR-015**: The existing `folder-changed` and `thumbnail-ready` events MUST be migrated onto the unified contract with no change in observable behavior.
- **FR-016**: A change to any event's name or payload MUST surface as a build/type error on whichever side is out of sync, preventing silent drift.

#### Cancellation

- **FR-017**: The user MUST be able to cancel an in-progress batch; after cancellation the system MUST stop dispatching photos that have not yet started.
- **FR-018**: After a cancellation, every photo MUST have a definitive outcome — saved, failed, or cancelled (not started) — with no item left ambiguous; a photo already mid-write MAY complete but MUST be reported accurately.

### Key Entities *(include if feature involves data)*

- **Batch Save Request**: The whole operation's input — an ordered list of items, each item being a photo's path plus its resolved target metadata (title, description, keywords, category) and target filename. Built on the frontend from the current selection and the shared-field edits.
- **Per-File Save Result**: The outcome for one photo — its path, a status (saved, failed, or cancelled/not-started), and, on failure, a reason. Every request item produces exactly one result.
- **Batch Progress Update**: An incremental signal carrying how many items are complete out of the total (and optionally the latest per-file result), delivered while the batch runs.
- **Event Contract Catalog**: The single shared definition mapping each event name to its payload type, consumed by both backend and frontend.
- **Signal Kind**: The classification that decides delivery — a broadcast signal (ambient, of interest to any listener) versus a per-operation progress signal (scoped to one invocation).
- **Cancellation Signal**: A user-initiated request to stop the running batch; it prevents dispatch of not-yet-started photos and drives the cancelled outcomes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a 4-core (or better) machine, saving a batch of 50 photos completes at least 3× faster than the current one-by-one flow for the same selection.
- **SC-002**: 100% of items in a batch produce a definitive outcome (saved, failed, or cancelled); none are silently dropped.
- **SC-003**: In a batch mixing writable files with at least one unwritable file, every writable file is saved successfully and each unwritable file is reported as failed with a reason.
- **SC-004**: Progress is observed advancing during the batch (at least one update before the final item completes), not as a single jump at the end.
- **SC-005**: A metadata-only batch save causes 0 thumbnail regenerations and does not reload/replace the folder tree for the unchanged images.
- **SC-006**: For every photo, the metadata fields written by batch are identical to those written by saving the photo individually.
- **SC-007**: The event contract is defined exactly once; an intentional payload change produces a build/type-check failure on the mismatched side (0 silent drift).
- **SC-008**: The UI stays responsive throughout a batch of at least 50 photos (no unresponsive/frozen window).
- **SC-009**: When the user cancels a running batch, no photo that had not yet started is written, and the batch stops promptly rather than running every remaining file to completion.

## Assumptions

- **Error policy**: Batch saving is best-effort — a per-file failure is reported and the batch continues. Chosen because the user explicitly wants per-file "saved / not saved" status.
- **Concurrency**: The batch uses a CPU-bounded thread pool (`rayon`, per Constitution §VIII "batch photo processing within the app's logic uses `rayon`"), with parallelism roughly matching available cores; it does not spawn one unbounded thread per file.
- **Channel decision (resolves the user's question)**: Per-batch progress and per-file results are delivered over a **per-invocation typed channel** (`tauri::ipc::Channel<T>`), because that signal is scoped to one operation, is naturally ordered, and is cleaned up when the call ends — a better fit than a global broadcast. The existing ambient signals (`folder-changed`, `thumbnail-ready`) remain **global broadcast events**. This is the concrete application of FR-014.
- **In-place writes only**: Batch writes each file in place under its own name (no rename during batch), matching current behavior; renaming remains a single-file concern.
- **Out of scope — per-file write-path micro-optimization**: The dominant per-file backend cost (the `little_exif` whole-file entropy-coded scan on the write path, which reads avoid via a header prefix) is a separate per-photo concern. This feature mitigates total wall-clock through concurrency; optimizing the single-file write path is a candidate follow-up, not part of this feature.
- **Cancellation (in scope)**: The user can cancel a running batch; not-yet-started photos are skipped and reported as cancelled (resolved in clarification). The cancel signal is delivered out-of-band from the progress channel (mechanism deferred to the plan).
- **No persistence**: Consistent with the project, no database or stored batch state is introduced; the batch is a transient operation.
- **Single-file save unchanged**: The existing single-file save command and its semantics remain available and unchanged.
