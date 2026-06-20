---
description: "Task list for Unified Photo File Handler"
---

# Tasks: Unified Photo File Handler

**Input**: Design documents from `specs/001-photo-file-handler/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Included — the existing `src-tauri/tests/metadata_test.rs` exercises the module being
evolved, and `quickstart.md` enumerates validation scenarios. Tests extend that suite.

**Organization**: Tasks are grouped by user story (US1 read, US2 write, US3 decode) for independent
implementation and testing.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different file, no dependency on an incomplete task)
- **[Story]**: US1 / US2 / US3 (user-story phases only)

## Path Conventions

Backend Rust under `src-tauri/src/`; integration tests under `src-tauri/tests/`. The feature evolves
the existing `metadata.rs` into a new `photo/` module and retires the legacy `xmp.rs` path.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Scaffold the new module and test assets.

- [ ] T001 Create the `photo/` module skeleton (empty `Photo` and `Metadata` placeholders) and declare `mod photo;` in `src-tauri/src/lib.rs`, file `src-tauri/src/photo/mod.rs`
- [ ] T002 [P] Add minimal PNG and WebP test fixtures (`test_img.png`, `test_img.webp`) in `src-tauri/test_images/`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The shared abstraction skeleton every user story builds on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T003 Define the `Metadata` struct (`title`, `description`, `keywords`, `category`; `#[derive(Serialize, Default, Debug, PartialEq, Clone)]`, `#[serde(rename_all = "camelCase")]`) in `src-tauri/src/photo/mod.rs`
- [ ] T004 Implement `Photo::open(path)` with file-type detection via `little_exif` `get_file_type` / `FileExtension::auto_detect` (error on unsupported format, no file body loaded) in `src-tauri/src/photo/mod.rs`
- [ ] T005 [P] Move XMP packet build/parse helpers (`build_xmp_packet`, `build_xmp_body`, `parse_xmp_fields`) from `metadata.rs` into `src-tauri/src/photo/xmp.rs`
- [ ] T006 [P] Move shared utilities (`unique_keywords`, `split_semicolons`, `non_empty`) into `src-tauri/src/photo/mod.rs`

**Checkpoint**: `photo` module compiles with `open()`, `Metadata`, and XMP/utility helpers in place.

---

## Phase 3: User Story 1 - Merge read across EXIF/IPTC/XMP (Priority: P1) 🎯 MVP

**Goal**: Open a photo and return one merged `Metadata` from all blocks, EXIF-wins precedence, streaming.

**Independent Test**: Open a JPEG with values split across blocks → merged result is the union; on a
conflicting field the EXIF value wins; missing/no metadata yields empty fields without error.

- [ ] T007 [P] [US1] Add merge-read tests (union across blocks, EXIF>IPTC>XMP precedence, keyword union, missing-block, no-metadata) in `src-tauri/tests/metadata_test.rs`
- [ ] T008 [US1] Implement streaming EXIF/IPTC/XMP extraction via `little_exif::metadata::Metadata::new_from_path` (no `fs::read`/`new_from_vec`) in `src-tauri/src/photo/read.rs`
- [ ] T009 [US1] Implement EXIF-first merge precedence and keyword union per `data-model.md` in `src-tauri/src/photo/read.rs`
- [ ] T010 [US1] Implement `Photo::read_metadata()` delegating to `read.rs` (skip+log missing/corrupt blocks; empty on none) in `src-tauri/src/photo/mod.rs`
- [ ] T011 [US1] Switch the `read_metadata` Tauri command to `photo` and map `Metadata`→`ReadResult` (`category`→`categories`, `releaseFilename=""`) in `src-tauri/src/lib.rs`

**Checkpoint**: Reading metadata in the app now merges EXIF+IPTC+XMP and is independently testable.

---

## Phase 4: User Story 2 - Save into every block, duplicated (Priority: P2)

**Goal**: Save the four fields into EXIF, IPTC, and XMP (duplicated), remove cleared fields, preserve
pixels and unrelated tags.

**Independent Test**: Edit fields, save, reopen → values present in every format-supported block and
read back identically; a cleared field is absent from all blocks; image pixels byte-for-byte unchanged.

