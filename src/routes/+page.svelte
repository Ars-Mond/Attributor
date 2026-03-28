<script lang="ts">
    import {onMount, onDestroy} from "svelte";
    import {convertFileSrc} from "@tauri-apps/api/core";
    import {getCurrentWindow, PhysicalSize} from "@tauri-apps/api/window";
    import MetadataPanel from "$lib/MetadataPanel.svelte";
    import FilesPanel from "$lib/FilesPanel.svelte";
    import UnsavedChangesDialog from "$lib/UnsavedChangesDialog.svelte";
    import {loadAppState, saveAppState} from "$lib/store";
    import {themes, applyTheme, DEFAULT_THEME} from "$lib/themes";
    import DockLayout from "$lib/docking/DockLayout.svelte";
    import DockToolbar from "$lib/docking/DockToolbar.svelte";
    import type {LayoutNode, WindowConfig} from "$lib/docking/dockTypes";
    import {getDefaultLayout, removePanel, addPanelToRoot, findPanel, findSavePoint, insertPanel, serializeLayout, deserializeLayout} from "$lib/docking/dockStore";
    import type {PanelSavePoint} from "$lib/docking/dockStore";
    import MenuBar from "$lib/menu/MenuBar.svelte";
    import MenuTab from "$lib/menu/MenuTab.svelte";
    import MenuItem from "$lib/menu/MenuItem.svelte";
    import MenuSeparator from "$lib/menu/MenuSeparator.svelte";
    import AboutDialog from "$lib/AboutDialog.svelte";

    // --- Docking ---
    const windowConfigs: WindowConfig[] = [
        {id: 'control', title: 'Control', closable: true},
        {id: 'view', title: 'View', closable: false},
        {id: 'hierarchy', title: 'Hierarchy', closable: true},
    ];

    let layout = $state<LayoutNode>(getDefaultLayout());
    let hiddenWindows = $state<string[]>([]);
    // Saved positions for closed panels — plain variable, no reactivity needed
    let savedPositions: Record<string, PanelSavePoint> = {};

    function handleLayoutChange(newLayout: LayoutNode) {
        layout = newLayout;
        persistDock();
    }

    function handleCloseWindow(windowId: string) {
        // Save position before removal so we can restore it later
        const sp = findSavePoint(layout, windowId);
        if (sp) savedPositions[windowId] = sp;

        const result = removePanel(layout, windowId);
        if (result) {
            layout = result;
            hiddenWindows = [...hiddenWindows, windowId];
            persistDock();
        }
    }

    function handleShowWindow(windowId: string) {
        const sp = savedPositions[windowId];
        if (sp && findPanel(layout, sp.neighborId)) {
            // Restore to saved position with original size
            layout = insertPanel(layout, sp.neighborId, windowId, sp.zone, sp.size);
            delete savedPositions[windowId];
        } else {
            layout = addPanelToRoot(layout, windowId);
        }
        hiddenWindows = hiddenWindows.filter(id => id !== windowId);
        persistDock();
    }

    function persistDock() {
        saveAppState({
            dockLayout: serializeLayout(layout),
            dockHiddenWindows: JSON.stringify(hiddenWindows),
        });
    }

    // --- Image viewer ---
    let imageSrc = $state<string | null>(null);

    // --- Panel bindings ---
    let metaPanel: any = $state(null);
    let filesPanel: any = $state(null);
    let isDirty = $state(false);
    let isLoading = $state(false);
    let showAbout = $state(false);
    let currentTheme = $state(DEFAULT_THEME);
    let batchPaths = $state<string[]>([]);

    // --- Unsaved changes dialog ---
    let showDialog = $state(false);
    let pendingPath = $state<string | null>(null);
    let currentPath = $state<string | null>(null);

    const currentBasename = $derived(
        currentPath ? currentPath.replace(/\\/g, '/').split('/').pop() ?? currentPath : ''
    );

    // --- File-gone toast ---
    let goneMessage = $state<string | null>(null);
    let goneTimer: ReturnType<typeof setTimeout> | null = null;

    function showGoneToast(name: string) {
        if (goneTimer) clearTimeout(goneTimer);
        goneMessage = name;
        goneTimer = setTimeout(() => {
            goneMessage = null;
        }, 6000);
    }

    /** Called by FilesPanel when the open file disappears from the folder. */
    function handleFileGone() {
        const name = currentBasename;
        metaPanel?.clear();
        imageSrc = null;
        currentPath = null;
        showDialog = false;
        pendingPath = null;
        showGoneToast(name);
    }

    /** Update viewer when the file was renamed during save. */
    function handlePathChange(newPath: string) {
        currentPath = newPath;
        imageSrc = convertFileSrc(newPath);
        filesPanel?.setSelectedPath(newPath);
    }

    /** Actually open a file: load into metadata panel + show in viewer. */
    async function openFile(path: string) {
        await metaPanel?.loadFile(path);
        imageSrc = convertFileSrc(path);
        currentPath = path;
        showDialog = false;
        pendingPath = null;
        saveAppState({lastFile: path});
    }

    /** Called from FilesPanel when user clicks a file. */
    async function handleFileSelect(path: string) {
        if (isDirty && batchPaths.length <= 1) {
            pendingPath = path;
            showDialog = true;
        } else {
            await openFile(path);
        }
    }

    /** Called when selection changes (single or multi). */
    function handleSelectionChange(paths: string[]) {
        batchPaths = paths;
        if (paths.length > 1) {
            imageSrc = convertFileSrc(paths[paths.length - 1]);
        }
    }

    /** Called when Alt+click on a photo in batch mode — preview only. */
    function handleAltSelect(path: string) {
        imageSrc = convertFileSrc(path);
    }

    // --- Dialog actions ---

    async function handleDialogDiscard() {
        if (pendingPath) await openFile(pendingPath);
    }

    async function handleDialogSave() {
        try {
            await metaPanel?.save();
            if (pendingPath) await openFile(pendingPath);
        } catch {
            // Validation failed — close dialog so the user can fill required fields
            showDialog = false;
            pendingPath = null;
        }
    }

    function handleDialogCancel() {
        showDialog = false;
        pendingPath = null;
        filesPanel?.setSelectedPath(currentPath ?? '');
    }

    // --- Session restore & window state ---

    let unlistenResize: (() => void) | null = null;
    let winResizeTimer: ReturnType<typeof setTimeout> | null = null;

    onMount(async () => {
        const win = getCurrentWindow();
        const state = await loadAppState();

        // 0. Restore theme
        if (state.theme) {
            currentTheme = state.theme;
            applyTheme(state.theme);
        } else {
            applyTheme(DEFAULT_THEME);
        }

        // 1. Restore window size / maximize
        if (state.windowMaximized) {
            await win.maximize();
        } else if (state.windowWidth && state.windowHeight) {
            await win.setSize(new PhysicalSize(state.windowWidth, state.windowHeight));
        }

        // 2. Restore dock layout
        if (state.dockLayout) {
            const restored = deserializeLayout(state.dockLayout);
            if (restored) layout = restored;
        }
        if (state.dockHiddenWindows) {
            try {
                const parsed = JSON.parse(state.dockHiddenWindows);
                if (Array.isArray(parsed)) hiddenWindows = parsed;
            } catch { /* ignore */ }
        }

        // 3. Restore last folder, then last file
        if (state.lastFolder) {
            const ok = await filesPanel?.openFolderByPath(state.lastFolder);
            if (ok && state.lastFile) {
                try {
                    await openFile(state.lastFile);
                    filesPanel?.setSelectedPath(state.lastFile);
                } catch {
                    // File no longer exists — ignore
                }
            }
        }

        // 4. Show window after full UI init
        await win.show();

        // Save window size whenever it changes (debounced)
        unlistenResize = await win.onResized(async () => {
            if (winResizeTimer) clearTimeout(winResizeTimer);
            winResizeTimer = setTimeout(async () => {
                const maximized = await win.isMaximized();
                if (maximized) {
                    saveAppState({windowMaximized: true});
                } else {
                    const size = await win.outerSize();
                    saveAppState({windowMaximized: false, windowWidth: size.width, windowHeight: size.height});
                }
            }, 500);
        });
    });

    onDestroy(() => {
        unlistenResize?.();
        if (winResizeTimer) clearTimeout(winResizeTimer);
    });
