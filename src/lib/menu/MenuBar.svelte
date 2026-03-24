<script lang="ts">
    import {setContext} from 'svelte';
    import type {Snippet} from 'svelte';

    let {children}: {children: Snippet} = $props();

    let openTabId = $state<string | null>(null);
    let barEl = $state<HTMLElement | undefined>(undefined);

    setContext('menu-bar', {
        get openTabId() { return openTabId; },
        setOpenTabId(id: string | null) { openTabId = id; },
    });

    function handleWindowClick(e: MouseEvent) {
        if (!barEl) return;
        // Close all menus when clicking outside the bar or any open dropdown
        // Dropdowns are rendered inside the nav, so contains() covers them too
        if (!barEl.contains(e.target as Node)) {
            openTabId = null;
        }
    }

    function handleWindowKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') openTabId = null;
    }
</script>

<svelte:window onclick={handleWindowClick} onkeydown={handleWindowKeydown} />

<nav class="menu-bar" bind:this={barEl}>
    {@render children()}
</nav>

<style lang="scss">
    @use '../../styles/mixins' as *;

    .menu-bar {
        @include flex(row, flex-start, stretch);
        height: 28px;
        min-height: 28px;
        padding: 0 4px;
        background: $bg-panel;
        border-bottom: 1px solid $border;
        flex-shrink: 0;
    }
</style>