- [ ] T012 [P] [US2] Add write tests (round-trip across all blocks, empty-field removal, pixels preserved, unrelated tag preserved) in `src-tauri/tests/metadata_test.rs`
- [ ] T013 [US2] Implement writing each non-empty field to EXIF/IPTC/XMP duplicated, skipping (block,format) pairs a format cannot carry (IPTC=JPEG only; category=XMP only) in `src-tauri/src/photo/write.rs`
- [ ] T014 [US2] Implement cleared-field removal (`remove_tag`/`remove_tag_by_hex_group` for EXIF, `clear_iptc`/rebuild for IPTC, `clear_xmp`/rebuild for XMP) in `src-tauri/src/photo/write.rs`
- [ ] T015 [US2] Implement `Photo::save_metadata()` persisting via `little_exif` `write_to_file` (preserve pixels and unmanaged tags) in `src-tauri/src/photo/mod.rs`
- [ ] T016 [US2] Switch the `save_metadata` Tauri command to `photo`, map `SaveRequest`→`Metadata` (`categories`→`category`), and preserve the rename flow + `releaseFilename` passthrough in `src-tauri/src/lib.rs`

**Checkpoint**: Saving writes all blocks; US1 + US2 both work end-to-end in the app.

---

## Phase 5: User Story 3 - Decode image in-process (Priority: P3)

**Goal**: A backend-only RGBA decode independent of metadata, never crossing IPC.

**Independent Test**: Call decode on a supported photo → non-empty RGBA buffer returned; no metadata
parsed, file unmodified, nothing sent over IPC.

- [ ] T017 [P] [US3] Add a `decode_image` test (non-empty RGBA, file unchanged, no metadata parse) in `src-tauri/tests/metadata_test.rs`
- [ ] T018 [US3] Implement `Photo::decode_image()` returning `image::RgbaImage` via the `image` crate (no Tauri command, no IPC) in `src-tauri/src/photo/image.rs` and expose the method in `src-tauri/src/photo/mod.rs`

**Checkpoint**: All three user stories are independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Retire the legacy path, trim dependencies, finalize quality gates.

- [ ] T019 [P] Add PNG and WebP round-trip tests (XMP/EXIF written, IPTC skipped on unsupported formats) in `src-tauri/tests/metadata_test.rs`
- [ ] T020 Remove legacy `src-tauri/src/metadata.rs` and repoint the `photo_metadata` re-export to the `photo` module in `src-tauri/src/lib.rs`
- [ ] T021 Remove legacy `src-tauri/src/xmp.rs` and update/retire its `read_*_xmp_fast` re-exports and `src-tauri/tests/xmp_read.rs`
- [ ] T022 [P] Remove the now-dead `img-parts` dependency from `src-tauri/Cargo.toml`
- [ ] T023 [P] Logging audit across `src-tauri/src/photo/` — error-site logging present, concise English messages, no `println!`/`dbg!`
- [ ] T024 Run `cargo test` (all green) and validate the `quickstart.md` scenarios; run `npx svelte-check --tsconfig ./tsconfig.json` only if any frontend file was touched

---

## Dependencies & Execution Order

- **Setup (T001–T002)** → **Foundational (T003–T006)** → user stories.
- **US1 (T007–T011)** depends on Foundational; is the MVP.
- **US2 (T012–T016)** depends on Foundational; reuses `photo/xmp.rs` build helpers; independent of US1 logic but shares the module.
- **US3 (T017–T018)** depends on Foundational only.
- **Polish**: legacy removal **T020/T021 require T011 and T016** (commands switched off the old path first); **T022 requires T021** (`img-parts` only used by `xmp.rs`); **T024 is last**.
- Within a file, tasks are sequential: T008→T009 (`read.rs`), T013→T014 (`write.rs`).

## Parallel Opportunities

- T002 alongside Setup.
- T005 and T006 in parallel (different files) during Foundational.
- Each story's test task runs parallel to that story's implementation: T007 ∥ T008–T010; T012 ∥ T013–T015; T017 ∥ T018.
- In Polish, T019, T022, T023 can run in parallel (different files).

```text
# Foundational parallel batch:
Task: "T005 Move XMP build/parse into src-tauri/src/photo/xmp.rs"
Task: "T006 Move shared utilities into src-tauri/src/photo/mod.rs"
```

## Implementation Strategy

- **MVP** = Phase 1 + Phase 2 + **User Story 1** (merge read). Ship/validate, then add US2, then US3.
- **Incremental**: each story is a complete, independently testable increment; the legacy path stays
  live until US1 + US2 are switched over, then Polish retires it.