</script>

<div class="app">
    <MenuBar>
        <MenuTab label="File">
            <MenuItem label="Open directory..." onClick={() => filesPanel?.openFolderDialog()} />
            <MenuSeparator />
            <MenuTab label="Theme">
                {#each themes as t}
                    <MenuItem
                        label={t.name}
                        onClick={() => { currentTheme = t.id; applyTheme(t.id); saveAppState({theme: t.id}); }}
                    />
                {/each}
            </MenuTab>
            <MenuSeparator />
            <MenuItem label="Settings" onClick={() => {}} />
        </MenuTab>
        <MenuTab label="Windows">
            <MenuItem
                label="Show Control"
                onClick={() => handleShowWindow('control')}
                disabled={!hiddenWindows.includes('control')}
            />
            <MenuItem
                label="Show Hierarchy"
                onClick={() => handleShowWindow('hierarchy')}
                disabled={!hiddenWindows.includes('hierarchy')}
            />
        </MenuTab>
        <MenuTab label="Help">
            <MenuItem label="Help?" onClick={() => {}} />
            <MenuItem label="About" onClick={() => { showAbout = true; }} />
        </MenuTab>
    </MenuBar>

<!--    <DockToolbar-->
<!--        {hiddenWindows}-->
<!--        {windowConfigs}-->
<!--        onShowWindow={handleShowWindow}-->
<!--    />-->

    <DockLayout
        bind:layout
        {windowConfigs}
        onClose={handleCloseWindow}
        onLayoutChange={handleLayoutChange}
    >
        {#snippet renderWindow(windowId)}
            {#if windowId === 'control'}
                <MetadataPanel bind:this={metaPanel} bind:isDirty onPathChange={handlePathChange} {batchPaths} />
            {:else if windowId === 'view'}
                <div class="viewer">
                    {#if imageSrc}
                        <img class="image" src={imageSrc} alt="Preview" />
                    {:else}
                        <div class="placeholder">
                            <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2">
                                <rect x="3" y="3" width="18" height="18" rx="2" />
                                <circle cx="8.5" cy="8.5" r="1.5" />
                                <path d="M21 15l-5-5L5 21" />
                            </svg>
                            <p>No image open</p>
                        </div>
                    {/if}

                    {#if goneMessage}
                        <div class="gone-toast" role="alert">
                            <svg class="gone-icon" viewBox="0 0 20 20" fill="currentColor">
                                <path fill-rule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd"/>
                            </svg>
                            <span>
                                <strong>{goneMessage}</strong> was moved or deleted externally.
                            </span>
                            <button class="gone-close" onclick={() => {goneMessage = null;}} aria-label="Dismiss">×</button>
                        </div>
                    {/if}
                </div>
            {:else if windowId === 'hierarchy'}
                <FilesPanel
                    bind:this={filesPanel}
                    onFileSelect={handleFileSelect}
                    onFileGone={handleFileGone}
                    onFolderOpen={(path) => saveAppState({lastFolder: path})}
                    onBusy={(b) => (isLoading = b)}
                    onSelectionChange={handleSelectionChange}
                    onAltSelect={handleAltSelect}
                    disabled={showDialog}
                />
            {/if}
        {/snippet}
    </DockLayout>
</div>

<!-- Loading overlay -->
{#if isLoading}
    <div class="loading-overlay" aria-hidden="true">
        <div class="spinner"></div>
    </div>
{/if}

<!-- Unsaved changes dialog -->
{#if showAbout}
    <AboutDialog onClose={() => { showAbout = false; }} />
{/if}

{#if showDialog}
    <UnsavedChangesDialog
        filename={currentBasename}
        onDiscard={handleDialogDiscard}
        onSave={handleDialogSave}
        onCancel={handleDialogCancel}
    />
{/if}

<style lang="scss">
    @use '../styles/mixins' as *;

    .app {
        @include flex(column, flex-start, stretch);
        height: 100vh;
        overflow: hidden;
    }

    .viewer {
        @include flex(row, center, center, 1);
        overflow: hidden;
        background: $bg-app;
        min-width: 0;
        min-height: 0;
        position: relative;
    }

    .image {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        display: block;
    }

    .placeholder {
        @include flex(column, center, center);
        gap: 12px;
        color: $text-muted;

        svg {opacity: 0.4;}
        p {font-size: $fs-regular;}
    }

    // File-gone toast
    .gone-toast {
        position: absolute;
        bottom: 20px;
        left: 50%;
        transform: translateX(-50%);
        @include flex(row, flex-start, center);
        gap: 10px;
        padding: 10px 14px;
        background: #2a1f10;
        border: 1px solid #92400e;
        border-radius: $radius-md;
        color: #fcd34d;
        font-size: $fs-small;
        max-width: 420px;
        width: max-content;
        box-shadow: 0 4px 20px var(--shadow);
        animation: toast-in 0.2s ease;

        strong {color: #fde68a;}
    }

    .gone-icon {
        width: 18px;
        height: 18px;
        flex-shrink: 0;
        color: #f59e0b;
    }

    .gone-close {
        @include btn-reset;
        margin-left: auto;
        padding: 0 2px;
        font-size: 18px;
        line-height: 1;
        color: #92400e;
        opacity: 0.7;
        flex-shrink: 0;
        @include transition(opacity);

        &:hover {opacity: 1;}
    }

    @keyframes toast-in {
        from {opacity: 0; transform: translateX(-50%) translateY(8px);}
        to {opacity: 1; transform: translateX(-50%) translateY(0);}
    }

    // Loading overlay
    .loading-overlay {
        position: fixed;
        inset: 0;
        z-index: 200;
        backdrop-filter: blur(4px);
        background: var(--overlay-loading);
        pointer-events: all;
        @include flex(row, center, center);
    }

    .spinner {
        width: 30px;
        height: 30px;
        border-radius: 50%;
        border: 2.5px solid var(--spinner-track);
        border-top-color: var(--spinner-color);
        animation: spin 0.65s linear infinite;
    }

    @keyframes spin {
        to {transform: rotate(360deg);}
    }
</style>
