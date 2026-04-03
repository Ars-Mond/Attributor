<script lang="ts">
    import {getContext, setContext} from 'svelte';
    import type {Snippet} from 'svelte';

    let {
        label,
        children,
    }: {
        label: string;
        children: Snippet;
    } = $props();

    const id = crypto.randomUUID();

    // Context from MenuBar — always present when used inside a MenuBar
    const menuBar = getContext<{
        openTabId: string | null;
        setOpenTabId: (id: string | null) => void;
    } | undefined>('menu-bar');

    // Context from parent MenuTab's dropdown — present only when nested
    const parentDropdown = getContext<{
        openNestedId: string | null;
        setOpenNestedId: (id: string | null) => void;
    } | undefined>('menu-tab-dropdown');

    const isNested = !!parentDropdown;

    // Dropdown context we provide to our own children
    let openNestedId = $state<string | null>(null);
    setContext('menu-tab-dropdown', {
        get openNestedId() { return openNestedId; },
        setOpenNestedId(newId: string | null) { openNestedId = newId; },
    });

    const isOpen = $derived(
        isNested
            ? parentDropdown!.openNestedId === id
            : menuBar?.openTabId === id
    );

    // Reset nested submenu state when our dropdown closes
    $effect(() => {
        if (!isOpen) openNestedId = null;
    });

    let triggerEl = $state<HTMLElement | undefined>(undefined);
    let pos = $state({top: 0, left: 0});

    function computePos() {
        if (!triggerEl) return;
        const rect = triggerEl.getBoundingClientRect();
        if (isNested) {
            // Prefer right side, flip to left if not enough space
            const spaceRight = window.innerWidth - rect.right;
            if (spaceRight >= 160) {
                pos = {top: rect.top, left: rect.right + 2};
            } else {
                pos = {top: rect.top, left: rect.left - 162};
            }
        } else {
            pos = {top: rect.bottom, left: rect.left};
        }
    }

    function open() {
        computePos();
        if (isNested) {
            parentDropdown!.setOpenNestedId(id);
        } else {
            menuBar?.setOpenTabId(id);
        }
    }

    function close() {
        if (isNested) {
            parentDropdown!.setOpenNestedId(null);
        } else {
            menuBar?.setOpenTabId(null);
        }
    }

    function handleTriggerClick() {
        if (isOpen) close(); else open();
    }

    function handleMouseEnter() {
        if (isNested) {
            // Always open submenu on hover
            open();
        } else if (menuBar?.openTabId !== null) {
            // Switch to this tab when another top-level tab is open
            open();
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter' || e.key === ' ') handleTriggerClick();
        if (e.key === 'Escape') close();
    }
</script>

{#if isNested}
    <!-- Nested: renders as a list item with submenu indicator -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <li
        class="menu-item menu-item--has-sub"
        role="menuitem"
        tabindex="0"
        bind:this={triggerEl}
        onclick={handleTriggerClick}
        onkeydown={handleKeydown}
        onmouseenter={handleMouseEnter}
    >
        <span>{label}</span>
        <span class="menu-arrow">›</span>
        {#if isOpen}
            <ul
                class="menu-dropdown"
                style="top:{pos.top}px;left:{pos.left}px"
            >
                {@render children()}
            </ul>
        {/if}
    </li>
{:else}
    <!-- Top-level: renders as a trigger button in the menu bar -->
    <button
        class="menu-tab-trigger"
        class:menu-tab-trigger--open={isOpen}
        bind:this={triggerEl}
        onclick={handleTriggerClick}
        onmouseenter={handleMouseEnter}
    >
        {label}
    </button>
    {#if isOpen}
        <ul
            class="menu-dropdown"
            style="top:{pos.top}px;left:{pos.left}px"
        >
            {@render children()}
        </ul>
    {/if}
{/if}

<style lang="scss">
    @use 'styles/mixins' as *;

    .menu-tab-trigger {
        @include btn-reset;
        @include flex(row, flex-start, center);
        @include transition(background, color);
        height: 100%;
        padding: 0 8px;
        font-size: $fs-small;
        font-family: $font-base;
        color: $text-secondary;
        border-radius: $radius-sm;

        &:hover,
        &--open {
            background: $bg-input-focus;
            color: $text;
        }
    }

    .menu-item--has-sub {
        @include flex(row, space-between, center);
        gap: 16px;
        padding: 5px 8px 5px 12px;
        list-style: none;
        @include transition(background, color);

        &:hover {
            background: $bg-input-focus;
            color: $text;
        }
    }

    .menu-arrow {
        font-size: $fs-regular;
        color: $text-muted;
        line-height: 1;
    }

    .menu-dropdown {
        position: fixed;
        z-index: 1200;
        list-style: none;
        padding: 3px 0;
        margin: 0;
        min-width: 160px;
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        box-shadow: 0 6px 20px var(--shadow);
    }
</style>
