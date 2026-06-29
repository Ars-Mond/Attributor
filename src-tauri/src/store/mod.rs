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
        "SELECT size, mtime, hash, title, description, keywords, categories, release_filename, synced
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
                },
                fp: Fingerprint {
                    size: r.get::<_, i64>(0)? as u64,
                    mtime: r.get(1)?,
                    hash: r.get::<_, i64>(2)? as u64,
                },
                synced: r.get::<_, i64>(8)? != 0,
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
            (path,size,mtime,hash,title,description,keywords,categories,release_filename,synced,created_at,updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)
         ON CONFLICT(path) DO UPDATE SET
            size=excluded.size, mtime=excluded.mtime, hash=excluded.hash,
            title=excluded.title, description=excluded.description, keywords=excluded.keywords,
            categories=excluded.categories, release_filename=excluded.release_filename,
            synced=excluded.synced, updated_at=excluded.updated_at",
        params![
            path, fp.size as i64, fp.mtime, fp.hash as i64,
            meta.title, meta.description, kw, meta.categories, meta.release_filename,
            synced as i64, now
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
        release_filename: String::new(),
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
                release_filename=?5, synced=0, updated_at=?6 WHERE path=?7",
            params![meta.title, meta.description, kw, meta.categories, meta.release_filename, now, path],
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

/// Cancel / revert-to-file: overwrite the record from the file but retain the stored release_filename.
fn revert(conn: &Mutex<Connection>, path: &str) -> Result<StoredMetadata, String> {
    let mut meta = read_file_metadata(path)?;
    {
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(rec) = read_record(&c, path)? {
            meta.release_filename = rec.meta.release_filename; // retained (no file equivalent)
        }
    }
    let fp = fingerprint::compute(Path::new(path))?;
    let c = conn.lock().unwrap_or_else(|e| e.into_inner());
    upsert_record(&c, path, &meta, &fp, true)?;
    Ok(meta)
}

fn sync_after_save(conn: &Mutex<Connection>, old_path: &str, new_path: &str, meta: &StoredMetadata) -> Result<(), String> {
    let fp = fingerprint::compute(Path::new(new_path))?;
    let c = conn.lock().unwrap_or_else(|e| e.into_inner());
    if old_path != new_path {
        let _ = c.execute("DELETE FROM photo_metadata WHERE path=?1", [old_path]);
    }
    upsert_record(&c, new_path, meta, &fp, true)
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

/// Cancel / revert-to-file: restore the record (and the returned fields) from the file, retaining
/// the stored release_filename. Falls back to a plain file read when the store is unavailable.
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
}
