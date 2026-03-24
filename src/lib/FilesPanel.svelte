<script lang="ts">
    import {invoke} from "@tauri-apps/api/core";
    import {listen} from "@tauri-apps/api/event";
    import {onMount, onDestroy} from "svelte";
    import FileTree from "./FileTree.svelte";
    import type {FileNode} from "./types";

    let {
        onFileSelect,
        onFileGone,
        onFolderOpen,
        onBusy,
        onSelectionChange,
        disabled = false,
    }: {
        onFileSelect: (path: string) => void;
        onFileGone?: () => void;
        onFolderOpen?: (path: string) => void;
        onBusy?: (busy: boolean) => void;
        onSelectionChange?: (paths: string[]) => void;
        disabled?: boolean;
    } = $props();

    // --- State ---
    let fileTree = $state<FileNode | null>(null);
    let selectedPaths = $state<Set<string>>(new Set());
    let activePath = $state('');
    let anchorPath = ''; // anchor for shift-click range (plain var, no reactivity needed)
    let contentEl = $state<HTMLElement | null>(null);

    // ── Folder watching ──────────────────────────────────────────────────

    let unlisten: (() => void) | null = null;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;

    /** Collect all non-directory paths from the tree recursively. */
    function flatFilePaths(node: FileNode): Set<string> {
        const set = new Set<string>();
        function walk(n: FileNode) {
            if (!n.is_dir) set.add(n.path);
            for (const c of n.children) walk(c);
        }
        walk(node);
        return set;
    }

    // ── Selection helpers ─────────────────────────────────────────────────

    function getVisibleFilePaths(): string[] {
        if (!contentEl) return [];
        return [...contentEl.querySelectorAll<HTMLElement>('[data-path]')]
            .map(el => el.dataset.path!);
    }

    function doSingleSelect(path: string) {
        selectedPaths = new Set([path]);
        activePath = path;
        anchorPath = path;
        onFileSelect(path);
        onSelectionChange?.([path]);
    }

    function doRangeSelect(targetPath: string) {
        const items = getVisibleFilePaths();
        const anchorIdx = items.indexOf(anchorPath);
        const targetIdx = items.indexOf(targetPath);
        if (anchorIdx === -1 || targetIdx === -1) {
            doSingleSelect(targetPath);
            return;
        }
        const start = Math.min(anchorIdx, targetIdx);
        const end = Math.max(anchorIdx, targetIdx);
        const range = items.slice(start, end + 1);
        selectedPaths = new Set(range);
        activePath = targetPath;
        // don't update anchorPath for range selections
        onSelectionChange?.(range);
    }

    function doToggleSelect(path: string) {
        const next = new Set(selectedPaths);
        if (next.has(path)) {
            next.delete(path);
        } else {
            next.add(path);
        }
        activePath = path;
        anchorPath = path;
        selectedPaths = next;
        const arr = [...next];
        onSelectionChange?.(arr);
        if (arr.length === 1) {
            onFileSelect(arr[0]);
        }
    }

    function handleTreeSelect(path: string, e: MouseEvent) {
        if (disabled) return;
        if (e.shiftKey) {
            doRangeSelect(path);
        } else if (e.ctrlKey || e.metaKey) {
            doToggleSelect(path);
        } else {
            doSingleSelect(path);
        }
    }

    function handleKeyDown(e: KeyboardEvent) {
        if (disabled) return;
        if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
        if (!contentEl || !fileTree) return;

        // Don't hijack keyboard when user is typing
        const active = document.activeElement;
        if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA')) return;

        e.preventDefault();

        const items = [...contentEl.querySelectorAll<HTMLElement>('[data-path]')];
        if (items.length === 0) return;

        const currentIdx = items.findIndex(el => el.dataset.path === activePath);
        const nextIdx = e.key === 'ArrowDown'
            ? (currentIdx === -1 ? 0 : Math.min(currentIdx + 1, items.length - 1))
            : (currentIdx === -1 ? 0 : Math.max(currentIdx - 1, 0));

        if (nextIdx === currentIdx && currentIdx !== -1) return;

        const path = items[nextIdx].dataset.path!;

        if (e.shiftKey) {
            doRangeSelect(path);
        } else {
            doSingleSelect(path);
        }

        items[nextIdx].scrollIntoView({block: 'nearest'});
    }

    onMount(async () => {
        window.addEventListener('keydown', handleKeyDown);
        unlisten = await listen<string>("folder-changed", () => {
            // Debounce: rapid fs events collapse into one refresh
            if (refreshTimer) clearTimeout(refreshTimer);
            refreshTimer = setTimeout(async () => {
                if (fileTree) {
                    try {
                        const updated = await invoke<FileNode>("scan_folder", {path: fileTree.path});
                        fileTree = updated;

                        const allPaths = flatFilePaths(updated);

                        // Remove no-longer-existing paths from selection
                        const newSelected = new Set([...selectedPaths].filter(p => allPaths.has(p)));
                        if (newSelected.size !== selectedPaths.size) {
                            selectedPaths = newSelected;
                            onSelectionChange?.([...newSelected]);
                        }

                        // Notify parent if the active file disappeared
                        if (activePath && !allPaths.has(activePath)) {
                            activePath = '';
                            anchorPath = '';
                            onFileGone?.();
                        }
                    } catch (e) {
                        console.error("scan_folder failed:", e);
                    }
                }
            }, 400);
        });
    });

    onDestroy(() => {
        window.removeEventListener('keydown', handleKeyDown);
        unlisten?.();
        if (refreshTimer) clearTimeout(refreshTimer);
    });

    // ── Actions ──────────────────────────────────────────────────────────

    async function openFolder() {
        onBusy?.(true);
        try {
            const result = await invoke<FileNode | null>("open_folder");
            if (result) {
                fileTree = result;
                onFolderOpen?.(result.path);
            }
        } finally {
            onBusy?.(false);
        }
    }

    /** Open a folder using the OS dialog. */
    export async function openFolderDialog() {
        await openFolder();
    }

    /** Open a folder by path without a dialog (used to restore last session). */
    export async function openFolderByPath(path: string): Promise<boolean> {
        try {
            const result = await invoke<FileNode>("open_folder_path", {path});
            fileTree = result;
            return true;
        } catch {
            return false;
        }
    }

    /** Reset selection to a single file (used after rename). */
    export function setSelectedPath(path: string) {
        selectedPaths = new Set([path]);
        activePath = path;
        anchorPath = path;
        onSelectionChange?.([path]);
    }
