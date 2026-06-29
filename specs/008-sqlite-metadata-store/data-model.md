# Phase 1 Data Model: SQLite Intermediate Metadata Store

## Entity: Photo metadata record (`photo_metadata` table)

The app's stored copy of one photo's metadata + its fingerprint + sync state. Keyed by path.

| Column | Type | Notes |
|--------|------|-------|
| `path` | TEXT **PRIMARY KEY** | Absolute file path — the primary identity (FR-002). |
| `size` | INTEGER NOT NULL | File size in bytes (fingerprint part 1). |
| `mtime` | INTEGER NOT NULL | File modification time, Unix nanoseconds (fingerprint part 2). |
| `hash` | INTEGER NOT NULL | xxh3-64 of the whole file, stored as i64 (fingerprint part 3). |
| `title` | TEXT NOT NULL DEFAULT '' | Metadata field. |
| `description` | TEXT NOT NULL DEFAULT '' | Metadata field. |
| `keywords` | TEXT NOT NULL DEFAULT '[]' | JSON array of strings (`serde_json`). |
| `categories` | TEXT NOT NULL DEFAULT '' | Comma-joined categories (matches the file pipeline's single `category` string). |
| `release_filename` | TEXT NOT NULL DEFAULT '' | Stored here only — the file pipeline does not carry it (see note). |
| `synced` | INTEGER NOT NULL DEFAULT 1 | 1 = in sync with the file; 0 = app-only changes not yet written (FR-005). |
| `created_at` | INTEGER NOT NULL | Unix seconds at insert. |
| `updated_at` | INTEGER NOT NULL | Unix seconds at last update (drives "store is newer" reasoning). |

- **Field set** mirrors what the editor edits (FR-004). The three attribution flags
  (editorial / mature content / illustration) are **not** stored (UI-only).
- **`release_filename` note**: today's file read/write neither reads nor writes it
  (`read_metadata` → `""`, `save_one` ignores it). The store therefore becomes its source of
  truth; resolving a conflict to "file" or pressing Cancel clears it (the file has none).
- **Indexes**: PRIMARY KEY on `path` suffices for all lookups. (No `hash` index — content-based
  lookup / move-rename re-link is out of scope.)
- **Pragmas at open**: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON` (none used yet).

## Value object: Fingerprint

`(size: u64, mtime_nanos: i64, hash: u64)` — computed by `store::fingerprint::compute(path)`:
read `std::fs::metadata` for size + modified→nanos, and stream the whole file through xxh3-64.

**Match rule (clarify Q2)**: a record matches the file **iff** `size`, `mtime`, **and** `hash`
all equal the stored values. Any single difference (including mtime-only) is a mismatch.

## Enum: Sync state

| Value | Meaning | File status shown |
|-------|---------|-------------------|
| `synced` | Store record equals the file's metadata; fingerprint current. | `open` (or `edit` while the form is dirty pre-persist) |
| `appOnly` | Store has metadata not yet written to the file (edits / attribution / revert pending). | **`app`** ("in app") |

Serialized camelCase across IPC. Backend enum `SyncState { Synced, AppOnly }`.

## Read-flow resolution (state machine, per `docs/SQLite.puml`)

```
open_metadata(path):
  rec = SELECT by path
  if rec is None:
      meta = read file; INSERT (fingerprint=now, synced=1)
      → Resolved(meta, synced)
  else:
      fp = compute(path)
      if fp == rec.fingerprint (all three):
          → Resolved(rec.meta, rec.syncState)            # unchanged file → trust store
      else if rec.synced == 0 (appOnly):
          → Resolved(rec.meta, appOnly)                  # store is newer → store wins (no prompt)
      else:
          → Conflict(store=rec.meta, file=read file)     # external edit → ask the user

apply_metadata_source(path, source):
  if source == "store":
      UPDATE fingerprint = compute(path)                 # keep store, adopt new fingerprint
      → Resolved(rec.meta, synced)
  if source == "file":
      meta = read file; UPDATE meta + fingerprint, synced=1
      → Resolved(meta, synced)
```

## Write / lifecycle transitions

| Trigger | Effect on store | Resulting state |
|---------|-----------------|-----------------|
| Open, no record | INSERT from file, fingerprint=now | `synced` |
| Field edit / single attribution (debounced) | UPSERT fields, `synced=0`, file untouched | `appOnly` |
| Batch attribution (per item) | UPSERT fields, `synced=0`, file untouched | `appOnly` |
| **Save** (file write) | After file write: refresh fingerprint, `synced=1`; on rename move row to new path | `synced` |
| Batch **Save** (per item) | Same as Save, per file | `synced` |
| **Cancel** (`revert_to_file`) | Overwrite store from file, refresh fingerprint, `synced=1` | `synced` |
| Conflict → "store" | Refresh fingerprint only | `synced` |
| Conflict → "file" | Overwrite store from file, refresh fingerprint | `synced` |
| File deleted on disk | Record retained (no cleanup) | unchanged |
| Store error (any op) | Log + fall back to direct file read/write | n/a (acts as today) |

## Status precedence (single mode, frontend `$derived`)

`none` (no file) → `edit` (form dirty, not yet persisted to store — transient) →
`app` (record `appOnly`) → `open` (record `synced`). Batch mode keeps `batch`.
