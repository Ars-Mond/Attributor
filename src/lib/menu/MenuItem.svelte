<script lang="ts">
    import {getContext} from 'svelte';

    let {
        label,
        onClick,
        disabled = false,
        shortcut,
    }: {
        label: string;
        onClick: () => void;
        disabled?: boolean;
        shortcut?: string;
    } = $props();

    const menuBar = getContext<{setOpenTabId: (id: string | null) => void} | undefined>('menu-bar');
    const parentDropdown = getContext<{setOpenNestedId: (id: string | null) => void} | undefined>('menu-tab-dropdown');

    function handleClick() {
        if (disabled) return;
        menuBar?.setOpenTabId(null);
        onClick();
    }

    function handleMouseEnter() {
        // Close any open submenu when hovering a plain item
        parentDropdown?.setOpenNestedId(null);
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter' || e.key === ' ') handleClick();
    }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<li
    class="menu-item"
    class:menu-item--disabled={disabled}
    role="menuitem"
    tabindex={disabled ? -1 : 0}
    onclick={handleClick}
    onkeydown={handleKeydown}
    onmouseenter={handleMouseEnter}
>
    {label}
    {#if shortcut}
        <span class="shortcut">{shortcut}</span>
    {/if}
</li>

<style lang="scss">
    @use 'styles/mixins' as *;

    .menu-item {
        display: flex;
        align-items: center;
        gap: 16px;
        padding: 5px 12px;
        font-size: $fs-small;
        color: $text-secondary;
        cursor: pointer;
        white-space: nowrap;
        list-style: none;
        @include transition(background, color);

        &:hover:not(.menu-item--disabled) {
            background: $bg-input-focus;
            color: $text;
        }

        &--disabled {
            opacity: 0.4;
            cursor: default;
        }
    }

    .shortcut {
        margin-left: auto;
        font-size: $fs-footnote1;
        color: $text-muted;
    }
</style>
