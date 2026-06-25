# Research: Configurable Photo Caching

Phase 0 decisions. Each entry: Decision / Rationale / Alternatives considered. No new dependencies
are introduced, so this phase is design decisions over the existing code.

## R1 — Where the settings live and how the backend learns them

**Decision**: Keep the four toggles in the existing registry-driven settings store
(`src/lib/settings/`) as the single source of truth. The backend stays stateless about settings:
the frontend reads them and passes the relevant flags as **typed command arguments** — a generation
config on folder open/scan, and explicit `low`/`high` flags on the on-demand command.

**Rationale**: The settings system already persists (`tauri-plugin-store`), auto-renders boolean
fields in the settings dialog, and exposes reactive reads (`settings.subscribe` in `$derived`). The
generation entry points are already frontend-driven commands, so passing flags per call is a tiny,
typed change (§IX) that avoids the backend reading a store inside hot loops (§VIII) and avoids a
second source of truth. Settings cross the boundary once per folder open (config) or per explicit
user action (lazy), never inside a loop.

**Alternatives considered**:
- Backend reads `settings.json` directly — adds a second reader, risks drift, and couples the backend
  to the store format. Rejected.
- Managed Tauri state mirroring settings, updated by a command on every change — more moving parts
  than passing flags per call; the frontend already calls the generation commands. Rejected.

## R2 — Splitting small/large generation (one decode when both)

**Decision**: Refactor `photo::thumbnail` so the public worker is
`ensure(source, low: bool, high: bool) -> Thumbnails`: it checks `is_valid` per requested size,
decodes the source **once** if either requested size is missing, then generates only the requested
sizes; valid existing sizes are reused. `ensure_thumbnails(source)` becomes `ensure(source, true, true)`
(unchanged callers keep working). `thumbnail_paths` is unchanged.

**Rationale**: This is the minimal change that satisfies FR-010 (independent sizes) and FR-011 (single
decode when both produced together) in one place. The existing decode→resize→atomic-write path and the
concurrency-safe unique-temp write are preserved; only "which sizes" becomes a parameter. In lazy mode
the two sizes are requested at different moments (low on hierarchy show, high on viewer open) and thus
decode independently — consistent with the clarified FR-011.

**Alternatives considered**:
- Two separate functions `ensure_low`/`ensure_high` with no combined path — would decode twice when
  both are needed eagerly, violating FR-011. Rejected (a single `ensure(low,high)` subsumes both).

## R3 — Generation triggers (eager vs lazy) and scope

**Decision**: The frontend derives a generation config from settings and drives triggers:
- `recursive = !currentFolderOnly`
- `eagerLow = !lazy && smallThumbnails`, `eagerHigh = !lazy && photoCaching`
- On folder open/scan, pass `{low: eagerLow, high: eagerHigh, recursive}`; the backend starts the
  pipeline only if `low || high` (otherwise it generates nothing up front).
- Lazy low: when an image item is shown in the tree/hierarchy and its low thumbnail isn't ready, the
  frontend calls `cache_thumbnail(path, low=true, high=false)` and adds the path to `readyThumbs`.
- Lazy/explicit high: the viewer calls `cache_thumbnail(path, low=false, high=true)` on open (this is
  also the FR-017 "explicit open ignores scope" path — it generates for the one opened photo
  regardless of `currentFolderOnly`).

The pipeline's `collect_jobs` honors `recursive`: top-level photos only when false, the whole subtree
when true. Scope therefore bounds only automatic (eager + lazy-on-hierarchy) generation; the on-demand
viewer command is per-path and inherently scope-free.

**Rationale**: Maps each clarified trigger to the existing mechanisms — the folder pipeline for eager
bulk generation, and the existing on-viewer-open command (generalized) for explicit/lazy generation.
The lazy-low trigger keys off the item being rendered in the (expandable) tree, which is exactly
"shown in the hierarchy".

**Alternatives considered**:
- Backend-driven lazy (watch which nodes are visible) — the backend has no view of UI visibility;
  the frontend owns "shown in the hierarchy". Rejected.
- An `IntersectionObserver` for pixel-perfect visibility — more machinery than needed; a node is in the
  tree only when its folder is expanded, so render-time triggering matches the requirement. Kept as a
  possible refinement, not required for v1.

## R4 — Display source per size

**Decision**: The frontend chooses what to show per size from settings + cache availability:
- **Viewer (high)**: `photoCaching` on → show the cached high thumbnail (`cache_thumbnail` high),
  falling back to the original until ready; off → show the original directly (`convertFileSrc(path)`),
  with no generation.
- **List (low)**: `smallThumbnails` on → show the cached low thumbnail when ready (existing
  `readyThumbs` gate), placeholder/original until ready; off → show the original directly.

An existing cache file whose toggle is now off is simply not used for display and is not deleted
(FR-009).

**Rationale**: Directly implements FR-005..FR-008 with the display layer the app already has (the
viewer's `showInViewer`, FileTree's `showThumb` gate). The "direct" path reuses `convertFileSrc` on the
original — the pre-cache behavior.

**Alternatives considered**:
- List shows only an icon (never the original) when small caching is off — cheaper but less useful;
  the spec allows "original or placeholder/icon". We show the original directly to mirror the viewer's
  "direct link", and note the icon-only fallback as a cheaper option if large-original list loads prove
  heavy.

## R5 — `readyThumbs` population across eager and lazy

**Decision**: Keep `readyThumbs` (the reactive set the tree gates on) fed from two sources: the
pipeline's `thumbnail-ready` broadcast (eager mode) and, for lazy/on-demand low generation, the
frontend adds the path to `readyThumbs` when `cache_thumbnail` resolves successfully. No new event is
needed for the on-demand path (the command return is sufficient).

**Rationale**: Reuses the existing event-driven set for eager mode and a direct return for on-demand,
avoiding extra broadcast churn for single-photo generation.

## R6 — Settings UI

**Decision**: Register a new "Caching" settings section with four `type: 'boolean'` descriptors
(`cache.photo`, `cache.smallThumbnails`, `cache.lazy`, `cache.currentFolderOnly`) and the specified
defaults. The registry-driven `SettingsDialog` renders boolean fields automatically (as the existing
`editor.autosave` boolean does), so no new component is required.

**Rationale**: Zero-UI-code addition via the existing registry (§V). Labels/descriptions live in the
descriptor.

**Alternatives considered**:
- A bespoke caching settings panel — unnecessary; the registry already renders booleans. Rejected.
