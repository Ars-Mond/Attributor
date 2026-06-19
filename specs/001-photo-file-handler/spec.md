# Feature Specification: Unified Photo File Handler

**Feature Branch**: `001-photo-file-handler`

**Created**: 2026-06-20

**Status**: Draft

**Input**: User description: "Create a dedicated photo abstraction in the project that reads a photo, reads metadata, edits that metadata, saves metadata, and reads the image content (as a separate function). All reading must use Seek + Read; reading the whole file at once is discouraged and not allowed. Metadata is read in the order EXIF -> IPTC/IPTC-IIM -> XMP/iTXt, fully from every available block, and merged together. There is no write order: metadata is written into every possible area (EXIF, IPTC, XMP), duplicating across them. After creation, integrate this abstraction into the project."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read and merge metadata from every block (Priority: P1)

When a photo is opened, the application reads metadata from all standard locations
inside the file — EXIF first, then IPTC/IPTC-IIM, then XMP/iTXt — and presents a
single, combined view of the fields (title, description, keywords, category). The
user sees existing metadata regardless of which tool or agency stored it, and
regardless of which block it lives in.

**Why this priority**: Reading is the foundation of the whole feature; without a
correct, merged read the user cannot trust or edit anything. It delivers immediate
standalone value: surfacing metadata that the current XMP-only path silently misses.

**Independent Test**: Open a photo whose fields are split across different blocks
(e.g. description only in EXIF, keywords only in IPTC, title only in XMP) and
confirm the combined result contains every present field with the expected
precedence applied to conflicts.

**Acceptance Scenarios**:

1. **Given** a photo with metadata present in EXIF, IPTC, and XMP, **When** it is opened, **Then** the merged result contains the union of all populated fields.
2. **Given** a photo where the same field has different values in two blocks, **When** it is opened, **Then** the value from the later block in read order (XMP over IPTC over EXIF) is shown.
3. **Given** a photo with metadata in only one block, **When** it is opened, **Then** those fields are read and the others are empty.
4. **Given** a photo with no metadata at all, **When** it is opened, **Then** an empty field set is returned without error.

---

### User Story 2 - Save metadata into every block, duplicated (Priority: P2)

When the user saves, the application writes the edited fields into every metadata
area the file format supports — EXIF, IPTC, and XMP — storing the same logical
values in each. Stock agencies and downstream tools then find the metadata no
matter which block they read, while the image itself is left untouched.

**Why this priority**: Writing to all blocks is the core differentiator of the
feature and the reason stock submissions are accepted; it depends on a working read
(P1) to round-trip values.

**Independent Test**: Edit the fields on a photo, save, then reopen the file with
independent tools and confirm the same values appear in EXIF, IPTC, and XMP, and
that the image pixels are unchanged.

**Acceptance Scenarios**:

1. **Given** edited fields, **When** the photo is saved, **Then** the values are present in EXIF, IPTC, and XMP for formats that support those blocks.
2. **Given** a saved photo, **When** it is reopened, **Then** the previously edited values are read back identically.
3. **Given** a save operation, **When** it completes, **Then** the image pixel data is byte-for-byte identical to before the save.
4. **Given** a file that already contains unrelated metadata tags, **When** it is saved, **Then** those unrelated tags are preserved.
5. **Given** a format that does not support a particular block, **When** the photo is saved, **Then** the unsupported block is skipped and the supported blocks are still written.

---

### User Story 3 - Read the image content as a separate operation (Priority: P3)

The application can load the decoded image content for preview through a function
that is independent of any metadata operation, so the viewer can display a photo
without reading or touching its metadata, and metadata work never forces an image
decode.

**Why this priority**: Preview is valuable but secondary to correct metadata
handling; separating it keeps each path fast and independently usable.

**Independent Test**: Call the image-read operation on a photo and confirm decoded
image content is returned, with no metadata read performed and the file left
unmodified.

**Acceptance Scenarios**:

1. **Given** a supported photo, **When** the image-read operation is called, **Then** decoded image content suitable for preview is returned.
2. **Given** the image-read operation, **When** it runs, **Then** no metadata is parsed and the file is not modified.

---

### Edge Cases

