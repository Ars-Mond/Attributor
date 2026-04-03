<script lang="ts">
    import {onMount, onDestroy} from 'svelte';
    import {readText} from '@tauri-apps/plugin-clipboard-manager';

    // Approximate menu dimensions for viewport clamping before DOM measurement
    const MENU_W = 170;
    const MENU_H = 70;

    let visible = $state(false);
    let x = $state(0);
    let y = $state(0);
    let hasSelection = $state(false);
    let menuEl = $state<HTMLElement | null>(null);

    let target: HTMLInputElement | HTMLTextAreaElement | null = null;
    let savedStart = 0;
    let savedEnd = 0;

    function onContextMenu(e: MouseEvent) {
        const el = e.target as HTMLElement;
        if (el.tagName !== 'INPUT' && el.tagName !== 'TEXTAREA') return;
        const inp = el as HTMLInputElement | HTMLTextAreaElement;
        // Skip non-text inputs
        if ('type' in inp && ['checkbox', 'radio', 'range', 'color', 'file'].includes(inp.type)) return;

        e.preventDefault();

        target = inp;
        savedStart = inp.selectionStart ?? 0;
        savedEnd   = inp.selectionEnd   ?? 0;
        hasSelection = savedStart !== savedEnd;

        // Clamp position so menu stays in viewport
        x = Math.min(e.clientX, window.innerWidth  - MENU_W - 4);
        y = Math.min(e.clientY, window.innerHeight - MENU_H - 4);
        visible = true;
    }

    function onWindowClick(e: MouseEvent) {
        if (visible && menuEl && !menuEl.contains(e.target as Node)) {
            visible = false;
        }
    }

    function onKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') visible = false;
    }

    function copy() {
        visible = false;
        if (!target) return;
        target.focus();
        target.setSelectionRange(savedStart, savedEnd);
        document.execCommand('copy');
    }

    async function paste() {
        visible = false;
        if (!target) return;
        const text = await readText();
        if (!text) return;
        target.focus();
        target.setSelectionRange(savedEnd, savedEnd);
        document.execCommand('insertText', false, text);
    }

    onMount(() => {
        window.addEventListener('contextmenu', onContextMenu);
        window.addEventListener('click', onWindowClick, true);
        window.addEventListener('keydown', onKeydown);
    });

    onDestroy(() => {
        window.removeEventListener('contextmenu', onContextMenu);
        window.removeEventListener('click', onWindowClick, true);
        window.removeEventListener('keydown', onKeydown);
    });
</script>

{#if visible}
    <ul
        class="ctx-menu"
        bind:this={menuEl}
        role="menu"
        style="left:{x}px;top:{y}px"
    >
        <li
            class="ctx-item"
            class:ctx-item--disabled={!hasSelection}
            role="menuitem"
            tabindex={hasSelection ? 0 : -1}
            onclick={copy}
            onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && copy()}
        >
            <span>Copy</span>
            <span class="ctx-shortcut">Ctrl+C</span>
        </li>
        <li
            class="ctx-item"
            role="menuitem"
            tabindex="0"
            onclick={paste}
            onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && paste()}
        >
            <span>Paste</span>
            <span class="ctx-shortcut">Ctrl+V</span>
        </li>
    </ul>
{/if}

<style lang="scss">
    @use '../../styles/mixins' as *;

    .ctx-menu {
        position: fixed;
        z-index: 1300;
        list-style: none;
        margin: 0;
        padding: 3px 0;
        min-width: 160px;
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        box-shadow: 0 6px 20px var(--shadow);
    }

    .ctx-item {
        @include flex(row, space-between, center);
        padding: 5px 12px;
        gap: 24px;
        font-size: $fs-small;
        color: $text-secondary;
        cursor: pointer;
        user-select: none;
        @include transition(background, color);

        &:hover:not(.ctx-item--disabled) {
            background: $bg-input-focus;
            color: $text;
        }

        &--disabled {
            opacity: 0.4;
            cursor: default;
            pointer-events: none;
        }
    }

    .ctx-shortcut {
        font-size: $fs-footnote1;
        color: $text-muted;
        flex-shrink: 0;
    }
</style>
