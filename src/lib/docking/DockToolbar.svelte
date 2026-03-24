<script lang="ts">
    import ThemeSwitcher from '$lib/ThemeSwitcher.svelte';
    import type {WindowConfig} from './dockTypes';

    let {
        hiddenWindows = [],
        windowConfigs = [],
        currentTheme = 'dark',
        onShowWindow,
    }: {
        hiddenWindows?: string[];
        windowConfigs?: WindowConfig[];
        currentTheme?: string;
        onShowWindow?: (id: string) => void;
    } = $props();

    const hiddenConfigs = $derived(
        windowConfigs.filter(c => hiddenWindows.includes(c.id))
    );
</script>

{#if hiddenConfigs.length > 0}
    <div class="dock-toolbar">
        <div class="dock-toolbar-left">
            {#each hiddenConfigs as cfg}
                <button class="dock-toolbar-btn" onclick={() => onShowWindow?.(cfg.id)}>
                    Show {cfg.title}
                </button>
            {/each}
        </div>
        <div class="dock-toolbar-right">
            <ThemeSwitcher current={currentTheme} />
        </div>
    </div>
{/if}

<style lang="scss">
    @use '../../styles/mixins' as *;

    .dock-toolbar {
        @include flex(row, space-between, center);
        height: 32px;
        min-height: 32px;
        padding: 0 8px;
        background: $bg-panel;
        border-bottom: 1px solid $border;
    }

    .dock-toolbar-left {
        @include flex(row, flex-start, center);
        gap: 6px;
    }

    .dock-toolbar-right {
        @include flex(row, flex-end, center);
    }

    .dock-toolbar-btn {
        @include btn-reset;
        @include transition(background, color, border-color);
        padding: 3px 10px;
        font-size: $fs-footnote1;
        color: $text-secondary;
        background: transparent;
        border: 1px solid $border;
        border-radius: $radius-sm;

        &:hover {
            background: $bg-surface;
            color: $text;
            border-color: $text-muted;
        }
    }
</style>
