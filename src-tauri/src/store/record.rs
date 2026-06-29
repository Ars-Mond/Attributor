//! Types crossing the store / IPC boundary. All `camelCase` (Constitution IX).

use serde::{Deserialize, Serialize};

/// The editable metadata fields the store holds — mirrors what the editor edits. `release_filename`
/// lives only in the store (the file pipeline does not carry it).
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StoredMetadata {
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: String,
    pub release_filename: String,
}

/// Whether the stored record matches the file (`Synced`) or holds app-only changes not yet written
/// to the file (`AppOnly`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    Synced,
    AppOnly,
}

/// Outcome of resolving a photo's metadata on open (read-flow). `Resolved` loads directly;
/// `Conflict` carries both versions for the frontend to prompt on (US3).
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MetadataResolution {
    Resolved {
        metadata: StoredMetadata,
        sync_state: SyncState,
    },
    Conflict {
        store: StoredMetadata,
        file: StoredMetadata,
    },
}
