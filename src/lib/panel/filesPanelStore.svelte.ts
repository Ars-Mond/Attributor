import type {FileNode} from '$lib/types';

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
});
