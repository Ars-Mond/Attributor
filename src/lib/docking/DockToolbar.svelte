<script lang="ts">
    import type {WindowConfig} from './dockTypes';

    let {
        hiddenWindows = [],
        windowConfigs = [],
        onShowWindow,
    }: {
        hiddenWindows?: string[];
        windowConfigs?: WindowConfig[];
        onShowWindow?: (id: string) => void;
    } = $props();

    const hiddenConfigs = $derived(
        windowConfigs.filter(c => hiddenWindows.includes(c.id))
    );
</script>

{#if hiddenConfigs.length > 0}
    <div class="dock-toolbar">
        {#each hiddenConfigs as cfg}
            <button class="dock-toolbar-btn" onclick={() => onShowWindow?.(cfg.id)}>
                Show {cfg.title}
            </button>
        {/each}
    </div>
{/if}

<style lang="scss">
    @use '../../styles/mixins' as *;

    .dock-toolbar {
        @include flex(row, flex-start, center);
        height: 32px;
        min-height: 32px;
        padding: 0 8px;
        gap: 6px;
        background: $bg-panel;
        border-bottom: 1px solid $border;
        flex-shrink: 0;
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
