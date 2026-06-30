//! Intermediate metadata store (SQLite via `rusqlite`). It sits in front of the photo files:
//! opening a photo resolves metadata store-first (read-flow per docs/SQLite.puml), manual edits and
//! attribution persist here as app-only (file untouched), and Save writes the file then refreshes
//! the record. The full-file hash is authoritative for change detection. Every operation logs; if
//! the store is unavailable the commands fall back to direct file access so editing still works.

mod fingerprint;
mod record;
mod schema;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use fingerprint::Fingerprint;
pub use record::{MetadataResolution, StoredMetadata, SyncState};

type Conn = Arc<Mutex<Connection>>;

/// Tauri-managed store handle. A single connection guarded by a mutex (single-user desktop app); all
/// access runs inside `spawn_blocking`. `None` means the store failed to open — callers fall back to
/// direct file access (FR-021).
pub struct DbState {
    conn: Option<Conn>,
}

impl DbState {
    /// Open (or create) the database and ensure the schema exists. Never fails the app: on error the
    /// handle is left empty and the store degrades to direct file access.
    pub fn open(db_path: &Path) -> Self {
        match Connection::open(db_path).and_then(|c| schema::init(&c).map(|_| c)) {
            Ok(conn) => {
                log::info!("metadata store opened: {}", db_path.display());
                DbState { conn: Some(Arc::new(Mutex::new(conn))) }
            }
            Err(e) => {
                log::error!(
                    "metadata store unavailable ({}): {e}; falling back to direct file access",
                    db_path.display()
                );
                DbState { conn: None }
            }
        }
    }

    /// A disabled store (no database) — every command falls back to direct file access.
    pub fn disabled() -> Self {
        DbState { conn: None }
    }

    fn handle(&self) -> Option<Conn> {
        self.conn.clone()
    }

    /// A handle sharing the same underlying connection — for moving into a blocking/parallel task
    /// (batch save, batch attribution) where a `tauri::State` reference cannot be captured.
    pub fn share(&self) -> DbState {
        DbState { conn: self.conn.clone() }
    }

    /// After a file write, refresh the store record for `new_path` to mirror the saved metadata and
    /// mark it synced; move the row from `old_path` on rename. Best-effort (logs on error).
    pub fn sync_after_save(&self, old_path: &str, new_path: &str, meta: &StoredMetadata) {
        let Some(conn) = self.handle() else {
            log::warn!("store sync_after_save skipped (store unavailable): {new_path}");
            return;
        };
        if let Err(e) = sync_after_save(&conn, old_path, new_path, meta) {
            log::warn!("store sync_after_save failed for {new_path}: {e}");
        }
    }

    /// Batch save sync (preserves the store-only fields from the existing record). Best-effort.
    pub fn sync_after_save_batch(&self, old_path: &str, new_path: &str, file_meta: &StoredMetadata) {
        let Some(conn) = self.handle() else {
            log::warn!("store batch sync skipped (store unavailable): {new_path}");
            return;
        };
        if let Err(e) = sync_after_save_keep_store_only(&conn, old_path, new_path, file_meta) {
            log::warn!("store batch sync failed for {new_path}: {e}");
        }
    }

    /// Store a batch-attribution result as app-only (file untouched). Best-effort.
    pub fn store_attribution(&self, path: &str, model: &StoredMetadata) {
        let Some(conn) = self.handle() else {
            log::warn!("store attribution skipped (store unavailable): {path}");
            return;
        };
        if let Err(e) = attribute_app_only(&conn, path, model) {
            log::warn!("store attribution failed for {path}: {e}");
        }
    }

    /// Pure read for CSV export (FR-009): return the stored metadata for `path` without any mutation
    /// (no fingerprint/mtime refresh). `None` if the store is unavailable or the path has no record.
    pub fn fetch(&self, path: &str) -> Option<StoredMetadata> {
        let conn = self.handle()?;
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        read_record(&c, path).ok().flatten().map(|r| r.meta)
    }
}

