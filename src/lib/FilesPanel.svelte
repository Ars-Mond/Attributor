<script lang="ts">
    import {invoke} from "@tauri-apps/api/core";
    import {convertFileSrc} from "@tauri-apps/api/core";
    import {listen} from "@tauri-apps/api/event";
    import {onMount, onDestroy} from "svelte";
    import FileTree from "./FileTree.svelte";
    import type {FileNode} from "./types";
    import {panelState} from './filesPanelStore.svelte';
    import type {ViewMode, LayoutDir} from './filesPanelStore.svelte';

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

    // contentEl is intentionally local — it's a DOM ref that must be re-bound each mount
    let contentEl = $state<HTMLElement | null>(null);

    function isImageFile(name: string): boolean {
        return /\.(jpg|jpeg|png|webp)$/i.test(name);
    }

    // Horizontal wheel scroll for content/icons modes
    $effect(() => {
        if (!contentEl || panelState.viewMode === 'table' || panelState.layoutDir !== 'horizontal') return;

        function onWheel(e: WheelEvent) {
            e.preventDefault();
            contentEl!.scrollLeft += e.deltaY;
        }

        contentEl.addEventListener('wheel', onWheel, {passive: false});
        return () => contentEl!.removeEventListener('wheel', onWheel);
    });

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
        panelState.selectedPaths = new Set([path]);
        panelState.activePath = path;
        panelState.anchorPath = path;
        onFileSelect(path);
        onSelectionChange?.([path]);
    }

    function doRangeSelect(targetPath: string) {
        const items = getVisibleFilePaths();
        const anchorIdx = items.indexOf(panelState.anchorPath);
        const targetIdx = items.indexOf(targetPath);
        if (anchorIdx === -1 || targetIdx === -1) {
            doSingleSelect(targetPath);
            return;
        }
        const start = Math.min(anchorIdx, targetIdx);
        const end = Math.max(anchorIdx, targetIdx);
        const range = items.slice(start, end + 1);
        panelState.selectedPaths = new Set(range);
        panelState.activePath = targetPath;
        // don't update anchorPath for range selections
        onSelectionChange?.(range);
    }

    function doToggleSelect(path: string) {
        const next = new Set(panelState.selectedPaths);
        if (next.has(path)) {
            next.delete(path);
        } else {
            next.add(path);
        }
        panelState.activePath = path;
        panelState.anchorPath = path;
        panelState.selectedPaths = next;
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
        if (!contentEl || !panelState.fileTree) return;

        // Don't hijack keyboard when user is typing
        const active = document.activeElement;
        if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA')) return;

        e.preventDefault();

        const items = [...contentEl.querySelectorAll<HTMLElement>('[data-path]')];
        if (items.length === 0) return;

        const currentIdx = items.findIndex(el => el.dataset.path === panelState.activePath);
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

        items[nextIdx].scrollIntoView({block: 'nearest', inline: 'nearest'});
    }

    onMount(async () => {
        window.addEventListener('keydown', handleKeyDown);
        unlisten = await listen<string>("folder-changed", () => {
            // Debounce: rapid fs events collapse into one refresh
            if (refreshTimer) clearTimeout(refreshTimer);
            refreshTimer = setTimeout(async () => {
                if (panelState.fileTree) {
                    try {
                        const updated = await invoke<FileNode>("scan_folder", {path: panelState.fileTree.path});
                        panelState.fileTree = updated;

                        const allPaths = flatFilePaths(updated);

                        // Remove no-longer-existing paths from selection
                        const newSelected = new Set([...panelState.selectedPaths].filter(p => allPaths.has(p)));
                        if (newSelected.size !== panelState.selectedPaths.size) {
                            panelState.selectedPaths = newSelected;
                            onSelectionChange?.([...newSelected]);
                        }

                        // Notify parent if the active file disappeared
                        if (panelState.activePath && !allPaths.has(panelState.activePath)) {
                            panelState.activePath = '';
                            panelState.anchorPath = '';
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
                panelState.fileTree = result;
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
            panelState.fileTree = result;
            return true;
        } catch {
            return false;
        }
    }

    /** Reset selection to a single file (used after rename). */
    export function setSelectedPath(path: string) {
        panelState.selectedPaths = new Set([path]);
        panelState.activePath = path;
        panelState.anchorPath = path;
        onSelectionChange?.([path]);
    }
</script>

<aside class="panel panel--files">
    <div class="files-header">
        <span class="files-title">Files</span>
        <div class="view-controls">
            <!-- View mode buttons -->
            <div class="btn-group">
                <button
                    class="view-btn"
                    class:active={panelState.viewMode === 'table'}
                    onclick={() => panelState.viewMode = 'table'}
                    title="Table"
                >
                    <!-- Table: three horizontal lines -->
                    <svg viewBox="0 0 14 14" fill="currentColor">
                        <rect x="1" y="2" width="12" height="2" rx="0.5"/>
                        <rect x="1" y="6" width="12" height="2" rx="0.5"/>
                        <rect x="1" y="10" width="12" height="2" rx="0.5"/>
                    </svg>
                </button>
                <button
                    class="view-btn"
                    class:active={panelState.viewMode === 'content'}
                    onclick={() => panelState.viewMode = 'content'}
                    title="Content"
                >
                    <!-- Content: thumbnail + text row, twice -->
                    <svg viewBox="0 0 14 14" fill="currentColor">
                        <rect x="1" y="1" width="4" height="5" rx="0.5"/>
                        <rect x="6" y="2" width="7" height="1.5" rx="0.5"/>
                        <rect x="6" y="4.5" width="5" height="1.5" rx="0.5"/>
                        <rect x="1" y="8" width="4" height="5" rx="0.5"/>
                        <rect x="6" y="9" width="7" height="1.5" rx="0.5"/>
                        <rect x="6" y="11.5" width="5" height="1.5" rx="0.5"/>
                    </svg>
                </button>
                <button
                    class="view-btn"
                    class:active={panelState.viewMode === 'icons'}
                    onclick={() => panelState.viewMode = 'icons'}
                    title="Icons"
                >
                    <!-- Icons: 2x2 grid -->
                    <svg viewBox="0 0 14 14" fill="currentColor">
                        <rect x="1" y="1" width="5.5" height="5.5" rx="0.5"/>
                        <rect x="7.5" y="1" width="5.5" height="5.5" rx="0.5"/>
                        <rect x="1" y="7.5" width="5.5" height="5.5" rx="0.5"/>
                        <rect x="7.5" y="7.5" width="5.5" height="5.5" rx="0.5"/>
                    </svg>
                </button>
            </div>

            <!-- Layout direction buttons (only for content and icons) -->
            {#if panelState.viewMode !== 'table'}
                <div class="btn-group">
                    <button
                        class="view-btn"
                        class:active={panelState.layoutDir === 'vertical'}
                        onclick={() => panelState.layoutDir = 'vertical'}
                        title="Vertical"
                    >
                        <!-- Vertical: three horizontal bars (stack top-down) -->
                        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
                            <line x1="2" y1="3" x2="12" y2="3"/>
                            <line x1="2" y1="7" x2="12" y2="7"/>
                            <line x1="2" y1="11" x2="12" y2="11"/>
                        </svg>
                    </button>
                    <button
                        class="view-btn"
                        class:active={panelState.layoutDir === 'horizontal'}
                        onclick={() => panelState.layoutDir = 'horizontal'}
                        title="Horizontal"
                    >
                        <!-- Horizontal: three vertical bars (stack left-right) -->
                        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
                            <line x1="3" y1="2" x2="3" y2="12"/>
                            <line x1="7" y1="2" x2="7" y2="12"/>
                            <line x1="11" y1="2" x2="11" y2="12"/>
                        </svg>
                    </button>
                </div>
            {/if}
        </div>
    </div>

    <div
        class="panel-content files-content"
        class:files-content--icons={panelState.viewMode === 'icons'}
        class:files-content--horizontal={panelState.viewMode !== 'table' && panelState.layoutDir === 'horizontal'}
        bind:this={contentEl}
    >
        {#if panelState.fileTree}
            {#if panelState.viewMode === 'icons'}
                <!-- Flat list of image files from the root level only -->
                {#each panelState.fileTree.children.filter(c => !c.is_dir && isImageFile(c.name)) as node (node.path)}
                    <button
                        class="icon-item"
                        class:selected={panelState.selectedPaths.has(node.path)}
                        class:active={panelState.activePath === node.path}
                        class:icon-item--horizontal={panelState.layoutDir === 'horizontal'}
                        data-path={node.path}
                        title={node.name}
                        onclick={(e) => handleTreeSelect(node.path, e)}
                    >
                        <img class="icon-thumb" src={convertFileSrc(node.path)} alt={node.name} />
                        <span class="icon-overlay">{node.name}</span>
                    </button>
                {/each}
            {:else}
                <!-- Table / content mode: recursive tree -->
                {#each panelState.fileTree.children as child (child.path)}
                    <FileTree
                        node={child}
                        depth={0}
                        selectedPaths={panelState.selectedPaths}
                        activePath={panelState.activePath}
                        viewMode={panelState.viewMode}
                        layoutDir={panelState.layoutDir}
                        onSelect={handleTreeSelect}
                    />
                {/each}
            {/if}
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
        @include flex(row, space-between, center);
        padding: 6px 8px 6px 12px;
        border-bottom: 1px solid $border;
        flex-shrink: 0;
        gap: 8px;
    }

    .files-title {
        font-size: $fs-small;
        font-weight: 600;
        letter-spacing: 0.04em;
        color: $text-secondary;
        text-transform: uppercase;
        flex-shrink: 0;
    }

    // ── View / layout controls ──

    .view-controls {
        @include flex(row, flex-end, center);
        gap: 4px;
    }

    .btn-group {
        @include flex(row, flex-start, center);
        border: 1px solid $border;
        border-radius: $radius-sm;
        overflow: hidden;
    }

    .view-btn {
        @include btn-reset;
        @include flex(row, center, center);
        @include transition(background, color);
        width: 26px;
        height: 24px;
        color: $text-muted;

        svg {
            width: 14px;
            height: 14px;
            flex-shrink: 0;
        }

        & + & {
            border-left: 1px solid $border;
        }

        &:hover { background: var(--hover-bg); color: $text; }
        &.active { background: $chip-bg; color: $chip-text; }
    }

    // ── Content area ──

    .files-content {
        padding: 6px 4px;
        gap: 1px;

        // Icons mode: vertical stack, each item fills full width
        &--icons {
            flex-direction: column;
            flex-wrap: nowrap;
            align-items: stretch;
            padding: 4px;
            gap: 2px;
        }

        // Horizontal layout: single row, scroll sideways, items fill full height
        &--horizontal {
            flex-direction: row !important;
            flex-wrap: nowrap !important;
            overflow-x: auto;
            overflow-y: hidden;
            align-items: stretch;
            padding: 6px;
            gap: 2px;
            @include scrollbar;
        }
    }

    // ── Icon items ──

    .icon-item {
        @include btn-reset;
        @include transition(background, color);
        padding: 2px;
        border-radius: $radius-sm;
        position: relative;
        overflow: hidden;
        color: $text-secondary;
        flex-shrink: 0;

        &:hover { background: var(--hover-bg); color: $text; }

        &.selected {
            outline: 2px solid $accent;
            outline-offset: -2px;
        }

        &.active {
            outline: 2px solid $accent;
            outline-offset: -2px;
        }

        // Horizontal layout: fill full container height, auto width
        &--horizontal {
            height: 100%;
        }
    }

    .icon-thumb {
        display: block;
        object-fit: contain;
        border-radius: $radius-sm;
        // Vertical: fill full width of item
        width: 100%;
        height: auto;

        .icon-item--horizontal & {
            // Horizontal: fill full height of item
            width: auto;
            height: 100%;
        }
    }

    .icon-overlay {
        position: absolute;
        bottom: 0;
        left: 0;
        right: 0;
        padding: 4px 6px;
        background: rgba(0, 0, 0, 0.6);
        color: #fff;
        font-size: $fs-small;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        text-align: left;
        opacity: 0;
        transition: opacity 0.15s ease;
        border-bottom-left-radius: $radius-sm;
        border-bottom-right-radius: $radius-sm;
        pointer-events: none;

        .icon-item:hover & {
            opacity: 1;
        }
    }

    // ── Empty state ──

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
