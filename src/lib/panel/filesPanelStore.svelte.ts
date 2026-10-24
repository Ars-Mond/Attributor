import type {FileNode} from '$lib/types';
import {SvelteSet} from 'svelte/reactivity';
import {settings} from '$lib/settings';
import {saveAppState, type AppState} from '$lib/store';

export type ViewMode = 'table' | 'content' | 'icons';
export type LayoutDir = 'vertical' | 'horizontal';

const VIEW_MODES: readonly string[] = ['table', 'content', 'icons'] satisfies ViewMode[];
const LAYOUT_DIRS: readonly string[] = ['vertical', 'horizontal'] satisfies LayoutDir[];

/** Persistent state for FilesPanel — survives component unmount/remount (e.g. dock drag). */
export const panelState = $state({
    fileTree: null as FileNode | null,
    selectedPaths: new Set<string>(),
    activePath: '',
    anchorPath: '',
    viewMode: 'table' as ViewMode,
    layoutDir: 'vertical' as LayoutDir,
    // Source photo paths whose thumbnail is ready (driven by the `thumbnail-ready` event).
    readyThumbs: new SvelteSet<string>(),
    // True while a batch metadata save runs — suppresses the watcher-driven folder rescan.
    batchInProgress: false,
    // The same for MCP writes of many files; kept apart so the two operations never end each other's pause.
    mcpWriting: false,
});

/** Switches the file list's view mode and remembers it for the next start. */
export function setViewMode(mode: ViewMode): void {
    panelState.viewMode = mode;
    saveAppState({filesViewMode: mode});
}

/** Switches the layout direction of the Content and Icons modes and remembers it for the next start. */
export function setLayoutDir(dir: LayoutDir): void {
    panelState.layoutDir = dir;
    saveAppState({filesLayoutDir: dir});
}

/** Applies the view mode and layout direction saved by a previous run; missing or unknown values keep the defaults. */
export function restoreFilesView(state: Partial<AppState>): void {
    if (state.filesViewMode && VIEW_MODES.includes(state.filesViewMode)) panelState.viewMode = state.filesViewMode as ViewMode;
    if (state.filesLayoutDir && LAYOUT_DIRS.includes(state.filesLayoutDir)) panelState.layoutDir = state.filesLayoutDir as LayoutDir;
}

/** Eager-generation config sent to the backend, derived from the cache + folder settings.
 *  Small (low) thumbnails are always generated up front; only the large (high) thumbnail is deferred by lazy caching.
 *  `recursive` follows the general "Read nested folders" setting. */
export function cacheGenConfig() {
    const lazy = settings.get<boolean>('cache.lazy');
    return {
        low: settings.get<boolean>('cache.smallThumbnails'),
        high: !lazy && settings.get<boolean>('cache.photo'),
        recursive: settings.get<boolean>('general.nestedFolders'),
    };
}
