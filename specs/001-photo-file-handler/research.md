# Phase 0 Research: Unified Photo File Handler

All unknowns from the Technical Context are resolved below. Findings are grounded in the
current code (`src-tauri/src/metadata.rs`, `xmp.rs`, `lib.rs`) and the pinned `little_exif`
fork (rev `ba71e6a`, v0.6.23) under `~/.cargo/git/checkouts/little_exif-*/ba71e6a/`.

## 1. Streaming metadata I/O (Seek + Read, no whole-file load)

- **Decision**: Read metadata with `little_exif::metadata::Metadata::new_from_path(&Path)` and
  write with `Metadata::write_to_file(&Path)`. Stop using `new_from_vec` / `write_to_vec`.
- **Rationale**: `new_from_path` dispatches to `jpg::file_read_metadata` / `png::file_read_metadata` /
  `webp::file::read_metadata` (plus `file_read_iptc` / `file_read_xmp`), which open the file via
  `open_read_file` and walk segments/chunks with `Seek(SeekFrom::Current(..))`, reading only the
  metadata regions and seeking past image data. This satisfies FR-008 (incremental `Seek + Read`,
  no whole-file load) and keeps memory independent of file size (SC-004).
- **Alternatives considered**:
  - `new_from_vec(&fs::read(path))` — current `metadata.rs` approach; **rejected**: loads the entire
    file into a `Vec`, violating the streaming constraint.
  - Hand-rolled per-format readers (as `xmp.rs` does for XMP) — **rejected**: duplicates what the fork
    already provides for all three blocks; more code, more risk.
- **Note on writes**: `write_to_file` necessarily reads and rewrites the container to splice metadata.
  The "no whole-file read" rule targets reads/scans; a rewrite on save is inherent and acceptable, and
  pixel data is preserved (FR-006).

## 2. Conflict precedence on merge — EXIF > IPTC > XMP

- **Decision**: When the same logical field is populated in more than one block, the earlier block in
  read order wins: EXIF first, then IPTC, then XMP. Empty/absent values never override a populated one.
- **Rationale**: Confirmed in the 2026-06-20 clarification. Implement as an ordered `Option::or` chain
  starting from EXIF. This is a behavior change from the current `metadata.rs`, which resolves
  title/description IPTC-first.
- **Alternatives considered**: XMP-first (modern-authoritative) and first-non-empty — **rejected** per
  the clarification.
- **Keywords**: treated as a set union across all blocks (dedup, order-preserving), not a single-value
  conflict; precedence is therefore not applied to keywords.

## 3. Removing cleared fields from every block

- **Decision**: On each save, rebuild the managed fields in all blocks from the current `Metadata`.
  For a field that is now empty: EXIF — `Metadata::remove_tag` / `remove_tag_by_hex_group`; IPTC —
  rebuild the record without the dataset (or `clear_iptc` when no managed IPTC content remains); XMP —
  rebuild the packet omitting the empty element (or `clear_xmp` when nothing remains).
- **Rationale**: Q3 clarification requires cleared fields to disappear from EXIF/IPTC/XMP (SC-008,
  FR-015). The fork exposes `set_tag`, `remove_tag`, `remove_tag_by_hex_group`, `set_iptc`,
  `clear_iptc`, `set_xmp`, `clear_xmp`, confirmed in `metadata/set.rs` and `iptc/mod.rs`.
- **Alternatives considered**: leaving stale values (current EXIF write always sets tags, even empty) —
  **rejected**; writing empty placeholders — **rejected** (keeps junk tags).

## 4. In-process image decode (RGBA), no IPC

- **Decision**: Add `Photo::decode_image()` returning an in-memory RGBA buffer via the `image` crate
  (`image = { version = "0.25", features = ["jpeg","png","webp"] }`, already a dependency). It is a
  backend-only capability and MUST NOT be exposed as a Tauri command or sent over IPC.
- **Rationale**: Q4 clarification — decode to RGBA in Rust, not transferred to the frontend, reserved
  for future features (FR-009). The current viewer keeps its existing display path (asset protocol),
  so no UI change is needed.
- **Alternatives considered**: returning raw bytes or a downscaled preview over IPC — **rejected** by
  the user; both imply IPC transfer which is explicitly out of scope.

## 5. Logical-field ↔ block-tag mapping (resolves a deferred clarify item)

- **Decision**: Use the mapping table in [data-model.md](./data-model.md). Highlights:
  - `category` is written to XMP only (EXIF has no category tag; IPTC 2:15 is a constrained 3-char code,
    unsuitable for free-text stock categories) — a format/block that cannot carry it is skipped (FR-011).
  - `keywords` map to EXIF `XPKeywords` (semicolon-joined), IPTC `2:25` (repeatable), XMP `dc:subject` (Bag).
- **Rationale**: Mirrors and tidies the existing `metadata.rs` tag usage; keeps write redundant across
  blocks (FR-005) while honoring per-format support.

## 6. File-watcher interaction on save (resolves a deferred clarify item)

- **Decision**: Keep the current model — saving rewrites the file in place via `write_to_file`; the
  existing `notify` watcher + frontend debounce already tolerate self-writes. No watcher suppression is
  added in this feature; revisit only if a reload loop is observed.
- **Rationale**: Low risk; the app already saves files today without a watcher loop. Avoids scope creep.

## 7. Retiring the legacy XMP path

- **Decision**: After the `Photo` module is wired into `read_metadata` / `save_metadata` and tests pass,
  remove `xmp.rs` and the `img-parts` dependency. Retain `quick-xml` (XMP build/parse moves into
  `photo/xmp.rs`).
- **Rationale**: Principle X (smallest justified dependency set) and avoiding two parallel metadata paths.
  `img-parts` is used only by `xmp.rs`; once that is gone the dependency is dead.
- **Alternatives considered**: keeping `xmp.rs` fast XMP-only reader as an optimization — **rejected**;
  the merged read must consult all blocks anyway, so an XMP-only fast path no longer fits.