</script>

<aside class="panel panel--files">
    <div class="files-header">
        <span class="files-title">Files</span>
    </div>

    <div class="panel-content files-content" bind:this={contentEl}>
        {#if fileTree}
            <!-- Skip root node, show its children directly -->
            {#each fileTree.children as child (child.path)}
                <FileTree
                    node={child}
                    depth={0}
                    {selectedPaths}
                    {activePath}
                    onSelect={handleTreeSelect}
                />
            {/each}
        {:else}
            <div class="files-empty">
                <svg width="36" height="36" viewBox="0 0 16 16" fill="currentColor">
                    <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h2.764c.958 0 1.76.56 2.311 1.184C7.985 3.648 8.48 4 9 4h4.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z"/>
                </svg>
                <p>No folder open</p>
            </div>
        {/if}
    </div>
</aside>

<style lang="scss">
    @use '../styles/mixins' as *;

    .panel--files {
        border-left: 1px solid $border;
        border-right: none;
    }

    .files-header {
        @include flex(row, flex-start, center);
        padding: 10px 12px;
        border-bottom: 1px solid $border;
        flex-shrink: 0;
    }

    .files-title {
        font-size: $fs-small;
        font-weight: 600;
        letter-spacing: 0.04em;
        color: $text-secondary;
        text-transform: uppercase;
    }


    .files-content {
        padding: 6px 4px;
        gap: 1px;
    }

    .files-empty {
        @include flex(column, center, center);
        gap: 10px;
        padding: 40px 16px;
        color: $text-muted;
        opacity: 0.5;
        text-align: center;

        p { font-size: $fs-small; }
    }
</style>