- A metadata block is missing, empty, truncated, or corrupt: it is skipped, the condition is logged, and reading continues with the remaining blocks.
- The same field appears in multiple blocks with conflicting values: precedence follows read order, with the later block (XMP > IPTC > EXIF) winning; empty or absent values never override a populated value.
- A photo carries metadata tags the application does not manage: those tags are preserved on save and not lost.
- A very large photo is opened or saved: memory use stays bounded because access is incremental, not whole-file.
- The file is read-only or the disk is full on save: the operation fails as a handled error (no crash) and the failure is logged.
- A format does not support a given block (e.g. IPTC-IIM in a format that lacks it): that block is skipped on write while supported blocks are still written.
- The same photo behaves identically on Windows, Linux, and macOS.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a single photo abstraction that, given a file path, exposes operations to read metadata, edit metadata in memory, save metadata, and read the image content.
- **FR-002**: System MUST read metadata from EXIF, then IPTC/IPTC-IIM, then XMP/iTXt, reading every available block and combining them into one unified metadata set.
- **FR-003**: When the same logical field exists in more than one block with differing values, system MUST resolve the conflict in read order so the later block wins (XMP over IPTC over EXIF); an absent or empty value MUST NOT override a populated one.
- **FR-004**: System MUST allow editing the unified metadata fields in memory, with no change reaching disk until a save is explicitly requested.
- **FR-005**: On save, system MUST write the metadata into every metadata block the file format supports (EXIF, IPTC, XMP), storing the same logical values in each; write order is not significant.
- **FR-006**: Saving metadata MUST preserve the image's pixel data exactly, with no re-encoding and no quality loss.
- **FR-007**: Saving metadata MUST preserve existing metadata tags that the application does not manage.
- **FR-008**: All file reading MUST use incremental Seek + Read access; the system MUST NOT load the entire file into memory at once.
- **FR-009**: System MUST provide a separate operation to read and decode the image content for preview, independent of metadata operations.
- **FR-010**: System MUST handle missing, empty, or corrupt metadata blocks gracefully — skipping them, logging the condition, and continuing — and MUST return an empty set (not an error) for a file with no metadata.
- **FR-011**: System MUST support JPEG, PNG, and WebP consistently; for a format that does not support a particular block, the system MUST skip that block (logged) and still write the supported blocks.
- **FR-012**: All operations MUST surface failures as handled errors without crashing, and MUST log concisely at each error site.
- **FR-013**: The application's existing metadata read/write path MUST be replaced by this unified photo abstraction, so the file tree, metadata panel, auto-save, batch operations, and the image viewer all use it.
- **FR-014**: All operations MUST behave identically across Windows, Linux, and macOS.

### Key Entities *(include if feature involves data)*

- **Photo File**: A single image file on disk (JPEG, PNG, or WebP). Owns its path and format and is the entry point for every operation (read metadata, edit, save, read image).
- **Unified Metadata**: The combined, editable set of the application's logical fields (title, description, keywords, category), formed by merging every metadata block on read and duplicated across every block on save.
- **Metadata Block**: A physical metadata location inside the file — EXIF, IPTC/IPTC-IIM, or XMP/iTXt — that is read in a defined order and written to redundantly.
- **Image Content**: The decoded picture suitable for preview, produced by a dedicated operation that is independent of metadata.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For a photo whose fields are spread across EXIF, IPTC, and XMP, 100% of populated fields appear in the merged result (no field is dropped).
- **SC-002**: After a save, 100% of the edited fields are present in every format-supported block, confirmed by independent tools.
- **SC-003**: The image pixel data is byte-for-byte identical before and after a metadata save in 100% of saves.
- **SC-004**: Reading or saving metadata of a 100 MB photo uses no more additional memory than doing so for a 5 MB photo (memory is independent of file size).
- **SC-005**: Metadata for a typical stock photo (50 MB or smaller) is read and displayed in under 500 ms on a typical machine.
- **SC-006**: Opening a photo that has no metadata, or whose blocks are partially corrupt, never produces an error or crash in 100% of such cases.
- **SC-007**: JPEG, PNG, and WebP each round-trip the edited fields successfully, and behave identically across Windows, Linux, and macOS.

## Assumptions

- **Conflict precedence**: Because the user described reading EXIF -> IPTC -> XMP and merging, conflicts are resolved with the later-read block winning (XMP > IPTC > EXIF). This is the most defensible default (XMP is the modern, authoritative block); it is flagged here for confirmation during clarification if the intended priority is the reverse.
- **Field scope**: The editable unified set stays the application's current logical fields (title, description, keywords, category). The read path gathers these from any block; exposing additional raw EXIF/IPTC tags in the UI is out of scope for this feature.
- **Image-read output**: The separate image-read operation returns decoded image content sufficient for the existing viewer; it does not add new viewing features.
- **Format/block support**: Metadata is written to whichever blocks each format supports; blocks a format cannot carry are skipped without error. JPEG, PNG, and WebP remain the supported set.
- **Surgical writes**: Saving rewrites only metadata regions and leaves image data and unrelated tags intact, consistent with the application's existing lossless approach.
- **Integration**: This abstraction replaces the current XMP-only read/write path; after integration, saving writes EXIF and IPTC in addition to XMP, which changes (expands) what the application stores compared to today.
- **Foundation**: The feature builds on the project's existing metadata capability for EXIF/IPTC/XMP and its established lossless binary read/write approach; no new external dependency is assumed beyond what the project already provides.
