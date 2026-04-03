<script lang="ts">
    import type {LayoutNode, SplitNode, SplitChild, DropZone, WindowConfig} from './dockTypes';
    import type {Snippet} from 'svelte';
    import DockWindow from './DockWindow.svelte';
    import {removePanel, insertPanel} from './dockStore';

    let {
        layout = $bindable(),
        windowConfigs = [],
        onClose,
        onLayoutChange,
        renderWindow,
    }: {
        layout: LayoutNode;
        windowConfigs?: WindowConfig[];
        onClose?: (windowId: string) => void;
        onLayoutChange?: (layout: LayoutNode) => void;
        renderWindow: Snippet<[string]>;
    } = $props();

    // --- Drag state ---
    let dragWindowId = $state<string | null>(null);
    let dragActive = $state(false);
    let dragStartX = $state(0);
    let dragStartY = $state(0);
    let ghostX = $state(0);
    let ghostY = $state(0);
    let targetWindowId = $state<string | null>(null);
    let targetZone = $state<DropZone | null>(null);

    // --- Resize state ---
    let resizing = $state(false);

    const DRAG_THRESHOLD = 5;

    function handleDragStart(windowId: string, e: PointerEvent) {
        dragWindowId = windowId;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        ghostX = e.clientX;
        ghostY = e.clientY;
        dragActive = false;

        window.addEventListener('pointermove', handleDragMove);
        window.addEventListener('pointerup', handleDragEnd);
    }

    function handleDragMove(e: PointerEvent) {
        if (!dragWindowId) return;

        const dx = e.clientX - dragStartX;
        const dy = e.clientY - dragStartY;

        if (!dragActive && Math.sqrt(dx * dx + dy * dy) < DRAG_THRESHOLD) return;
        dragActive = true;
        ghostX = e.clientX;
        ghostY = e.clientY;

        // Determine which window is under cursor
        const el = document.elementFromPoint(e.clientX, e.clientY);
        const windowEl = el?.closest('[data-window-id]') as HTMLElement | null;
        const hoveredId = windowEl?.dataset.windowId ?? null;

        if (hoveredId && hoveredId !== dragWindowId) {
            targetWindowId = hoveredId;
            const rect = windowEl!.getBoundingClientRect();
            targetZone = computeDropZone(e.clientX, e.clientY, rect);
        } else {
            targetWindowId = null;
            targetZone = null;
        }
    }

    function handleDragEnd(_e: PointerEvent) {
        window.removeEventListener('pointermove', handleDragMove);
        window.removeEventListener('pointerup', handleDragEnd);

        if (dragActive && dragWindowId && targetWindowId && targetZone) {
            const removed = removePanel(layout, dragWindowId);
            if (removed) {
                layout = insertPanel(removed, targetWindowId, dragWindowId, targetZone);
                onLayoutChange?.(layout);
            }
        }

        dragWindowId = null;
        dragActive = false;
        targetWindowId = null;
        targetZone = null;
    }

    function computeDropZone(x: number, y: number, rect: DOMRect): DropZone {
        const dTop = y - rect.top;
        const dBottom = rect.bottom - y;
        const dLeft = x - rect.left;
        const dRight = rect.right - x;
        const min = Math.min(dTop, dBottom, dLeft, dRight);
        if (min === dTop) return 'top';
        if (min === dBottom) return 'bottom';
        if (min === dLeft) return 'left';
        return 'right';
    }

    function getWindowConfig(id: string): WindowConfig | undefined {
        return windowConfigs.find(c => c.id === id);
    }

    // --- Split resize ---
    // Handle[i] resizes children[i] and children[i+1] — only those two change.
    function startSplitResize(
        node: SplitNode,
        index: number,
        containerEl: HTMLElement,
        e: MouseEvent,
    ) {
        e.preventDefault();
        resizing = true;
        const rect = containerEl.getBoundingClientRect();
        const isHorizontal = node.direction === 'horizontal';
        const totalSize = isHorizontal ? rect.width : rect.height;
        const handleCount = node.children.length - 1;
        const handleTotal = handleCount * 5; // handle-width = 5px
        const availableSize = totalSize - handleTotal;

        const childA = node.children[index];
        const childB = node.children[index + 1];
        const sumSize = childA.size + childB.size;

        // Min sizes in pixels
        const minA = getMinSize(childA.node, node.direction);
        const minB = getMinSize(childB.node, node.direction);

        // Convert min pixel sizes to fractions of the combined pair
        const pairPixels = sumSize * availableSize;
        const minFracA = pairPixels > 0 ? (minA / pairPixels) * sumSize : 0.1;
        const minFracB = pairPixels > 0 ? (minB / pairPixels) * sumSize : 0.1;

        // Track the starting pixel position of childA within the container
        let offsetBefore = 0;
        for (let i = 0; i < index; i++) {
            offsetBefore += node.children[i].size * availableSize + 5;
        }
        const paneStart = (isHorizontal ? rect.left : rect.top) + offsetBefore;

        function onMove(ev: MouseEvent) {
            const pos = isHorizontal ? ev.clientX : ev.clientY;
            const pxFromStart = pos - paneStart;
            const newSizeA = Math.max(minFracA, Math.min(sumSize - minFracB, (pxFromStart / availableSize) * 1));
            childA.size = newSizeA;
            childB.size = sumSize - newSizeA;
            layout = layout; // trigger reactivity
        }

        function onUp() {
            resizing = false;
            window.removeEventListener('mousemove', onMove);
            window.removeEventListener('mouseup', onUp);
            onLayoutChange?.(layout);
        }

        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
    }

    function getMinSize(node: LayoutNode, parentDirection: string): number {
        if (node.type === 'panel') {
            const isWidth = parentDirection === 'horizontal';
            switch (node.windowId) {
                case 'control': return isWidth ? 260 : 200;
                case 'view': return isWidth ? 200 : 200;
                case 'hierarchy': return isWidth ? 160 : 150;
                default: return isWidth ? 100 : 100;
            }
        }
        if (node.direction === parentDirection) {
            return node.children.reduce((sum, c) => sum + getMinSize(c.node, parentDirection), 0)
                + (node.children.length - 1) * 5;
        }
        return Math.max(...node.children.map(c => getMinSize(c.node, parentDirection)));
    }
