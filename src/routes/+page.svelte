<script lang="ts">
    import MetadataPanel from "$lib/MetadataPanel.svelte";
    import FilesPanel from "$lib/FilesPanel.svelte";

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

    function handleFileSelect(path: string) {
        // TODO: load image into viewer
    }
</script>

<div class="app" class:resizing>
    <!-- ── Left: metadata panel ── -->
    <div class="panel-wrapper" style="width: {panelWidth}px;">
        <MetadataPanel />
    </div>

    <!-- ── Left resize handle ── -->
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
    </main>

    <!-- ── Right resize handle ── -->
    <div
        class="resize-handle"
        onmousedown={startRightResize}
        role="separator"
        aria-label="Resize files panel"
        aria-orientation="vertical"
    ></div>

    <!-- ── Right: files panel ── -->
    <div class="panel-wrapper" style="width: {rightPanelWidth}px;">
        <FilesPanel onFileSelect={handleFileSelect} />
    </div>
</div>

<style lang="scss">
    @use '../styles/mixins' as *;

    // ── Root layout ──
    .app {
        @include flex(row, flex-start, stretch);
        height: 100vh;
        overflow: hidden;

        &.resizing {
            user-select: none;
            cursor: col-resize;
        }
    }

    // Wrapper enforces width; child <aside> must fill it completely
    .panel-wrapper {
        @include flex(column, flex-start, stretch, null, 0);
        min-width: 0;
        overflow: hidden;

        :global(aside.panel) {
            flex: 1;        // fill wrapper height
            min-height: 0;  // allow inner scroll to work in a flex child
            width: 100%;
        }
    }

    // ── Resize handle ──
    .resize-handle {
        width: $handle-width;
        flex-shrink: 0;
        background: $border;
        cursor: col-resize;
        @include transition(background);
        position: relative;

        &::after {
            // Wider invisible hit area for easier grabbing
            content: '';
            position: absolute;
            inset: 0 -4px;
        }

        &:hover { background: $accent; }
    }

    // ── Image viewer ──
    .viewer {
        @include flex(row, center, center, 1);
        overflow: hidden;
        background: $bg-app;
        min-width: 0;
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
</style>
