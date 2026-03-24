<script lang="ts">
    import {themes, applyTheme} from './themes';
    import {saveAppState} from './store';

    let {current = 'dark'}: {current?: string} = $props();

    let open = $state(false);
    let triggerEl: HTMLButtonElement | undefined;
    let listEl = $state<HTMLUListElement | undefined>(undefined);

    const currentName = $derived(themes.find(t => t.id === current)?.name ?? current);

    function toggle() {
        open = !open;
    }

    function select(id: string) {
        current = id;
        open = false;
        applyTheme(id);
        saveAppState({theme: id});
    }

    function handleWindowClick(e: MouseEvent) {
        if (!triggerEl || !listEl) return;
        if (!triggerEl.contains(e.target as Node) && !listEl.contains(e.target as Node)) {
            open = false;
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') open = false;
    }
</script>

<svelte:window onclick={handleWindowClick} onkeydown={handleKeydown} />

<div class="theme-switcher">
    <button class="theme-trigger" bind:this={triggerEl} onclick={toggle}>
        {currentName}
        <svg class="chevron" class:open viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M4 6l4 4 4-4"/>
        </svg>
    </button>

    {#if open}
        <ul class="theme-list" bind:this={listEl}>
            {#each themes as t}
                <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                <li
                    class="theme-item"
                    class:theme-item--selected={t.id === current}
                    onmousedown={(e) => { e.preventDefault(); select(t.id); }}
                >{t.name}</li>
            {/each}
        </ul>
    {/if}
</div>

<style lang="scss">
    @use '../styles/mixins' as *;

    .theme-switcher {
        position: relative;
    }

    .theme-trigger {
        @include btn-reset;
        @include flex(row, flex-start, center);
        @include transition(background, color, border-color);
        gap: 5px;
        padding: 4px 10px;
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        color: $text-secondary;
        font-size: $fs-small;
        font-family: $font-base;
        box-shadow: 0 6px 20px var(--shadow);

        &:hover { color: $text; }
    }

    .chevron {
        width: 12px;
        height: 12px;
        flex-shrink: 0;
        color: $text-muted;
        transition: transform 0.15s;

        &.open { transform: rotate(180deg); }
    }

    .theme-list {
        position: absolute;
        top: calc(100% + 3px);
        right: 0;
        min-width: 100%;
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        list-style: none;
        padding: 3px 0;
        box-shadow: 0 6px 20px var(--shadow);
        z-index: 1100;
    }

    .theme-item {
        padding: 5px 10px;
        font-size: $fs-small;
        color: $text-secondary;
        cursor: pointer;
        white-space: nowrap;
        @include transition(background, color);

        &:hover, &--selected {
            background: $bg-input-focus;
            color: $text;
        }
    }
</style>
