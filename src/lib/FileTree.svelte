<script lang="ts">
    import { untrack } from "svelte";
    import FileTree from "./FileTree.svelte";
    import type { FileNode } from "./types";

    let {
        node,
        depth = 0,
        selectedPaths,
        activePath,
        onSelect,
    }: {
        node: FileNode;
        depth?: number;
        selectedPaths: Set<string>;
        activePath: string;
        onSelect: (path: string, e: MouseEvent) => void;
    } = $props();

    // untrack: depth is a static prop per node instance; we only need its
    // initial value to decide whether to start expanded.
    let expanded = $state(untrack(() => depth === 0));

    const isImage = $derived(
        !node.is_dir &&
        /\.(jpg|jpeg|png|webp)$/i.test(node.name)
    );
</script>

<div class="tree-node">
    {#if node.is_dir}
        <!-- Folder row -->
        <button
            class="tree-item"
            style="padding-left: {depth * 14 + 8}px"
            onclick={() => (expanded = !expanded)}
        >
            <svg class="chevron" class:open={expanded} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M6 4l4 4-4 4"/>
            </svg>
            <svg class="icon" viewBox="0 0 16 16" fill="currentColor">
                <path d="M1.5 3A1.5 1.5 0 0 0 0 4.5v8A1.5 1.5 0 0 0 1.5 14h13a1.5 1.5 0 0 0 1.5-1.5v-7A1.5 1.5 0 0 0 14.5 4H7.621a1.5 1.5 0 0 1-1.06-.44L5.5 2.5A1.5 1.5 0 0 0 4.379 2H1.5A1.5 1.5 0 0 0 0 3.5v.5h1.5V3z"/>
            </svg>
            <span class="name">{node.name}</span>
        </button>

        {#if expanded}
            {#each node.children as child (child.path)}
                <FileTree
                    node={child}
                    depth={depth + 1}
                    {selectedPaths}
                    {activePath}
                    {onSelect}
                />
            {/each}
        {/if}

    {:else}
        <!-- File row -->
        <button
            class="tree-item file"
            class:selected={selectedPaths.has(node.path)}
            class:active={activePath === node.path}
            class:image={isImage}
            style="padding-left: {depth * 14 + 8}px"
            data-path={node.path}
            onclick={(e) => onSelect(node.path, e)}
        >
            {#if isImage}
                <!-- Image file icon -->
                <svg class="icon image-icon" viewBox="0 0 16 16" fill="currentColor">
                    <path d="M4.502 9a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3z"/>
                    <path d="M14.002 13a2 2 0 0 1-2 2h-10a2 2 0 0 1-2-2V5A2 2 0 0 1 2 3a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v8a2 2 0 0 1-1.998 2zM14 2H4a1 1 0 0 0-1 1h9.002a2 2 0 0 1 2 2v7A1 1 0 0 0 15 11V3a1 1 0 0 0-1-1zM2.002 4a1 1 0 0 0-1 1v8l2.646-2.354a.5.5 0 0 1 .63-.062l2.66 1.773 3.71-3.71a.5.5 0 0 1 .577-.094l1.777 1.947V5a1 1 0 0 0-1-1h-10z"/>
                </svg>
            {:else}
                <!-- Generic file icon -->
                <svg class="icon" viewBox="0 0 16 16" fill="currentColor">
                    <path d="M4 0a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V4.5L9.5 0H4zm5.5 1v3.5H13L9.5 1z"/>
                </svg>
            {/if}
            <span class="name">{node.name}</span>
        </button>
    {/if}
</div>

<style lang="scss">
    @use '../styles/mixins' as *;

    .tree-node { width: 100%; }

    .tree-item {
        @include btn-reset;
        @include flex(row, flex-start, center);
        gap: 5px;
        width: 100%;
        padding-top: 3px;
        padding-bottom: 3px;
        padding-right: 8px;
        border-radius: $radius-sm;
        color: $text-secondary;
        font-size: $fs-small;
        font-family: $font-base;
        text-align: left;
        @include transition(background, color);
        min-width: 0;

        &:hover {
            background: var(--hover-bg);
            color: $text;
        }

        &.selected {
            background: $chip-bg;
            color: $chip-text;
        }

        &.active {
            box-shadow: inset 2px 0 0 $accent;
        }
    }

    .chevron {
        width: 12px;
        height: 12px;
        flex-shrink: 0;
        color: $text-muted;
        transition: transform 0.15s;

        &.open { transform: rotate(90deg); }
    }

    .icon {
        width: 13px;
        height: 13px;
        flex-shrink: 0;
        color: $text-muted;
        opacity: 0.7;
    }

    .image-icon { color: $accent; opacity: 0.8; }

    .name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        min-width: 0;
    }
</style>