</script>

<div class="dock-layout" class:dock-resizing={resizing} class:dock-dragging={dragActive}>
    {#snippet renderNode(node: LayoutNode)}
        {#if node.type === 'panel'}
            {@const cfg = getWindowConfig(node.windowId)}
            <DockWindow
                windowId={node.windowId}
                title={cfg?.title ?? node.windowId}
                closable={cfg?.closable ?? false}
                isDragTarget={dragActive && targetWindowId === node.windowId}
                dropZone={targetWindowId === node.windowId ? targetZone : null}
                onDragStart={handleDragStart}
                onClose={onClose}
            >
                {@render renderWindow(node.windowId)}
            </DockWindow>
        {:else}
            {@const splitNode = node}
            <div class="dock-split dock-split--{splitNode.direction}">
                {#each splitNode.children as child, i}
                    <div
                        class="dock-split-pane"
                        style={splitNode.direction === 'horizontal'
                            ? `flex:${child.size} 0 0%`
                            : `flex:${child.size} 0 0%`}
                    >
                        {@render renderNode(child.node)}
                    </div>
                    {#if i < splitNode.children.length - 1}
                        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                        <div
                            class="dock-handle dock-handle--{splitNode.direction}"
                            onmousedown={(e) => {
                                const container = e.currentTarget.parentElement;
                                if (container) startSplitResize(splitNode, i, container, e);
                            }}
                            role="separator"
                            aria-orientation={splitNode.direction === 'horizontal' ? 'vertical' : 'horizontal'}
                        ></div>
                    {/if}
                {/each}
            </div>
        {/if}
    {/snippet}

    {@render renderNode(layout)}

    {#if dragActive && dragWindowId}
        <div
            class="dock-ghost"
            style="left:{ghostX}px;top:{ghostY}px"
        >
            {getWindowConfig(dragWindowId)?.title ?? dragWindowId}
        </div>
    {/if}
</div>

<style lang="scss">
    @use 'styles/mixins' as *;

    .dock-layout {
        flex: 1;
        min-width: 0;
        min-height: 0;
        overflow: hidden;
        position: relative;
        @include flex(column, flex-start, stretch);

        &.dock-resizing {
            user-select: none;
        }

        &.dock-dragging {
            user-select: none;
            cursor: grabbing;
        }
    }

    .dock-split {
        @include flex(row, flex-start, stretch);
        flex: 1;
        min-width: 0;
        min-height: 0;
        overflow: hidden;

        &--vertical {
            flex-direction: column;
        }
    }

    .dock-split-pane {
        @include flex(column, flex-start, stretch);
        min-width: 0;
        min-height: 0;
        overflow: hidden;
    }

    .dock-handle {
        flex-shrink: 0;
        background: $border;
        @include transition(background);
        position: relative;

        &--horizontal {
            width: $handle-width;
            cursor: col-resize;

            &::after {
                content: '';
                position: absolute;
                inset: 0 -4px;
            }
        }

        &--vertical {
            height: $handle-width;
            cursor: row-resize;

            &::after {
                content: '';
                position: absolute;
                inset: -4px 0;
            }
        }

        &:hover {
            background: $accent;
        }
    }

    .dock-ghost {
        position: fixed;
        pointer-events: none;
        z-index: 9999;
        transform: translate(-50%, -50%);
        padding: 4px 12px;
        background: $bg-surface;
        border: 1px solid $border;
        border-radius: $radius-sm;
        box-shadow: 0 4px 16px var(--shadow);
        opacity: 0.85;
        font-size: $fs-footnote1;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.04em;
        color: $text-secondary;
        white-space: nowrap;
    }
</style>
