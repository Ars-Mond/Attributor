<script lang="ts">
    import {progress} from '$lib/progress.svelte';
    import {t} from '$lib/i18n';
</script>

{#if progress.active}
    <!-- Top-most progress surface (above dialogs/settings). `blocking` swallows interaction. -->
    <div class="progress-overlay" class:progress-overlay--blocking={progress.blocking} role="alert" aria-live="polite">
        <div class="progress-card">
            <span class="progress-label">{progress.label}</span>

            {#if progress.determinate}
                <div class="progress-track">
                    <div class="progress-fill" style="width:{progress.percent}%"></div>
                </div>
                <span class="progress-count">
                    {progress.value}{#if progress.total}&nbsp;/&nbsp;{progress.total}{/if}
                </span>
            {:else}
                <div class="progress-track progress-track--indeterminate">
                    <div class="progress-fill progress-fill--indeterminate"></div>
                </div>
            {/if}

            {#if progress.cancelable}
                <button class="progress-cancel" onclick={() => progress.cancel()}>{t('common.cancel')}</button>
            {/if}
        </div>
    </div>
{/if}

<style lang="scss">
    @use 'styles/mixins' as *;

    .progress-overlay {
        position: fixed;
        inset: 0;
        z-index: 600; // above SettingsDialog (500) and dialogs/loading (200)
        @include flex(row, center, flex-start);
        padding-top: 22vh;
        pointer-events: none; // non-blocking by default

        &--blocking {
            pointer-events: all; // freeze: swallow all interaction
            background: var(--overlay-bg);
            backdrop-filter: blur(2px);
        }
    }

    .progress-card {
        pointer-events: all;
        @include flex(column, flex-start, stretch);
        gap: 10px;
        min-width: 320px;
        max-width: min(520px, calc(100vw - 48px));
        padding: 16px 18px;
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        box-shadow: 0 12px 40px var(--shadow-heavy);
    }

    .progress-label {
        font-size: $fs-small;
        color: $text;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .progress-track {
        position: relative;
        height: 6px;
        border-radius: $radius-sm;
        background: var(--hover-bg);
        overflow: hidden;
    }

    .progress-fill {
        height: 100%;
        background: $accent;
        border-radius: $radius-sm;
        @include transition(width);

        &--indeterminate {
            position: absolute;
            width: 35%;
            animation: indeterminate 1.1s ease-in-out infinite;
        }
    }

    @keyframes indeterminate {
        0% {left: -35%;}
        100% {left: 100%;}
    }

    .progress-count {
        font-size: $fs-footnote1;
        color: $text-muted;
        align-self: flex-end;
    }

    .progress-cancel {
        @include btn-reset;
        @include transition(background, color, border-color);
        align-self: flex-end;
        padding: 4px 14px;
        border: 1px solid $border;
        border-radius: $radius-sm;
        font-size: $fs-small;
        font-family: $font-base;
        color: $text-secondary;
        background: transparent;
        cursor: pointer;

        &:hover {
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }
    }
</style>
