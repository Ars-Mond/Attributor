// Frontend half of the event contract. Names mirror src-tauri/src/events.rs; payload
// types come from the ts-rs-generated ./generated/events.d.ts (single source of truth).
import {listen, type UnlistenFn} from "@tauri-apps/api/event";
import type {FolderChanged, ThumbnailReady} from "./generated/events";

/** Broadcast event names — keep in sync with src-tauri/src/events.rs constants. */
export const EVENT = {
    folderChanged: "folder-changed",
    thumbnailReady: "thumbnail-ready",
} as const;

/** Map of broadcast event name → payload type, for typed listening. */
export interface EventPayloads {
    "folder-changed": FolderChanged;
    "thumbnail-ready": ThumbnailReady;
}

/** Typed wrapper over Tauri's `listen`: forwards the typed payload to the callback. */
export function listenEvent<K extends keyof EventPayloads>(
    name: K,
    cb: (payload: EventPayloads[K]) => void,
): Promise<UnlistenFn> {
    return listen<EventPayloads[K]>(name, (e) => cb(e.payload));
}

export type {FolderChanged, ThumbnailReady, BatchProgress, ItemStatus, PullProgress} from "./generated/events";
