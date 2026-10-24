import type {FileNode} from '$lib/types';
import {SvelteSet} from 'svelte/reactivity';
import {settings} from '$lib/settings';

export type ViewMode = 'table' | 'content' | 'icons';
export type LayoutDir = 'vertical' | 'horizontal';

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
