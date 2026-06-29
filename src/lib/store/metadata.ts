// Typed invoke wrappers for the intermediate metadata store (feature 008).
import {invoke} from '@tauri-apps/api/core';
import type {MetadataResolution, StoredMetadata, SyncState} from '$lib/types';

/** Store-first resolution of one photo's metadata (read-flow). */
export function openMetadata(path: string): Promise<MetadataResolution> {
    return invoke('open_metadata', {path});
}

/** Persist the working fields to the store as app-only (file untouched). Returns the new sync state. */
export function storeMetadata(path: string, fields: StoredMetadata): Promise<SyncState> {
    return invoke('store_metadata', {path, fields});
}

/** Cancel / revert-to-file: restore the record from the file (retaining the stored releaseFilename). */
export function revertToFile(path: string): Promise<StoredMetadata> {
    return invoke('revert_to_file', {path});
}
