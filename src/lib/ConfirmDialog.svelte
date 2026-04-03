<script lang="ts" module>
    export type DialogIcon = 'info' | 'question' | 'warning' | 'error' | 'fatal';
</script>

<script lang="ts">
    import type {PopupButton} from './MarkdownPopup.svelte';

    let {
        title,
        body,
        icon = 'info',
        buttons = [],
        onClose,
    }: {
        title: string;
        body: string;
        icon?: DialogIcon;
        buttons?: PopupButton[];
        onClose?: () => void;
    } = $props();

    function btnStyle(btn: PopupButton): string {
        const v: string[] = [];
        if (btn.bg)           v.push(`--_bg:${btn.bg}`);
        if (btn.color)        v.push(`--_color:${btn.color}`);
        if (btn.border)       v.push(`--_border:${btn.border}`);
        if (btn.hoverBg)      v.push(`--_hover-bg:${btn.hoverBg}`);
        if (btn.hoverColor)   v.push(`--_hover-color:${btn.hoverColor}`);
        if (btn.hoverBorder)  v.push(`--_hover-border:${btn.hoverBorder}`);
        if (btn.activeBg)     v.push(`--_active-bg:${btn.activeBg}`);
        if (btn.activeColor)  v.push(`--_active-color:${btn.activeColor}`);
        if (btn.activeBorder) v.push(`--_active-border:${btn.activeBorder}`);
        return v.join(';');
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') onClose?.();
    }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
    class="overlay"
    role="presentation"
    onclick={onClose}
    onkeydown={() => {}}
>
    <div
        class="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="cd-title"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.key !== 'Escape' && e.stopPropagation()}
    >
        <div class="dlg-content">
            <div class="dlg-icon dlg-icon--{icon}" aria-hidden="true">
                {#if icon === 'info'}
                    <svg viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a.75.75 0 000 1.5h.253a.25.25 0 01.244.304l-.459 2.066A1.75 1.75 0 0010.747 15H11a.75.75 0 000-1.5h-.253a.25.25 0 01-.244-.304l.459-2.066A1.75 1.75 0 009.253 9H9z" clip-rule="evenodd"/>
                    </svg>
                {:else if icon === 'question'}
                    <svg viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM8.94 6.94a.75.75 0 11-1.061-1.061 3 3 0 112.871 5.026v.345a.75.75 0 01-1.5 0v-.5c0-.72.57-1.172 1.081-1.287A1.5 1.5 0 108.94 6.94zM10 15a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd"/>
                    </svg>
                {:else if icon === 'warning'}
                    <svg viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd"/>
                    </svg>
                {:else if icon === 'error'}
                    <svg viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.28 7.22a.75.75 0 00-1.06 1.06L8.94 10l-1.72 1.72a.75.75 0 101.06 1.06L10 11.06l1.72 1.72a.75.75 0 101.06-1.06L11.06 10l1.72-1.72a.75.75 0 00-1.06-1.06L10 8.94 8.28 7.22z" clip-rule="evenodd"/>
                    </svg>
                {:else}
                    <svg viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M13.477 14.89A6 6 0 015.11 6.524l8.367 8.368zm1.414-1.414L6.524 5.11a6 6 0 018.367 8.367zM18 10a8 8 0 11-16 0 8 8 0 0116 0z" clip-rule="evenodd"/>
                    </svg>
                {/if}
            </div>
            <div class="dlg-text">
                <p class="dlg-title" id="cd-title">{title}</p>
                <p class="dlg-body">{body}</p>
            </div>
        </div>

        {#if buttons.length > 0}
            <div class="dlg-footer">
                {#each buttons as btn}
                    <button class="dlg-btn" style={btnStyle(btn)} onclick={btn.onClick}>
                        {btn.label}
                    </button>
                {/each}
            </div>
        {/if}
    </div>
</div>

<style lang="scss">
    @use '../styles/mixins' as *;

    .overlay {
        position: fixed;
        inset: 0;
        background: var(--overlay-bg);
        backdrop-filter: blur(3px);
        @include flex(row, center, center);
        z-index: 200;
    }

    .dialog {
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        width: 360px;
        max-width: calc(100vw - 48px);
        @include flex(column, flex-start, stretch);
        box-shadow: 0 12px 40px var(--shadow-heavy);
        overflow: hidden;
    }

    .dlg-content {
        @include flex(row, flex-start, flex-start);
        gap: 14px;
        padding: 22px 22px 18px;
    }

    .dlg-icon {
        flex-shrink: 0;
        width: 36px;
        height: 36px;
        margin-top: 1px;

        svg {
            width: 100%;
            height: 100%;
        }

        &--info     { color: #3b82f6; }
        &--question { color: #6366f1; }
        &--warning  { color: #f59e0b; }
        &--error    { color: var(--required-color); }
        &--fatal    { color: #b91c1c; }
    }

    .dlg-text {
        @include flex(column, flex-start, stretch);
        gap: 6px;
        min-width: 0;
    }

    .dlg-title {
        font-size: $fs-regular;
        font-weight: 600;
        color: $text;
        line-height: 1.3;
    }

    .dlg-body {
        font-size: $fs-small;
        color: $text-secondary;
        line-height: 1.5;
    }

    .dlg-footer {
        @include flex(row, flex-end, center);
        gap: 8px;
        padding: 12px 20px;
        border-top: 1px solid $border;
        background: $bg-surface;
        flex-shrink: 0;
    }

    .dlg-btn {
        @include btn-reset;
        @include transition(background, color, border-color);
        padding: 6px 16px;
        border-radius: $radius-sm;
        font-size: $fs-small;
        font-family: $font-base;
        border: 1px solid var(--_border, $border);
        background: var(--_bg, transparent);
        color: var(--_color, $text-secondary);
        cursor: pointer;

        &:hover {
            background:   var(--_hover-bg,    var(--_bg,    var(--hover-bg)));
            color:        var(--_hover-color,  var(--_color, $text));
            border-color: var(--_hover-border, var(--_border, $text-muted));
        }

        &:active {
            background:   var(--_active-bg,     var(--_hover-bg,    var(--_bg,    var(--hover-bg-strong))));
            color:        var(--_active-color,   var(--_hover-color, var(--_color, $text)));
            border-color: var(--_active-border,  var(--_hover-border, var(--_border, $text-muted)));
        }
    }
</style>
