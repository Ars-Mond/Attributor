<script lang="ts">
    import {convertFileSrc} from "@tauri-apps/api/core";
    import MetadataPanel from "$lib/MetadataPanel.svelte";
    import FilesPanel from "$lib/FilesPanel.svelte";
    import UnsavedChangesDialog from "$lib/UnsavedChangesDialog.svelte";

    // --- Left panel resize ---
    const LEFT_MIN = 260;
    const LEFT_MAX = 700;
    let panelWidth = $state(380);
    let resizing = $state(false);

    function startResize(e: MouseEvent) {
        e.preventDefault();
        resizing = true;

        function onMove(ev: MouseEvent) {
            panelWidth = Math.min(LEFT_MAX, Math.max(LEFT_MIN, ev.clientX));
        }

        function onUp() {
            resizing = false;
            window.removeEventListener("mousemove", onMove);
            window.removeEventListener("mouseup", onUp);
        }

        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
    }

    // --- Right panel resize ---
    const RIGHT_MIN = 160;
    const RIGHT_MAX = 500;
    let rightPanelWidth = $state(240);

    function startRightResize(e: MouseEvent) {
        e.preventDefault();
        resizing = true;

        function onMove(ev: MouseEvent) {
            rightPanelWidth = Math.min(RIGHT_MAX, Math.max(RIGHT_MIN, window.innerWidth - ev.clientX));
        }

        function onUp() {
            resizing = false;
            window.removeEventListener("mousemove", onMove);
            window.removeEventListener("mouseup", onUp);
        }

        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
    }

    // --- Image viewer ---
    let imageSrc = $state<string | null>(null);

    // --- Panel bindings ---
    let metaPanel: any = $state(null);
    let filesPanel: any = $state(null);
    let isDirty = $state(false);
    let isLoading = $state(false);

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
    }

    /** Called from FilesPanel when user clicks a file. */
    async function handleFileSelect(path: string) {
        if (isDirty) {
            pendingPath = path;
            showDialog = true;
        } else {
            await openFile(path);
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
    }
</script>

<div class="app" class:resizing>
    <!-- ── Left: metadata panel ── -->
    <div class="panel-wrapper" style="width: {panelWidth}px;">
        <MetadataPanel bind:this={metaPanel} bind:isDirty onPathChange={handlePathChange} />
    </div>

    <!-- ── Left resize handle ── -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
        class="resize-handle"
        onmousedown={startResize}
        role="separator"
        aria-label="Resize metadata panel"
        aria-orientation="vertical"
    ></div>

    <!-- ── Center: image viewer ── -->
    <main class="viewer">
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

        <!-- ── File-gone toast ── -->
        {#if goneMessage}
            <div class="gone-toast" role="alert">
                <svg class="gone-icon" viewBox="0 0 20 20" fill="currentColor">
                    <path fill-rule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd"/>
                </svg>
                <span>
                    <strong>{goneMessage}</strong> was moved or deleted externally.
                </span>
                <button class="gone-close" onclick={() => { goneMessage = null; }} aria-label="Dismiss">×</button>
            </div>
        {/if}
    </main>

    <!-- ── Right resize handle ── -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
        class="resize-handle"
        onmousedown={startRightResize}
        role="separator"
        aria-label="Resize files panel"
        aria-orientation="vertical"
    ></div>

    <!-- ── Right: files panel ── -->
    <div class="panel-wrapper" style="width: {rightPanelWidth}px;">
        <FilesPanel bind:this={filesPanel} onFileSelect={handleFileSelect} onFileGone={handleFileGone} onBusy={(b) => (isLoading = b)} />
    </div>
</div>

<!-- ── Loading overlay (dialog open) ── -->
{#if isLoading}
    <div class="loading-overlay" aria-hidden="true">
        <div class="spinner"></div>
    </div>
{/if}

<!-- ── Unsaved changes dialog ── -->
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
        @include flex(row, flex-start, stretch);
        height: 100vh;
        overflow: hidden;

        &.resizing {
            user-select: none;
            cursor: col-resize;
        }
    }

    .panel-wrapper {
        @include flex(column, flex-start, stretch, null, 0);
        min-width: 0;
        overflow: hidden;

        :global(aside.panel) {
            flex: 1;
            min-height: 0;
            width: 100%;
        }
    }

    .resize-handle {
        width: $handle-width;
        flex-shrink: 0;
        background: $border;
        cursor: col-resize;
        @include transition(background);
        position: relative;

        &::after {
            content: '';
            position: absolute;
            inset: 0 -4px;
        }

        &:hover { background: $accent; }
    }

    .viewer {
        @include flex(row, center, center, 1);
        overflow: hidden;
        background: $bg-app;
        min-width: 0;
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

        svg { opacity: 0.4; }
        p   { font-size: $fs-regular; }
    }

    // ── File-gone toast ──
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
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
        animation: toast-in 0.2s ease;

        strong { color: #fde68a; }
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

        &:hover { opacity: 1; }
    }

    @keyframes toast-in {
        from { opacity: 0; transform: translateX(-50%) translateY(8px); }
        to   { opacity: 1; transform: translateX(-50%) translateY(0); }
    }

    // ── Loading overlay ──
    .loading-overlay {
        position: fixed;
        inset: 0;
        z-index: 200;
        backdrop-filter: blur(4px);
        background: rgba(0, 0, 0, 0.25);
        pointer-events: all;
        @include flex(row, center, center);
    }

    .spinner {
        width: 30px;
        height: 30px;
        border-radius: 50%;
        border: 2.5px solid rgba(255, 255, 255, 0.15);
        border-top-color: rgba(255, 255, 255, 0.75);
        animation: spin 0.65s linear infinite;
    }

    @keyframes spin {
        to { transform: rotate(360deg); }
    }
</style>
