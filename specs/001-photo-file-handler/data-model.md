# Phase 1 Data Model: Unified Photo File Handler

## Entities

### Photo

The abstraction ("class") for one image file. Owns the path; performs all operations.

| Field | Type | Notes |
|-------|------|-------|
| `path` | `PathBuf` | Absolute path to the image file |
| `file_type` | `little_exif::filetype::FileExtension` | Detected from extension/content |

Lifecycle: `open` → (`read_metadata` and/or `set_metadata`) → `save`; `decode_image` is independent
and read-only. No persistent in-memory copy of the file is held between calls (streaming reads).

### Metadata

The editable, merged set of the four logical fields. Already exists in `metadata.rs`; moves to `photo`.

| Field | Type | Validation / Notes |
|-------|------|--------------------|
| `title` | `String` | Free text; empty means "remove from all blocks" on save |
| `description` | `String` | Free text; empty means "remove" |
| `keywords` | `Vec<String>` | Deduplicated, trimmed, order-preserving; empty list means "remove" |
| `category` | `String` | Free text; empty means "remove"; persisted to XMP only |

`#[derive(Serialize, Default, Debug, PartialEq)]` with `#[serde(rename_all = "camelCase")]`.

### MetadataBlock (concept, not a struct)

The three physical locations, read in order and written redundantly: `EXIF`, `IPTC` (IPTC-IIM, JPEG),
`XMP` (APP1 / iTXt / RIFF `XMP `).

### ImageContent

In-memory decoded pixels from `decode_image`: an `image::RgbaImage` (or `(width, height, Vec<u8>)`).
Backend-only; never serialized over IPC.

## Logical field ↔ block-tag mapping

On **read**, each logical field is resolved with EXIF > IPTC > XMP precedence (keywords = union).
On **write**, each non-empty field is written to every listed block its format supports; empty fields
are removed from every block.

| Logical field | EXIF | IPTC (record 2) | XMP |
|---------------|------|-----------------|-----|
| `title` | `XPTitle` (0x9C9B) | ObjectName (5), Headline (105) | `dc:title` (x-default), `photoshop:Headline` |
| `description` | `ImageDescription` (0x010E), `XPSubject` (0x9C9F) | Caption/Abstract (120) | `dc:description` (x-default) |
| `keywords` | `XPKeywords` (0x9C9E, `;`-joined) | Keywords (25, repeatable) | `dc:subject` (`rdf:Bag`) |
| `category` | — (none) | — (2:15 unsuitable) | `photoshop:Category` |

Per-format block support (FR-011): IPTC writes apply to **JPEG** only; XMP applies to JPEG/PNG/WebP;
EXIF applies to all three. Unsupported (block, format) pairs are skipped and logged, not errored.

## Read merge precedence (resolution order)

```text
title       = exif_xp_title → iptc_object_name → iptc_headline → xmp_title → xmp_headline
description = exif_image_desc → exif_xp_subject → iptc_caption → xmp_description
keywords    = union(exif_xp_keywords split ';', iptc_keywords, xmp_subject)   # dedup, trimmed
category    = xmp_category
```

Empty/whitespace values are skipped (never override a populated earlier source).

## Command DTOs and mapping (integration boundary)

Defined in `src-tauri/src/types.rs`; signatures unchanged.

| DTO | Fields | Maps to `photo::Metadata` |
|-----|--------|---------------------------|
| `ReadResult` (out) | `title, description, keywords, categories, releaseFilename` | `title→title`, `description→description`, `keywords→keywords`, `category→categories`; `releaseFilename = ""` (not persisted) |
| `SaveRequest` (in) | `filepath, filename, title, description, keywords, categories, releaseFilename` | `title, description, keywords, categories→category`; `filepath`=target file; `filename`=desired stem (rename); `releaseFilename` retained, not written to image metadata |

Notes:
- Frontend uses plural `categories` (a single string) ↔ backend singular `category`.
- `filename` rename logic and atomic create-on-rename are preserved from the current `save_metadata_impl`.
- `releaseFilename` keeps current behavior: accepted but not written into any metadata block.
