<script lang="ts">
    import {onMount, onDestroy} from "svelte";
    import {convertFileSrc, invoke} from "@tauri-apps/api/core";
    import {getCurrentWindow, PhysicalSize} from "@tauri-apps/api/window";
    import MetadataPanel from "$lib/panel/MetadataPanel.svelte";
    import FilesPanel from "$lib/panel/FilesPanel.svelte";
    import UnsavedChangesDialog from "$lib/dialog/UnsavedChangesDialog.svelte";
    import {loadAppState, saveAppState} from "$lib/store";
    import {applyTheme, applyFontScale} from "$lib/themes";
    import DockLayout from "$lib/docking/DockLayout.svelte";
    import type {LayoutNode, WindowConfig} from "$lib/docking/dockTypes";
    import {getDefaultLayout, removePanel, addPanelToRoot, findPanel, findSavePoint, insertPanel, serializeLayout, deserializeLayout} from "$lib/docking/dockStore";
    import type {PanelSavePoint} from "$lib/docking/dockStore";
    import MenuBar from "$lib/menu/MenuBar.svelte";
    import MenuTab from "$lib/menu/MenuTab.svelte";
    import MenuItem from "$lib/menu/MenuItem.svelte";
    import MenuSeparator from "$lib/menu/MenuSeparator.svelte";
    import AboutDialog from "$lib/dialog/AboutDialog.svelte";
    import HelpDialog from "$lib/dialog/HelpDialog.svelte";
    import ImageViewerPanel from "$lib/panel/ImageViewerPanel.svelte";
    import InputContextMenu from "$lib/reusable/InputContextMenu.svelte";
    import SettingsDialog from "$lib/settings/SettingsDialog.svelte";
    import ProgressOverlay from "$lib/reusable/ProgressOverlay.svelte";
    import {progress} from "$lib/progress.svelte";
    import {ollama} from "$lib/ollama/availability.svelte";
    import {settings} from "$lib/settings";
    import {shortcuts} from "$lib/shortcuts";
    import {t, initLocale} from "$lib/i18n";
    import ConfirmDialog, {type DialogIcon} from "$lib/dialog/ConfirmDialog.svelte";
    import {pickExportDir, exportCsv} from "$lib/csv/export";
    import type {CsvPreset} from "$lib/csv/csv";
    import {panelState} from "$lib/panel/filesPanelStore.svelte";
    import {error as logError} from "@tauri-apps/plugin-log";
    import {checkForUpdate, type UpdateInfo} from "$lib/update";
    import {openUrl} from "@tauri-apps/plugin-opener";

    // --- Docking ---
    // $derived so dock tab titles re-render on a language switch.
    const windowConfigs = $derived<WindowConfig[]>([
        {id: 'control', title: t('dock.window.control'), closable: true},
        {id: 'view', title: t('dock.window.view'), closable: false},
        {id: 'hierarchy', title: t('dock.window.hierarchy'), closable: true},
    ]);

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
    let viewerLoading = $state(false);
    let viewerToken = 0;

    /** Show a photo in the viewer via its high thumbnail (scans pre-generate it; fast). */
    async function showInViewer(path: string) {
        const token = ++viewerToken;
        // Photo caching off → show the original directly, no thumbnail generation.
        if (!settings.get<boolean>('cache.photo')) {
            imageSrc = convertFileSrc(path);
            viewerLoading = false;
            return;
        }
        viewerLoading = true;
        try {
            const thumbs = await invoke<{low: string; high: string}>('cache_thumbnail', {path, low: false, high: true});
            if (token !== viewerToken) return;
            imageSrc = convertFileSrc(thumbs.high);
        } catch {
            if (token !== viewerToken) return;
            imageSrc = convertFileSrc(path); // graceful fallback to the original
        } finally {
            if (token === viewerToken) viewerLoading = false;
        }
    }

    // --- Panel bindings ---
    let metaPanel: any = $state(null);
    let filesPanel: any = $state(null);
    let isDirty = $state(false);
    let isLoading = $state(false);
    let showAbout = $state(false);
    let showHelp = $state(false);
    let showSettings = $state(false);
    let batchPaths = $state<string[]>([]);

    // --- Appearance (theme + font scale) ---
    // Reactively apply the persisted appearance settings; re-runs when the user changes them in the dialog.
    $effect(() => {
        applyTheme(settings.subscribe<string>('appearance.theme')());
    });
    $effect(() => {
        applyFontScale(settings.subscribe<number>('appearance.font_size')());
    });
    // Re-apply on OS color-scheme change (only affects the 'system' theme).
    $effect(() => {
        const mq = window.matchMedia('(prefers-color-scheme: dark)');
        const onChange = () => applyTheme(settings.get<string>('appearance.theme'));
        mq.addEventListener('change', onChange);
        return () => mq.removeEventListener('change', onChange);
    });

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
        viewerToken++;
        viewerLoading = false;
        imageSrc = null;
        currentPath = null;
        showDialog = false;
        pendingPath = null;
        showGoneToast(name);
    }

    /** Opening a folder clears the viewer/editor so the previously open photo doesn't linger. */
    function handleFolderOpen(path: string) {
        metaPanel?.clear();
        viewerToken++;
        viewerLoading = false;
        imageSrc = null;
        currentPath = null;
        batchPaths = [];
        showDialog = false;
        pendingPath = null;
        saveAppState({lastFolder: path});
    }

    /** Update viewer when the file was renamed during save. */
    function handlePathChange(newPath: string) {
        currentPath = newPath;
        showInViewer(newPath);
        filesPanel?.setSelectedPath(newPath);
    }

    /** Actually open a file: load into metadata panel + show in viewer. */
    async function openFile(path: string) {
        await metaPanel?.loadFile(path);
        showInViewer(path);
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
            showInViewer(paths[paths.length - 1]);
        }
    }

    /** Called when Alt+click on a photo in batch mode — preview only. */
    function handleAltSelect(path: string) {
        showInViewer(path);
    }

    // --- CSV export ---
    let exportDialog = $state<{title: string; body: string; icon: DialogIcon} | null>(null);

    function isImageName(name: string): boolean {
        return /\.(jpg|jpeg|png|webp)$/i.test(name);
    }

    /** Resolve the export scope: selected photos, else every photo in the current folder (non-recursive). */
    function resolveExportScope(): string[] {
        const tree = panelState.fileTree;
        const folderPhotos = tree ? tree.children.filter(c => !c.is_dir && isImageName(c.name)).map(c => c.path) : [];
        const selected = panelState.selectedPaths;
        if (selected.size > 0) {
            const inFolder = folderPhotos.filter(p => selected.has(p));
            return inFolder.length > 0 ? inFolder : [...selected];
        }
        return folderPhotos;
    }

    /** File -> Export to CSV: validate, pick a destination folder, write one CSV per preset. */
    async function runCsvExport() {
        const presets = settings.get<CsvPreset[]>('csv.presets') ?? [];
        if (presets.length === 0) {
            exportDialog = {title: t('dialog.exportCsv.title'), body: t('dialog.exportCsv.noPresets'), icon: 'warning'};
            return;
        }
        const paths = resolveExportScope();
        if (paths.length === 0) {
            exportDialog = {title: t('dialog.exportCsv.title'), body: t('dialog.exportCsv.empty'), icon: 'warning'};
            return;
        }
        let dir: string | null;
        try {
            dir = await pickExportDir();
        } catch (e) {
            logError(`CSV export: folder picker failed: ${e}`);
            return;
        }
        if (!dir) return; // cancelled
        try {
            const summary = await exportCsv(dir, paths, presets);
            exportDialog = {
                title: t('dialog.exportCsv.title'),
                body: t('dialog.exportCsv.resultBody', {files: summary.filesWritten, exported: summary.photosExported, skipped: summary.skipped}),
                icon: 'info',
            };
        } catch (e) {
            logError(`CSV export failed: ${e}`);
            exportDialog = {title: t('dialog.exportCsv.title'), body: String(e), icon: 'error'};
        }
    }

    // --- Update check (notify-only) ---
    let updateDialog = $state<{title: string; body: string; icon: DialogIcon; url: string | null} | null>(null);

    /** Query GitHub for a newer version. `manual` also reports "up to date" / errors; startup checks stay silent unless an update exists. */
    async function runUpdateCheck(manual: boolean) {
        let info: UpdateInfo;
        try {
            info = await checkForUpdate();
        } catch (e) {
            logError(`update check failed: ${e}`);
            if (manual) updateDialog = {title: t('dialog.update.title'), body: t('dialog.update.error'), icon: 'error', url: null};
            return;
        }
        if (info.available) {
            updateDialog = {title: t('dialog.update.title'), body: t('dialog.update.available', {version: info.latestVersion}), icon: 'info', url: info.url};
        } else if (manual) {
            updateDialog = {title: t('dialog.update.title'), body: t('dialog.update.upToDate', {version: info.currentVersion}), icon: 'info', url: null};
        }
    }

    async function openReleases() {
        const url = updateDialog?.url;
        updateDialog = null;
        if (url) {
            try {
                await openUrl(url);
            } catch (e) {
                logError(`open releases page failed: ${e}`);
            }
        }
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

    function handleGlobalKeyDown(e: KeyboardEvent) {
        // A blocking progress overlay (attribution / batch save) freezes the whole app, shortcuts included.
        if (progress.blocking) {
            e.preventDefault();
            return;
        }
        if (shortcuts.handleKeyDown(e)) e.preventDefault();
    }

    onMount(async () => {
        window.addEventListener('keydown', handleGlobalKeyDown);
        const win = getCurrentWindow();
        const state = await loadAppState();

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

        // Load settings before restoring the folder so the cache config (cacheGenConfig) and the
        // viewer reflect the user's saved settings instead of the defaults.
        await settings.load();

        // One-time migration: carry the pre-settings theme choice (old ui-state.json) into the registry.
        if (!settings.wasPersisted('appearance.theme') && state.theme) {
            settings.set('appearance.theme', state.theme);
        }

        // Resolve the interface language (first-run OS detection) before the window is shown, so the
        // first painted frame is already localized.
        await initLocale();

        // Prefetch Ollama status + installed models so settings show suggestions with no wait.
        void ollama.init();

        // Quietly check for a newer version on startup (notify-only; gated by a setting).
        if (settings.get<boolean>('general.checkUpdatesOnStartup')) {
            void runUpdateCheck(false);
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

        // 4. Load shortcuts before showing window
        await shortcuts.load();

        // 5. Bind shortcut handlers (panel refs available after mount)
        shortcuts.setHandler('file.open_folder', () => filesPanel?.openFolderDialog());
        shortcuts.setHandler('file.settings',    () => { showSettings = true; });
        shortcuts.setHandler('editor.save',      () => metaPanel?.save());
        shortcuts.setHandler('files.navigate_up',          () => filesPanel?.navigate('up', false));
        shortcuts.setHandler('files.navigate_down',        () => filesPanel?.navigate('down', false));
        shortcuts.setHandler('files.navigate_up_extend',   () => filesPanel?.navigate('up', true));
        shortcuts.setHandler('files.navigate_down_extend', () => filesPanel?.navigate('down', true));

        // 6. Show window after full UI init
        await win.show();

        // 6. Save window size whenever it changes (debounced)
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
        window.removeEventListener('keydown', handleGlobalKeyDown);
        unlistenResize?.();
        if (winResizeTimer) clearTimeout(winResizeTimer);
    });
</script>

<div class="app">
    <MenuBar>
        <MenuTab label={t('menu.file.label')}>
            <MenuItem label={t('menu.file.openDirectory')} shortcut={shortcuts.getEffectiveBinding('file.open_folder') ?? undefined} onClick={() => filesPanel?.openFolderDialog()} />
            <MenuItem label={t('menu.file.exportCsv')} onClick={runCsvExport} />
            <MenuSeparator />
            <MenuItem label={t('menu.file.settings')} shortcut={shortcuts.getEffectiveBinding('file.settings') ?? undefined} onClick={() => {showSettings = true;}} />
        </MenuTab>
        <MenuTab label={t('menu.windows.label')}>
            <MenuItem
                label={t('menu.windows.showControl')}
                onClick={() => handleShowWindow('control')}
                disabled={!hiddenWindows.includes('control')}
            />
            <MenuItem
                label={t('menu.windows.showHierarchy')}
                onClick={() => handleShowWindow('hierarchy')}
                disabled={!hiddenWindows.includes('hierarchy')}
            />
        </MenuTab>
        <MenuTab label={t('menu.help.label')}>
            <MenuItem label={t('menu.help.checkUpdates')} onClick={() => runUpdateCheck(true)} />
            <MenuSeparator />
            <MenuItem label={t('menu.help.help')} onClick={() => { showHelp = true; }} />
            <MenuItem label={t('menu.help.about')} onClick={() => { showAbout = true; }} />
        </MenuTab>
    </MenuBar>

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
                <ImageViewerPanel {imageSrc} loading={viewerLoading} {goneMessage} onDismissGone={() => { goneMessage = null; }} />
            {:else if windowId === 'hierarchy'}
                <FilesPanel
                    bind:this={filesPanel}
                    onFileSelect={handleFileSelect}
                    onFileGone={handleFileGone}
                    onFolderOpen={handleFolderOpen}
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
<InputContextMenu />

{#if showHelp}
    <HelpDialog onClose={() => { showHelp = false; }} />
{/if}

{#if showAbout}
    <AboutDialog onClose={() => { showAbout = false; }} />
{/if}

<SettingsDialog open={showSettings} onClose={() => {showSettings = false;}} />

{#if exportDialog}
    <ConfirmDialog
        title={exportDialog.title}
        body={exportDialog.body}
        icon={exportDialog.icon}
        buttons={[{label: t('common.close'), onClick: () => (exportDialog = null)}]}
        onClose={() => (exportDialog = null)}
    />
{/if}

{#if updateDialog}
    <ConfirmDialog
        title={updateDialog.title}
        body={updateDialog.body}
        icon={updateDialog.icon}
        buttons={updateDialog.url
            ? [{label: t('dialog.update.open'), onClick: openReleases}, {label: t('common.close'), onClick: () => (updateDialog = null)}]
            : [{label: t('common.close'), onClick: () => (updateDialog = null)}]}
        onClose={() => (updateDialog = null)}
    />
{/if}

{#if showDialog}
    <UnsavedChangesDialog
        filename={currentBasename}
        onDiscard={handleDialogDiscard}
        onSave={handleDialogSave}
        onCancel={handleDialogCancel}
    />
{/if}

<!-- Top-most reusable progress overlay (Ollama pull/attribution, batch save freeze) -->
<ProgressOverlay />

<style lang="scss">
    @use 'styles/mixins' as *;

    .app {
        @include flex(column, flex-start, stretch);
        height: 100vh;
        overflow: hidden;
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
