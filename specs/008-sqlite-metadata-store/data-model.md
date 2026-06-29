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
| `release_filename` | TEXT NOT NULL DEFAULT '' | Store-only field — the file pipeline does not carry it (see note). |
| `editorial` | INTEGER NOT NULL DEFAULT 0 | Attribution flag (store-only; bool as 0/1). |
| `mature_content` | INTEGER NOT NULL DEFAULT 0 | Attribution flag (store-only; bool as 0/1). |
| `illustration` | INTEGER NOT NULL DEFAULT 0 | Attribution flag (store-only; bool as 0/1). |
| `synced` | INTEGER NOT NULL DEFAULT 1 | 1 = in sync with the file; 0 = app-only changes not yet written (FR-005). |
| `created_at` | INTEGER NOT NULL | Unix seconds at insert. |
| `updated_at` | INTEGER NOT NULL | Unix seconds at last update (drives "store is newer" reasoning). |

- **Field set**: the file-backed fields (title / description / keywords / categories) plus the
  store-only fields with no file equivalent — `release_filename` and the three attribution flags
  (editorial / mature content / illustration) (FR-004).
- **Store-only fields** (`release_filename`, attribution flags): the file pipeline neither reads
  nor writes them (`read_metadata` → defaults, `save_one` ignores them). The store is their source
  of truth. **Preservation rule (maintainer decision):** EVERY database update **preserves** them
  (single/batch save, batch attribution, conflict resolved to "file") — the file side never
  overwrites them. The ONE exception is the **Reset** button (`revert_to_file`), which **clears**
  them (Reset means "match the file", and the file has none). Older databases are migrated with
  `ALTER TABLE ADD COLUMN` for the flag columns.
- **Indexes**: PRIMARY KEY on `path` suffices for all lookups. (No `hash` index — content-based
  lookup / move-rename re-link is out of scope.)
- **Pragmas at open**: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON` (none used yet).

## Value object: Fingerprint

`(size: u64, mtime_nanos: i64, hash: u64)` — computed by `store::fingerprint::compute(path)`:
read `std::fs::metadata` for size + modified→nanos, and stream the whole file through xxh3-64.

**Match rule (analyze-review refinement)**: the **full-file hash is authoritative**. A record
matches the file when the stored `hash` equals the current `hash` (content identical), even if
`mtime` differs — the stored `mtime` is then silently refreshed. Only a **hash difference** is a
mismatch. The hash is always computed on open.

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
      if fp.hash == rec.hash:                            # content identical (hash authoritative)
          if fp.mtime != rec.mtime: UPDATE mtime         # silently refresh, no prompt
          → Resolved(rec.meta, rec.syncState)            # unchanged file → trust store
      else if rec.synced == 0 (appOnly):
          → Resolved(rec.meta, appOnly)                  # store is newer → store wins (no prompt)
      else:
          → Conflict(store=rec.meta, file=read file)     # external content edit → ask the user

apply_metadata_source(path, source):
  if source == "store":
      UPDATE fingerprint = compute(path)                 # keep store, adopt new fingerprint
      → Resolved(rec.meta, synced)
  if source == "file":
      meta = read file (preserve store-only fields); UPDATE meta + fingerprint, synced=1
      → Resolved(meta, synced)
```

## Write / lifecycle transitions

| Trigger | Effect on store | Resulting state |
|---------|-----------------|-----------------|
| Open, no record | INSERT from file, fingerprint=now | `synced` |
| Field edit / single attribution (debounced) | UPSERT form fields (incl. store-only), `synced=0`, file untouched | `appOnly` |
| Batch attribution (per item) | UPSERT model fields + flags, **preserve** `release_filename`, merge keywords, `synced=0`, file untouched | `appOnly` |
| **Save** (single, file write) | After file write: UPSERT form fields (incl. store-only), refresh fingerprint, `synced=1`; move row on rename | `synced` |
| Batch **Save** (per item) | After file write: UPSERT batch fields, **preserve** store-only fields, refresh fingerprint, `synced=1`; move row on rename | `synced` |
| **Reset** (`revert_to_file`) | Overwrite store from file, **CLEAR** store-only fields, refresh fingerprint, `synced=1` | `synced` |
| Conflict → "store" | Refresh fingerprint only | `synced` |
| Conflict → "file" | Overwrite store from file, **preserve** store-only fields, refresh fingerprint | `synced` |
| File deleted on disk | Record retained (no cleanup) | unchanged |
| Store error (any op) | Log + fall back to direct file read/write | n/a (acts as today) |

## Status precedence (single mode, frontend `$derived`)

`none` (no file) → `edit` (form dirty, not yet persisted to store — transient) →
`app` (record `appOnly`) → `open` (record `synced`). Batch mode keeps `batch`.
