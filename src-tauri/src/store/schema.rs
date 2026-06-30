//! Store schema: one table keyed by photo path. Created on open; WAL for resilience.

use rusqlite::Connection;

/// Apply pragmas and create the table if missing. Idempotent.
pub fn init(conn: &Connection) -> Result<(), rusqlite::Error> {
    // execute_batch tolerates the row that `PRAGMA journal_mode` returns.
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         CREATE TABLE IF NOT EXISTS photo_metadata (
            path             TEXT PRIMARY KEY,
            size             INTEGER NOT NULL,
            mtime            INTEGER NOT NULL,
            hash             INTEGER NOT NULL,
            title            TEXT NOT NULL DEFAULT '',
            description      TEXT NOT NULL DEFAULT '',
            keywords         TEXT NOT NULL DEFAULT '[]',
            categories       TEXT NOT NULL DEFAULT '',
            release_filename TEXT NOT NULL DEFAULT '',
            editorial        INTEGER NOT NULL DEFAULT 0,
            mature_content   INTEGER NOT NULL DEFAULT 0,
            illustration     INTEGER NOT NULL DEFAULT 0,
            synced           INTEGER NOT NULL DEFAULT 1,
            created_at       INTEGER NOT NULL,
            updated_at       INTEGER NOT NULL
         );",
    )?;

    // Migrate databases created before the attribution-flag columns existed. ALTER ... ADD COLUMN
    // errors with "duplicate column name" when already present — ignore that (idempotent).
    for col in ["editorial", "mature_content", "illustration"] {
        let _ = conn.execute(
            &format!("ALTER TABLE photo_metadata ADD COLUMN {col} INTEGER NOT NULL DEFAULT 0"),
            [],
        );
    }
    Ok(())
}
