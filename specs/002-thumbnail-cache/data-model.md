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

### FileNode (extended)

The folder-scan tree node gains two optional fields, populated during `scan_dir` for image files:

| Field | Type | Notes |
|-------|------|-------|
| `thumb_low` | `Option<String>` | Low thumbnail path; `None` for non-images or on generation failure |
| `thumb_high` | `Option<String>` | High thumbnail path; `None` likewise |

Serialized snake_case (matching the existing `is_dir`); the frontend reads `node.thumb_low` / `node.thumb_high`.

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

Triggered per image **during `scan_dir`** (results stored on the `FileNode`); the `get_thumbnails`
command is a viewer fallback for files opened outside a scan.

```text
ensure_thumbnails(photo) ──► thumb_dir exists? ── no ─► create _thumbnail
        │                          │ yes
        ▼                          ▼
   for each variant:  file exists & valid?
        ├─ yes ─► reuse path
        └─ no  ─► decode source → resize (longest side) → encode JPG
                  → write atomically (temp file in _thumbnail, then rename) → path
   return { low, high }
```

A subsequent failure to load a "present" thumbnail (FR-011) demotes it to invalid → regenerate.

## Integration boundary

Primary path: `scan_dir` calls `photo::thumbnail::ensure_thumbnails(&Path)` per image and stores
`{ low, high }` into `FileNode.thumb_low` / `thumb_high`. The frontend reads those directly — the
tree (content mode) renders `convertFileSrc(node.thumb_low)`; the viewer renders
`convertFileSrc(node.thumb_high)`.

Fallback IPC (viewer only, files opened outside a scan):

| IPC | In | Out | Maps to |
|-----|----|-----|---------|
| `get_thumbnails` | `path: String` | `Result<Thumbnails, String>` | `ensure_thumbnails(&Path)` then serialize `{ low, high }`; viewer shows a loading indicator until it resolves |

No path is persisted to any database or index file (FR-007).
