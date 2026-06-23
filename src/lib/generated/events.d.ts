// AUTO-GENERATED from src-tauri/src/events.rs via ts-rs. Do not edit by hand.
// Regenerate: UPDATE_EVENTS_CONTRACT=1 cargo test events_contract --manifest-path src-tauri/Cargo.toml

export type FolderChanged = { path: string, };
export type ThumbnailReady = { path: string, };
export type ItemStatus = { "kind": "ok", path: string, } | { "kind": "failed", error: string, } | { "kind": "cancelled" };
export type BatchProgress = { 
/**
 * Index into the input items list; the frontend keys UI by this (order-independent).
 */
index: number, status: ItemStatus, };