// ── Internals ────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

struct Record {
    meta: StoredMetadata,
    fp: Fingerprint,
    synced: bool,
}

/// Pure read-flow decision for an existing record (hash is authoritative). Kept separate so it can
/// be unit-tested without touching files or the database.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Unchanged, // hash matches → load store (mtime may be silently refreshed)
    StoreWins, // hash differs but record is app-only → store wins, no prompt
    Conflict,  // hash differs and record is synced → external content change → ask the user
}

fn decide(stored_hash: u64, synced: bool, current_hash: u64) -> Decision {
    if stored_hash == current_hash {
        Decision::Unchanged
    } else if !synced {
        Decision::StoreWins
    } else {
        Decision::Conflict
    }
}

fn read_record(conn: &Connection, path: &str) -> Result<Option<Record>, String> {
    conn.query_row(
        "SELECT size, mtime, hash, title, description, keywords, categories, release_filename,
                editorial, mature_content, illustration, synced
         FROM photo_metadata WHERE path = ?1",
        [path],
        |r| {
            let keywords_json: String = r.get(5)?;
            Ok(Record {
                meta: StoredMetadata {
                    title: r.get(3)?,
                    description: r.get(4)?,
                    keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
                    categories: r.get(6)?,
                    release_filename: r.get(7)?,
                    editorial: r.get(8)?,
                    mature_content: r.get(9)?,
                    illustration: r.get(10)?,
                },
                fp: Fingerprint {
                    size: r.get::<_, i64>(0)? as u64,
                    mtime: r.get(1)?,
                    hash: r.get::<_, i64>(2)? as u64,
                },
                synced: r.get::<_, i64>(11)? != 0,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Insert or replace a full record (used on first open, file-source resolution, and save sync).
fn upsert_record(
    conn: &Connection,
    path: &str,
    meta: &StoredMetadata,
    fp: &Fingerprint,
    synced: bool,
) -> Result<(), String> {
    let now = now_secs();
    let kw = serde_json::to_string(&meta.keywords).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO photo_metadata
            (path,size,mtime,hash,title,description,keywords,categories,release_filename,
             editorial,mature_content,illustration,synced,created_at,updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)
         ON CONFLICT(path) DO UPDATE SET
            size=excluded.size, mtime=excluded.mtime, hash=excluded.hash,
            title=excluded.title, description=excluded.description, keywords=excluded.keywords,
            categories=excluded.categories, release_filename=excluded.release_filename,
            editorial=excluded.editorial, mature_content=excluded.mature_content,
            illustration=excluded.illustration, synced=excluded.synced, updated_at=excluded.updated_at",
        params![
            path, fp.size as i64, fp.mtime, fp.hash as i64,
            meta.title, meta.description, kw, meta.categories, meta.release_filename,
            meta.editorial, meta.mature_content, meta.illustration, synced as i64, now
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Read the file's embedded metadata as a `StoredMetadata` (release_filename has no file source).
fn read_file_metadata(path: &str) -> Result<StoredMetadata, String> {
    let m = crate::photo::read_metadata(path.to_string())?;
    Ok(StoredMetadata {
        title: m.title,
        description: m.description,
        keywords: m.keywords,
        categories: m.category,
        // No file-side equivalent — defaults; the store retains its own values on file-source resolution.
        release_filename: String::new(),
        editorial: false,
        mature_content: false,
        illustration: false,
    })
}

/// Read-flow resolution (data-model.md). Hash is authoritative.
fn resolve_open(conn: &Mutex<Connection>, path: &str) -> Result<MetadataResolution, String> {
    let existing = {
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        read_record(&c, path)?
    };

    match existing {
        None => {
            let meta = read_file_metadata(path)?;
            let fp = fingerprint::compute(Path::new(path))?;
            let c = conn.lock().unwrap_or_else(|e| e.into_inner());
            upsert_record(&c, path, &meta, &fp, true)?;
            log::info!("store: created record for {path}");
            Ok(MetadataResolution::Resolved { metadata: meta, sync_state: SyncState::Synced })
        }
        Some(rec) => {
            let fp = fingerprint::compute(Path::new(path))?;
            match decide(rec.fp.hash, rec.synced, fp.hash) {
                Decision::Unchanged => {
                    // Content unchanged (hash authoritative). Silently refresh mtime/size if drifted.
                    if fp.mtime != rec.fp.mtime || fp.size != rec.fp.size {
                        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
                        let _ = c.execute(
                            "UPDATE photo_metadata SET mtime=?1, size=?2 WHERE path=?3",
                            params![fp.mtime, fp.size as i64, path],
                        );
                    }
                    let sync_state = if rec.synced { SyncState::Synced } else { SyncState::AppOnly };
                    Ok(MetadataResolution::Resolved { metadata: rec.meta, sync_state })
                }
                Decision::StoreWins => {
                    log::info!("store: hash changed but record app-only → store wins for {path}");
                    Ok(MetadataResolution::Resolved { metadata: rec.meta, sync_state: SyncState::AppOnly })
                }
                Decision::Conflict => {
                    log::info!("store: external content change for {path} → conflict");
                    let file = read_file_metadata(path)?;
                    Ok(MetadataResolution::Conflict { store: rec.meta, file })
                }
            }
        }
    }
}

/// Persist the working fields as app-only (file untouched). Keeps the existing fingerprint when the
/// record exists; computes one if this is the first persist for the path.
fn persist_app_only(conn: &Mutex<Connection>, path: &str, meta: &StoredMetadata) -> Result<SyncState, String> {
    let now = now_secs();
    let kw = serde_json::to_string(&meta.keywords).unwrap_or_else(|_| "[]".to_string());
    let updated = {
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        c.execute(
            "UPDATE photo_metadata SET title=?1, description=?2, keywords=?3, categories=?4,
                release_filename=?5, editorial=?6, mature_content=?7, illustration=?8,
                synced=0, updated_at=?9 WHERE path=?10",
            params![
                meta.title, meta.description, kw, meta.categories, meta.release_filename,
                meta.editorial, meta.mature_content, meta.illustration, now, path
            ],
        )
        .map_err(|e| e.to_string())?
    };
    if updated == 0 {
        let fp = fingerprint::compute(Path::new(path))
            .unwrap_or(Fingerprint { size: 0, mtime: 0, hash: 0 });
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        upsert_record(&c, path, meta, &fp, false)?;
    }
    Ok(SyncState::AppOnly)
}

/// Reset / revert-to-file: make the record match the file. The Reset button is the ONE operation
/// that CLEARS the store-only fields (release_filename + flags) — they revert to the file (which has
/// none). Every other DB update preserves them (see `keep_store_only`).
fn revert(conn: &Mutex<Connection>, path: &str) -> Result<StoredMetadata, String> {
    let meta = read_file_metadata(path)?;
    let fp = fingerprint::compute(Path::new(path))?;
    let c = conn.lock().unwrap_or_else(|e| e.into_inner());
    upsert_record(&c, path, &meta, &fp, true)?;
    Ok(meta)
}

/// Single-file save sync: refresh the record from the saved metadata (which carries the form's real
/// store-only fields) and mark it synced; move the row on rename.
fn sync_after_save(conn: &Mutex<Connection>, old_path: &str, new_path: &str, meta: &StoredMetadata) -> Result<(), String> {
    let fp = fingerprint::compute(Path::new(new_path))?;
    let c = conn.lock().unwrap_or_else(|e| e.into_inner());
    if old_path != new_path {
        let _ = c.execute("DELETE FROM photo_metadata WHERE path=?1", [old_path]);
    }
    upsert_record(&c, new_path, meta, &fp, true)
}

/// Overwrite `meta`'s store-only fields (release_filename + attribution flags) with the existing
/// record's values. This is the preservation policy for every DB update except the Reset button.
fn keep_store_only(c: &Connection, path: &str, meta: &mut StoredMetadata) -> Result<(), String> {
    if let Some(rec) = read_record(c, path)? {
        meta.release_filename = rec.meta.release_filename;
        meta.editorial = rec.meta.editorial;
        meta.mature_content = rec.meta.mature_content;
        meta.illustration = rec.meta.illustration;
    }
    Ok(())
}

/// Batch save sync: file-backed fields come from the batch item; store-only fields are preserved from
/// the existing record (batch editing does not touch them).
fn sync_after_save_keep_store_only(conn: &Mutex<Connection>, old_path: &str, new_path: &str, file_meta: &StoredMetadata) -> Result<(), String> {
    let fp = fingerprint::compute(Path::new(new_path))?;
    let c = conn.lock().unwrap_or_else(|e| e.into_inner());
    let mut meta = file_meta.clone();
    keep_store_only(&c, old_path, &mut meta)?;
    if old_path != new_path {
        let _ = c.execute("DELETE FROM photo_metadata WHERE path=?1", [old_path]);
    }
    upsert_record(&c, new_path, &meta, &fp, true)
}

/// Batch attribution: store the model's result as app-only. Flags come from the model; release_filename
/// is preserved, and keywords are merged with the existing record (keep existing, append new).
fn attribute_app_only(conn: &Mutex<Connection>, path: &str, model: &StoredMetadata) -> Result<SyncState, String> {
    let mut meta = model.clone();
    {
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(rec) = read_record(&c, path)? {
            meta.release_filename = rec.meta.release_filename; // preserved (model doesn't produce it)
            // Merge keywords: keep the existing ones, append the model's new ones (case-insensitive dedupe).
            let mut keywords = rec.meta.keywords;
            for kw in &model.keywords {
                let k = kw.trim();
                if !k.is_empty() && !keywords.iter().any(|e| e.eq_ignore_ascii_case(k)) {
                    keywords.push(k.to_string());
                }
            }
            meta.keywords = keywords;
        }
    }
    persist_app_only(conn, path, &meta)
}

/// Finalize a conflict (FR-012): keep the store version (refresh the fingerprint) or take the file
/// version (preserving the store-only fields, which have no file equivalent).
fn apply_source(conn: &Mutex<Connection>, path: &str, from_file: bool) -> Result<MetadataResolution, String> {
    let fp = fingerprint::compute(Path::new(path))?;
    if from_file {
        let mut meta = read_file_metadata(path)?;
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        keep_store_only(&c, path, &mut meta)?;
        upsert_record(&c, path, &meta, &fp, true)?;
        Ok(MetadataResolution::Resolved { metadata: meta, sync_state: SyncState::Synced })
    } else {
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        let metadata = read_record(&c, path)?.map(|r| r.meta).unwrap_or_default();
        c.execute(
            "UPDATE photo_metadata SET size=?1, mtime=?2, hash=?3, synced=1, updated_at=?4 WHERE path=?5",
            params![fp.size as i64, fp.mtime, fp.hash as i64, now_secs(), path],
        )
        .map_err(|e| e.to_string())?;
        Ok(MetadataResolution::Resolved { metadata, sync_state: SyncState::Synced })
    }
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Resolve a photo's metadata store-first (read-flow). Falls back to a direct file read when the
/// store is unavailable.
#[tauri::command]
pub async fn open_metadata(
    path: String,
    state: tauri::State<'_, DbState>,
) -> Result<MetadataResolution, String> {
    match state.handle() {
        Some(conn) => tokio::task::spawn_blocking(move || resolve_open(&conn, &path))
            .await
            .map_err(|e| e.to_string())?,
        None => {
            let meta = tokio::task::spawn_blocking(move || read_file_metadata(&path))
                .await
                .map_err(|e| e.to_string())??;
            Ok(MetadataResolution::Resolved { metadata: meta, sync_state: SyncState::Synced })
        }
    }
}

/// Persist the working fields to the store as app-only (file untouched). No-op (returns `Synced`)
/// when the store is unavailable.
#[tauri::command]
pub async fn store_metadata(
    path: String,
    fields: StoredMetadata,
    state: tauri::State<'_, DbState>,
) -> Result<SyncState, String> {
    match state.handle() {
        Some(conn) => tokio::task::spawn_blocking(move || persist_app_only(&conn, &path, &fields))
            .await
            .map_err(|e| e.to_string())?,
        None => {
            log::warn!("store_metadata skipped (store unavailable): {path}");
            Ok(SyncState::Synced)
        }
    }
}

/// Reset / revert-to-file: restore the record (and the returned fields) from the file, CLEARING the
/// store-only fields (release_filename + flags). Falls back to a plain file read when the store is
/// unavailable.
#[tauri::command]
pub async fn revert_to_file(
    path: String,
    state: tauri::State<'_, DbState>,
) -> Result<StoredMetadata, String> {
    match state.handle() {
        Some(conn) => tokio::task::spawn_blocking(move || revert(&conn, &path))
            .await
            .map_err(|e| e.to_string())?,
        None => tokio::task::spawn_blocking(move || read_file_metadata(&path))
            .await
            .map_err(|e| e.to_string())?,
    }
}

/// Finalize a conflict from `open_metadata` (FR-012). `source` is "store" (keep the store version) or
/// "file" (take the file version, preserving the store-only fields).
#[tauri::command]
pub async fn apply_metadata_source(
    path: String,
    source: String,
    state: tauri::State<'_, DbState>,
) -> Result<MetadataResolution, String> {
    let from_file = source != "store";
    match state.handle() {
        Some(conn) => tokio::task::spawn_blocking(move || apply_source(&conn, &path, from_file))
            .await
            .map_err(|e| e.to_string())?,
        None => {
            let meta = tokio::task::spawn_blocking(move || read_file_metadata(&path))
                .await
                .map_err(|e| e.to_string())??;
            Ok(MetadataResolution::Resolved { metadata: meta, sync_state: SyncState::Synced })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Mutex<Connection> {
        let c = Connection::open_in_memory().unwrap();
        schema::init(&c).unwrap();
        Mutex::new(c)
    }

    fn sample() -> StoredMetadata {
        StoredMetadata {
            title: "T".into(),
            description: "D".into(),
            keywords: vec!["a".into(), "b".into()],
            categories: "Nature".into(),
            release_filename: "r.pdf".into(),
            editorial: true,
            mature_content: false,
            illustration: true,
        }
    }

    #[test]
    fn decide_is_hash_authoritative() {
        // Same hash → unchanged regardless of the synced flag (covers the mtime-only touch case).
        assert_eq!(decide(10, true, 10), Decision::Unchanged);
        assert_eq!(decide(10, false, 10), Decision::Unchanged);
        // Different hash + app-only → store wins (no prompt).
        assert_eq!(decide(10, false, 20), Decision::StoreWins);
        // Different hash + synced → conflict.
        assert_eq!(decide(10, true, 20), Decision::Conflict);
    }

    #[test]
    fn upsert_and_read_round_trip() {
        let conn = mem();
        let fp = Fingerprint { size: 1, mtime: 2, hash: 3 };
        {
            let c = conn.lock().unwrap();
            upsert_record(&c, "p", &sample(), &fp, true).unwrap();
        }
        let c = conn.lock().unwrap();
        let rec = read_record(&c, "p").unwrap().expect("record exists");
        assert_eq!(rec.meta.keywords, vec!["a".to_string(), "b".to_string()]);
        assert!(rec.meta.editorial && !rec.meta.mature_content && rec.meta.illustration);
        assert_eq!(rec.fp, fp);
        assert!(rec.synced);
    }

    #[test]
    fn persist_app_only_clears_synced_and_keeps_fingerprint() {
        let conn = mem();
        let fp = Fingerprint { size: 1, mtime: 2, hash: 3 };
        {
            let c = conn.lock().unwrap();
            upsert_record(&c, "p", &sample(), &fp, true).unwrap();
        }
        let mut edited = sample();
        edited.title = "edited".into();
        let state = persist_app_only(&conn, "p", &edited).unwrap();
        assert_eq!(state, SyncState::AppOnly);
        let c = conn.lock().unwrap();
        let rec = read_record(&c, "p").unwrap().unwrap();
        assert!(!rec.synced);
        assert_eq!(rec.meta.title, "edited");
        assert_eq!(rec.fp, fp); // app-only persist must not change the fingerprint
    }

    #[test]
    fn sync_after_save_marks_synced_and_moves_row() {
        let conn = mem();
        {
            let c = conn.lock().unwrap();
            upsert_record(&c, "old", &sample(), &Fingerprint { size: 0, mtime: 0, hash: 0 }, false).unwrap();
        }
        let path = std::env::temp_dir().join(format!("attributor_store_test_{}.bin", std::process::id()));
        std::fs::write(&path, b"hello").unwrap();
        let new_path = path.to_string_lossy().to_string();

        sync_after_save(&conn, "old", &new_path, &sample()).unwrap();

        let c = conn.lock().unwrap();
        assert!(read_record(&c, "old").unwrap().is_none(), "old row moved on rename");
        let rec = read_record(&c, &new_path).unwrap().expect("new row exists");
        assert!(rec.synced);
        drop(c);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn batch_sync_preserves_store_only_fields() {
        let conn = mem();
        {
            let c = conn.lock().unwrap();
            upsert_record(&c, "old", &sample(), &Fingerprint { size: 0, mtime: 0, hash: 0 }, false).unwrap();
        }
        let path = std::env::temp_dir().join(format!("attributor_batchsync_{}.bin", std::process::id()));
        std::fs::write(&path, b"data").unwrap();
        let new_path = path.to_string_lossy().to_string();
        // The batch "file" metadata carries no store-only fields (defaults).
        let file_meta = StoredMetadata { title: "new title".into(), ..Default::default() };

        sync_after_save_keep_store_only(&conn, "old", &new_path, &file_meta).unwrap();

        let c = conn.lock().unwrap();
        let rec = read_record(&c, &new_path).unwrap().expect("new row exists");
        assert!(rec.synced);
        assert_eq!(rec.meta.title, "new title");
        // Store-only fields preserved from the seeded record (sample()).
        assert_eq!(rec.meta.release_filename, "r.pdf");
        assert!(rec.meta.editorial && rec.meta.illustration);
        drop(c);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn apply_source_store_keeps_metadata_refreshes_fingerprint() {
        let conn = mem();
        let path = std::env::temp_dir().join(format!("attributor_applystore_{}.bin", std::process::id()));
        std::fs::write(&path, b"data").unwrap();
        let p = path.to_string_lossy().to_string();
        {
            let c = conn.lock().unwrap();
            upsert_record(&c, &p, &sample(), &Fingerprint { size: 0, mtime: 0, hash: 0 }, false).unwrap();
        }

        let res = apply_source(&conn, &p, false).unwrap(); // keep store
        match res {
            MetadataResolution::Resolved { metadata, sync_state } => {
                assert_eq!(sync_state, SyncState::Synced);
                assert_eq!(metadata.title, "T");
                assert!(metadata.editorial);
            }
            _ => panic!("expected resolved"),
        }
        let c = conn.lock().unwrap();
        let rec = read_record(&c, &p).unwrap().unwrap();
        assert!(rec.synced);
        assert_ne!(rec.fp, Fingerprint { size: 0, mtime: 0, hash: 0 }); // fingerprint refreshed
        drop(c);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn disabled_store_write_methods_are_noops() {
        // When the store is unavailable, the best-effort write methods must not panic (FR-021).
        let db = DbState::disabled();
        db.sync_after_save("a", "b", &sample());
        db.sync_after_save_batch("a", "b", &sample());
        db.store_attribution("a", &sample());
    }

    #[test]
    fn resolve_open_unchanged_refreshes_mtime_silently() {
        // A synced record whose stored hash matches the file but with a stale mtime: hash is
        // authoritative → Unchanged (no conflict), and the mtime is silently refreshed.
        let conn = mem();
        let path = std::env::temp_dir().join(format!("attributor_resolve_unchanged_{}.bin", std::process::id()));
        std::fs::write(&path, b"content").unwrap();
        let p = path.to_string_lossy().to_string();
        let fp = fingerprint::compute(&path).unwrap();
        {
            let c = conn.lock().unwrap();
            let stale = Fingerprint { size: fp.size, mtime: fp.mtime - 1_000_000, hash: fp.hash };
            upsert_record(&c, &p, &sample(), &stale, true).unwrap();
        }

        let res = resolve_open(&conn, &p).unwrap();
        match res {
            MetadataResolution::Resolved { sync_state, .. } => assert_eq!(sync_state, SyncState::Synced),
            _ => panic!("matching hash must resolve (unchanged), never conflict"),
        }
        let c = conn.lock().unwrap();
        assert_eq!(read_record(&c, &p).unwrap().unwrap().fp.mtime, fp.mtime); // silently refreshed
        drop(c);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn resolve_open_app_only_store_wins_on_hash_change() {
        // An app-only record whose stored hash differs from the file → store wins, no conflict.
        let conn = mem();
        let path = std::env::temp_dir().join(format!("attributor_resolve_storewins_{}.bin", std::process::id()));
        std::fs::write(&path, b"content").unwrap();
        let p = path.to_string_lossy().to_string();
        let fp = fingerprint::compute(&path).unwrap();
        {
            let c = conn.lock().unwrap();
            let different = Fingerprint { size: fp.size, mtime: fp.mtime, hash: fp.hash ^ 0xFFFF };
            upsert_record(&c, &p, &sample(), &different, false).unwrap(); // app-only
        }

        let res = resolve_open(&conn, &p).unwrap();
        match res {
            MetadataResolution::Resolved { metadata, sync_state } => {
                assert_eq!(sync_state, SyncState::AppOnly);
                assert_eq!(metadata.title, "T"); // the store version, not the file
            }
            _ => panic!("app-only + hash change must be store-wins, not conflict"),
        }
        std::fs::remove_file(&path).ok();
    }

    fn mem_db() -> DbState {
        let c = Connection::open_in_memory().unwrap();
        schema::init(&c).unwrap();
        DbState { conn: Some(Arc::new(Mutex::new(c))) }
    }

    #[test]
    fn fetch_returns_record_without_mutation() {
        let db = mem_db();
        let conn = db.conn.clone().unwrap();
        let fp = Fingerprint { size: 7, mtime: 8, hash: 9 };
        {
            let c = conn.lock().unwrap();
            upsert_record(&c, "p", &sample(), &fp, true).unwrap();
        }
        let got = db.fetch("p").expect("record exists");
        assert_eq!(got.title, "T");
        assert_eq!(got.keywords, vec!["a".to_string(), "b".to_string()]);
        assert!(db.fetch("missing").is_none());
        // Pure read: the fingerprint must be unchanged after fetch.
        let c = conn.lock().unwrap();
        assert_eq!(read_record(&c, "p").unwrap().unwrap().fp, fp);
    }

    #[test]
    fn fetch_on_disabled_store_is_none() {
        assert!(DbState::disabled().fetch("p").is_none());
    }
}
