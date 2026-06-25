import type {FileNode} from '$lib/types';
import {SvelteSet} from 'svelte/reactivity';

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
});
