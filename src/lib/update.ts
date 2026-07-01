// Update check (notify-only) — queries GitHub releases via the Rust command. Never downloads or
// installs; it only reports whether a newer version exists.
import {invoke} from '@tauri-apps/api/core';

export interface UpdateInfo {
    available: boolean;
    currentVersion: string;
    latestVersion: string;
    url: string;
    notes: string;
}

export function checkForUpdate(): Promise<UpdateInfo> {
    return invoke('check_for_update');
}
