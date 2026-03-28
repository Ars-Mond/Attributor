<script lang="ts" module>
    /** Button configuration for MarkdownPopup.
     *  All color fields accept any CSS color value or CSS variable (e.g. "var(--accent)").
     *  Omitted fields fall back through the cascade: active → hover → base → theme default.
     */
    export interface PopupButton {
        label: string;
        onClick: () => void;
        // Base state
        bg?: string;
        color?: string;
        border?: string;
        // Hover state (falls back to base if omitted)
        hoverBg?: string;
        hoverColor?: string;
        hoverBorder?: string;
        // Active / pressed state (falls back to hover if omitted)
        activeBg?: string;
        activeColor?: string;
        activeBorder?: string;
    }

    /** Absolute viewport position. If omitted the popup is centered on screen. */
    export interface PopupPosition {
        x: number;
        y: number;
    }
</script>

<script lang="ts">
    import SvelteMarkdown from '@humanspeak/svelte-markdown';

    let {
        source,
        width = 480,
        height,
        position,
        buttons = [],
        onClose,
        backdrop = true,
    }: {
        source: string;
        /** px number or any CSS length string */
        width?: number | string;
        /** px number or any CSS length string; if omitted the popup grows with content */
        height?: number | string;
        /** Absolute viewport coords. Omit to centre on screen. */
        position?: PopupPosition;
        buttons?: PopupButton[];
        onClose?: () => void;
        /** Show a dimmed backdrop. Default: true. */
        backdrop?: boolean;
    } = $props();

    const w = $derived(typeof width  === 'number' ? `${width}px`  : width);
    const h = $derived(typeof height === 'number' ? `${height}px` : height);

    const popupStyle = $derived(
        [
            `width:${w}`,
            h ? `height:${h}` : '',
            position
                ? `left:${position.x}px;top:${position.y}px`
                : 'top:50%;left:50%;transform:translate(-50%,-50%)',
        ].filter(Boolean).join(';')
    );

    /** Build inline CSS-variable overrides for a single button. */
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

{#if backdrop}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
        class="md-backdrop"
        role="presentation"
        onclick={() => onClose?.()}
        onkeydown={() => {}}
    ></div>
{/if}

<div
    class="md-popup"
    style={popupStyle}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key !== 'Escape' && e.stopPropagation()}
