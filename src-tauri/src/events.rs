//! Backend→frontend event contract: the single source of truth for event names and
//! payload types. Emission stays in the domain modules that own each signal; this module
//! only defines the names and the serde-camelCase payload shapes. Under `cfg(test)`,
//! `ts-rs` exports the matching TypeScript to `src/lib/generated/events.d.ts`, and the
//! `events_contract` test guards against drift.

use serde::Serialize;

// ── Broadcast event names (global app.emit) ───────────────────────────────────

/// Emitted when the open folder changes on disk (payload: [`FolderChanged`]).
pub const FOLDER_CHANGED: &str = "folder-changed";
/// Emitted when a thumbnail finishes generating (payload: [`ThumbnailReady`]).
pub const THUMBNAIL_READY: &str = "thumbnail-ready";

// ── Broadcast payloads ────────────────────────────────────────────────────────

/// Payload of the `folder-changed` broadcast.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FolderChanged {
    pub path: String,
}

/// Payload of the `thumbnail-ready` broadcast.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ThumbnailReady {
    pub path: String,
}

// ── Batch save progress (per-call tauri::ipc::Channel) ────────────────────────

/// One file's outcome in a batch save.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ItemStatus {
    /// Saved successfully; `path` is the final file path.
    Ok { path: String },
    /// Write failed; `error` is a human-readable reason.
    Failed { error: String },
    /// The file had not started when cancellation was observed.
    Cancelled,
}

/// Incremental progress message streamed over the batch Channel; one per file.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct BatchProgress {
    /// Index into the input items list; the frontend keys UI by this (order-independent).
    pub index: usize,
    pub status: ItemStatus,
}

/// Drift guard: the committed `src/lib/generated/events.d.ts` must match the TypeScript
/// derived from the Rust contract types. A name/field change here without regenerating fails
/// this test. Regenerate with `UPDATE_EVENTS_CONTRACT=1 cargo test events_contract`.
#[cfg(test)]
mod tests {
    use super::*;
    use ts_rs::TS;

    fn generated_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/generated/events.d.ts")
    }

    fn render() -> String {
        let mut out = String::from(
            "// AUTO-GENERATED from src-tauri/src/events.rs via ts-rs. Do not edit by hand.\n\
             // Regenerate: UPDATE_EVENTS_CONTRACT=1 cargo test events_contract --manifest-path src-tauri/Cargo.toml\n\n",
        );
        for decl in [
            FolderChanged::decl(),
            ThumbnailReady::decl(),
            ItemStatus::decl(),
            BatchProgress::decl(),
        ] {
            out.push_str("export ");
            out.push_str(&decl);
            out.push('\n');
        }
        out
    }

    #[test]
    fn events_contract_matches_generated_types() {
        let path = generated_path();
        let expected = render();

        if std::env::var("UPDATE_EVENTS_CONTRACT").is_ok() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &expected).unwrap();
            return;
        }

        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| {
                panic!("missing {} — run with UPDATE_EVENTS_CONTRACT=1", path.display())
            })
            .replace("\r\n", "\n");
        assert_eq!(
            committed.trim(),
            expected.trim(),
            "events.d.ts is stale — regenerate with UPDATE_EVENTS_CONTRACT=1"
        );
    }
}
