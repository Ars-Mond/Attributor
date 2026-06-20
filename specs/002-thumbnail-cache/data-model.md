# Phase 1 Data Model: Photo Thumbnail Cache

## Entities

### Thumbnail Variant

The two fixed sizes, by longest side.

| Variant | Longest side (px) | Used by |
|---------|-------------------|---------|
| Low | 360 | File hierarchy (tree rows) |
| High | 1920 | On-screen viewer |

### Thumbnails (command result DTO)

Returned by `get_thumbnails`; defined in `photo/thumbnail.rs`.

| Field | Type | Notes |
|-------|------|-------|
| `low` | `String` | Absolute path to the low thumbnail JPG |
| `high` | `String` | Absolute path to the high thumbnail JPG |

`#[derive(Serialize)]` with `#[serde(rename_all = "camelCase")]` → `{ low, high }`.

### Source Photo

An existing JPEG/PNG/WebP file in a user folder; the decode input. Its path determines the cache
location and thumbnail names.

### Thumbnail Folder (`_thumbnail`)

A directory at `source.parent()/_thumbnail`, created on demand, holding that folder's thumbnails.
Excluded from the browsable hierarchy (FR-008).

## Derivation rules (deterministic, FR-012)

```text
dir(photo)        = parent directory of the source photo
thumb_dir         = dir(photo) / "_thumbnail"
file_name(photo)  = full source file name incl. extension   (e.g. "a.png")
low_path          = thumb_dir / "{file_name}.low.jpg"        (e.g. "a.png.low.jpg")
high_path         = thumb_dir / "{file_name}.high.jpg"       (e.g. "a.png.high.jpg")
```

## Geometry rules (FR-005)

```text
(w, h)            = source dimensions
longest           = max(w, h)
target            = 360 (low) | 1920 (high)
if longest <= target:  thumbnail dimensions = (w, h)      # never upscale
else:                  scale = target / longest
                       thumbnail dimensions = (round(w*scale), round(h*scale))
```

Aspect ratio is preserved; output is JPG (rgb8), strong compression.

## State / lifecycle

```text
needed(photo, ...) ──► thumb_dir exists? ── no ─► create _thumbnail
        │                     │ yes
        ▼                     ▼
   for each variant:  file exists & non-empty (valid)?
        ├─ yes ─► reuse path
        └─ no  ─► decode source → resize (longest side) → encode JPG → write → path
   return { low, high }
```

A subsequent failure to load a "present" thumbnail (FR-011) demotes it to invalid → regenerate.

## Command mapping (integration boundary)

| IPC | In | Out | Maps to |
|-----|----|-----|---------|
| `get_thumbnails` | `path: String` (source photo) | `Result<Thumbnails, String>` | `photo::thumbnail::ensure_thumbnails(&Path)` then serialize `{ low, high }` |

Frontend use: file tree (content mode) renders `convertFileSrc(thumbs.low)`; viewer renders
`convertFileSrc(thumbs.high)` after the call resolves, showing a loading indicator until then.
No path is persisted; an in-memory runes map dedupes calls per session only.