>
    <div class="md-body">
        <SvelteMarkdown {source} />
    </div>

    {#if buttons.length > 0}
        <div class="md-footer">
            {#each buttons as btn}
                <button class="md-btn" style={btnStyle(btn)} onclick={btn.onClick}>
                    {btn.label}
                </button>
            {/each}
        </div>
    {/if}
</div>

<style lang="scss">
    @use '../styles/mixins' as *;

    // ── Backdrop ──────────────────────────────────────────────────────────────

    .md-backdrop {
        position: fixed;
        inset: 0;
        z-index: 400;
        background: var(--overlay-bg);
        backdrop-filter: blur(3px);
    }

    // ── Popup shell ───────────────────────────────────────────────────────────

    .md-popup {
        position: fixed;
        z-index: 401;
        display: flex;
        flex-direction: column;
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        box-shadow: 0 12px 40px var(--shadow-heavy);
        overflow: hidden;
        max-width: calc(100vw - 32px);
        max-height: calc(100vh - 32px);
    }

    // ── Markdown body ─────────────────────────────────────────────────────────

    .md-body {
        flex: 1;
        overflow-y: auto;
        padding: 24px 28px;
        @include scrollbar;

        // ── Typography for rendered markdown ──

        :global(h1) { font-size: $fs-h4; font-weight: 700; color: $text;           margin: 0 0 16px; line-height: 1.25; }
        :global(h2) { font-size: $fs-h5; font-weight: 700; color: $text;           margin: 20px 0 12px; line-height: 1.3; }
        :global(h3) { font-size: $fs-h6; font-weight: 600; color: $text;           margin: 18px 0 10px; line-height: 1.35; }
        :global(h4) { font-size: $fs-regular; font-weight: 600; color: $text;      margin: 16px 0 8px; }
        :global(h5),
        :global(h6) { font-size: $fs-small; font-weight: 600; color: $text-secondary; margin: 14px 0 6px; }

        :global(p)  { font-size: $fs-small; color: $text-secondary; line-height: 1.7; margin: 0 0 12px; }

        :global(ul),
        :global(ol) { font-size: $fs-small; color: $text-secondary; line-height: 1.7; margin: 0 0 12px; padding-left: 20px; }

        :global(li) { margin-bottom: 4px; }

        :global(a)  { color: $accent; text-decoration: none;
            &:hover { text-decoration: underline; }
        }

        :global(strong) { font-weight: 600; color: $text; }
        :global(em)     { font-style: italic; }

        :global(code) {
            font-family: $font-mono;
            font-size: $fs-footnote1;
            background: $bg-surface;
            border: 1px solid $border;
            border-radius: $radius-sm;
            padding: 1px 5px;
            color: $text;
        }

        :global(pre) {
            background: $bg-surface;
            border: 1px solid $border;
            border-radius: $radius-md;
            padding: 14px 16px;
            overflow-x: auto;
            margin: 0 0 14px;
            @include scrollbar(4px);

            :global(code) {
                background: none;
                border: none;
                padding: 0;
                font-size: $fs-footnote1;
                line-height: 1.6;
            }
        }

        :global(blockquote) {
            border-left: 3px solid $accent;
            margin: 0 0 14px;
            padding: 6px 0 6px 14px;
            color: $text-muted;
            font-size: $fs-small;
            font-style: italic;
        }

        :global(hr) {
            border: none;
            border-top: 1px solid $border;
            margin: 20px 0;
        }

        :global(table) {
            width: 100%;
            border-collapse: collapse;
            font-size: $fs-small;
            margin-bottom: 14px;
        }

        :global(th),
        :global(td) {
            padding: 6px 12px;
            text-align: left;
            border-bottom: 1px solid $border;
        }

        :global(th) {
            font-weight: 600;
            color: $text;
            background: $bg-surface;
        }

        :global(td) { color: $text-secondary; }

        :global(img) {
            max-width: 100%;
            border-radius: $radius-md;
        }

        // Remove bottom margin from last child
        :global(> *:last-child) { margin-bottom: 0; }
    }

    // ── Footer / buttons ──────────────────────────────────────────────────────

    .md-footer {
        @include flex(row, flex-end, center);
        gap: 8px;
        padding: 12px 20px;
        border-top: 1px solid $border;
        background: $bg-surface;
        flex-shrink: 0;
    }

    // ── Button — all colors driven by CSS variables with theme fallbacks ──────
    //
    //   Cascade for each property:
    //     active  → --_active-*
    //     hover   → --_hover-*   (falls back to base)
    //     base    → --_*         (falls back to theme defaults)

    .md-btn {
        @include btn-reset;
        @include transition(background, color, border-color, box-shadow);
        padding: 6px 16px;
        border-radius: $radius-sm;
        font-size: $fs-small;
        font-family: $font-base;
        border: 1px solid var(--_border, $border);
        background: var(--_bg, transparent);
        color: var(--_color, $text-secondary);
        cursor: pointer;

        &:hover {
            background:   var(--_hover-bg,     var(--_bg,     var(--hover-bg)));
            color:        var(--_hover-color,   var(--_color,  $text));
            border-color: var(--_hover-border,  var(--_border, $text-muted));
        }

        &:active {
            background:   var(--_active-bg,     var(--_hover-bg,    var(--_bg,    var(--hover-bg-strong))));
            color:        var(--_active-color,   var(--_hover-color, var(--_color, $text)));
            border-color: var(--_active-border,  var(--_hover-border, var(--_border, $text-muted)));
        }
    }
</style>
