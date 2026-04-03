<script lang="ts">
    import type {DropZone} from './dockTypes';
    import type {Snippet} from 'svelte';

    let {
        windowId,
        title,
        closable = false,
        isDragTarget = false,
        dropZone = null,
        onDragStart,
        onClose,
        children,
    }: {
        windowId: string;
        title: string;
        closable?: boolean;
        isDragTarget?: boolean;
        dropZone?: DropZone | null;
        onDragStart?: (windowId: string, e: PointerEvent) => void;
        onClose?: (windowId: string) => void;
        children: Snippet;
    } = $props();

    function handlePointerDown(e: PointerEvent) {
        // Ignore if clicking the close button
        if ((e.target as HTMLElement).closest('.dock-close')) return;
        onDragStart?.(windowId, e);
    }

    function handleClose(e: MouseEvent) {
        e.stopPropagation();
        onClose?.(windowId);
    }

    const overlayStyle = $derived.by(() => {
        if (!isDragTarget || !dropZone) return null;
        switch (dropZone) {
            case 'top': return 'top:0;left:0;right:0;height:50%';
            case 'bottom': return 'bottom:0;left:0;right:0;height:50%';
            case 'left': return 'top:0;left:0;bottom:0;width:50%';
            case 'right': return 'top:0;right:0;bottom:0;width:50%';
        }
    });
</script>

<div class="dock-window" data-window-id={windowId}>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="dock-titlebar" onpointerdown={handlePointerDown} role="banner">
        <span class="dock-title">{title}</span>
        {#if closable}
            <button class="dock-close" onclick={handleClose} aria-label="Close {title}">×</button>
        {/if}
    </div>
    <div class="dock-content">
        {@render children()}
    </div>
    {#if overlayStyle}
        <div class="dock-drop-overlay" style={overlayStyle}></div>
    {/if}
</div>

<style lang="scss">
    @use 'styles/mixins' as *;

    .dock-window {
        @include flex(column, flex-start, stretch);
        flex: 1;
        min-width: 0;
        min-height: 0;
        overflow: hidden;
        position: relative;
    }

    .dock-titlebar {
        @include flex(row, space-between, center);
        height: 28px;
        min-height: 28px;
        padding: 0 8px;
        background: $bg-surface;
        border-bottom: 1px solid $border;
        cursor: grab;
        user-select: none;

        &:active {
            cursor: grabbing;
        }
    }

    .dock-title {
        font-size: $fs-footnote1;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.04em;
        color: $text-secondary;
    }

    .dock-close {
        @include btn-reset;
        @include flex(row, center, center);
        width: 20px;
        height: 20px;
        font-size: 16px;
        line-height: 1;
        color: $text-muted;
        border-radius: $radius-sm;
        @include transition(color, background);

        &:hover {
            color: $text;
            background: $bg-input;
        }
    }

    .dock-content {
        flex: 1;
        min-height: 0;
        min-width: 0;
        overflow: hidden;
        @include flex(column, flex-start, stretch);

        :global(aside.panel) {
            flex: 1;
            min-height: 0;
            width: 100%;
        }
    }

    .dock-drop-overlay {
        position: absolute;
        background: var(--accent);
        opacity: 0.12;
        border: 2px dashed var(--accent);
        pointer-events: none;
        transition: opacity 0.1s;
        z-index: 50;
    }
</style>
