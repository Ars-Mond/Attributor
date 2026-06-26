# Data Model: Configurable Photo Caching

Conceptual entities and their concrete shapes. No persistent schema changes beyond four new keys in
the existing settings store. IPC types use `#[serde(rename_all = "camelCase")]` (§IX).

## Cache Settings (persisted, frontend source of truth)

Four boolean settings registered in `src/lib/settings/` (section "Caching"), persisted in
`settings.json`, read reactively via `settings.subscribe(key)`.

| Key | Type | Default | Governs |
|-----|------|---------|---------|
| `cache.photo` | boolean | `false` | Large/viewer (high) preview: cached thumbnail vs direct original. |
| `cache.smallThumbnails` | boolean | `false` | Small/list (low) preview: cached thumbnail vs direct original. |
| `cache.lazy` | boolean | `false` | Generation timing: off = at folder open; on = low on hierarchy-show, high on viewer-open. |
| `cache.currentFolderOnly` | boolean | `true` | Automatic-generation scope: top level only vs recurse into subfolders. |

Independence: `cache.photo` and `cache.smallThumbnails` are independent (FR-004).

## GenConfig (IPC input — folder open/scan)

Derived on the frontend from the settings and sent to `open_folder` / `open_folder_path` /
`scan_folder`. Tells the backend what to generate eagerly and how deep.

| Field | Type | Derivation | Meaning |
|-------|------|------------|---------|
| `low` | bool | `!cache.lazy && cache.smallThumbnails` | Eagerly generate small thumbnails at folder open. |
| `high` | bool | `!cache.lazy && cache.photo` | Eagerly generate large thumbnails at folder open. |
| `recursive` | bool | `!cache.currentFolderOnly` | Generate for subfolder photos too (else top level only). |

The backend starts the pipeline only when `low || high`; otherwise it scans the tree and generates
nothing up front (covers lazy mode and "all caching off").

## On-demand generation (IPC input — `cache_thumbnail`)

Replaces/generalizes the viewer's `get_thumbnails`. Generates the requested size(s) for a single photo
and returns the cache paths.

| Field | Type | Meaning |
|-------|------|---------|
| `path` | String | The photo to generate for. |
| `low` | bool | Produce/refresh the small thumbnail. |
| `high` | bool | Produce/refresh the large thumbnail. |

Returns `Thumbnails { low, high }` (the existing type — both paths, deterministic). Callers:
- Viewer open: `cache_thumbnail(path, low=false, high=true)` (when `cache.photo` on) — FR-017 explicit-open path, scope-free.
- Lazy list show: `cache_thumbnail(path, low=true, high=false)` (when `cache.lazy && cache.smallThumbnails`).

## Backend generation primitive (in-process)

`photo::thumbnail::ensure(source, low: bool, high: bool) -> Result<Thumbnails, String>`:
- Reuse each requested size if its cached file is valid (`is_valid`).
- If any requested size is missing, decode the source **once**, then generate only the requested
  missing sizes (atomic temp+rename, concurrency-safe as today).
- `ensure_thumbnails(source) == ensure(source, true, true)` (unchanged callers).

## Display source (frontend, derived per photo + size)

Not persisted — computed in the UI from settings + cache availability:

| Size | Setting on | Show |
|------|-----------|------|
| Large (viewer) | `cache.photo` on | cached high (fallback original until ready) |
| Large (viewer) | `cache.photo` off | original directly (no generation) |
| Small (list) | `cache.smallThumbnails` on | cached low when ready (placeholder/original until ready; lazy triggers generation on show) |
| Small (list) | `cache.smallThumbnails` off | original directly |

## Relationships

```text
settings.json ──(reactive read)──▶ frontend
   frontend ──GenConfig{low,high,recursive}──▶ open/scan ──▶ pipeline::start ──▶ ensure(low,high) per photo (scope = recursive)
   frontend (viewer open) ──cache_thumbnail(path,false,true)──▶ ensure(false,true)  [scope-free, FR-017]
   frontend (lazy list show) ──cache_thumbnail(path,true,false)──▶ ensure(true,false) ──▶ add path to readyThumbs
   pipeline ──thumbnail-ready (eager)──▶ readyThumbs
   display source per size ◀── settings + (readyThumbs / cache availability)
```
