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
    // Attribution flags. Like `release_filename`, they have no file-side equivalent and live only in
    // the store (retained when resolving from the file / on revert).
    pub editorial: bool,
    pub mature_content: bool,
    pub illustration: bool,
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
///
/// `rename_all_fields` is required so struct-variant fields (e.g. `sync_state`) are camelCased to
/// `syncState` — the enum-level `rename_all` only renames the variant *names*, not their fields.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_serializes_camelcase_fields() {
        let r = MetadataResolution::Resolved {
            metadata: StoredMetadata::default(),
            sync_state: SyncState::AppOnly,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"resolved\""), "{json}");
        assert!(json.contains("\"syncState\":\"appOnly\""), "{json}");
        assert!(!json.contains("sync_state"), "{json}");
    }
}
