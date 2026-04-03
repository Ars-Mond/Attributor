<script lang="ts">
    import {invoke} from "@tauri-apps/api/core";

    let {
        inputEl,
        query,
        onSelect,
    }: {
        inputEl: HTMLInputElement | undefined;
        query: string;
        onSelect: (kw: string) => void;
    } = $props();

    let suggestions = $state<string[]>([]);
    let selectedIdx = $state(-1);
    let suggestEl = $state<HTMLUListElement | undefined>(undefined);
    let suggestTop = $state(0);
    let suggestLeft = $state(0);
    let suggestWidth = $state(0);
    let searchTimer: ReturnType<typeof setTimeout> | null = null;

    function updateSuggestPos() {
        if (!inputEl) return;
        const r = inputEl.getBoundingClientRect();
        suggestTop = r.bottom + 2;
        suggestLeft = r.left;
        suggestWidth = r.width;
    }

    async function runSearch(q: string) {
        suggestions = await invoke<string[]>('search_keywords', {query: q});
        selectedIdx = -1;
        updateSuggestPos();
    }

    export function close() {
        suggestions = [];
        selectedIdx = -1;
    }

    function selectSuggestion(kw: string) {
        onSelect(kw);
        close();
    }

    function scrollSelectedIntoView() {
        if (!suggestEl || selectedIdx < 0) return;
        const item = suggestEl.children[selectedIdx] as HTMLElement | undefined;
        item?.scrollIntoView({block: 'nearest'});
    }

    /** Returns true if the event was consumed. */
    export function handleKeydown(e: KeyboardEvent): boolean {
        if (suggestions.length === 0) return false;
        if (e.key === 'ArrowDown') {
            e.preventDefault();
            selectedIdx = Math.min(selectedIdx + 1, suggestions.length - 1);
            scrollSelectedIntoView();
            return true;
        }
        if (e.key === 'ArrowUp') {
            e.preventDefault();
            selectedIdx = Math.max(selectedIdx - 1, -1);
            scrollSelectedIntoView();
            return true;
        }
        if (e.key === 'Escape') {
            close();
            return true;
        }
        if (e.key === 'Enter' && selectedIdx >= 0) {
            e.preventDefault();
            selectSuggestion(suggestions[selectedIdx]);
            return true;
        }
        return false;
    }

    /** Call from the input's blur handler. */
    export function handleBlur() {
        setTimeout(close, 150);
    }

    $effect(() => {
        const q = query;
        if (searchTimer) clearTimeout(searchTimer);
        if (q.trim()) {
            searchTimer = setTimeout(() => runSearch(q), 150);
        } else {
            close();
        }
        return () => {
            if (searchTimer) clearTimeout(searchTimer);
        };
    });
</script>

{#if suggestions.length > 0}
    <ul
        bind:this={suggestEl}
        class="kw-suggestions"
        style="top:{suggestTop}px; left:{suggestLeft}px; width:{suggestWidth}px"
    >
        {#each suggestions as kw, i}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <li
                class="kw-suggestion-item"
                class:kw-suggestion-item--selected={i === selectedIdx}
                onmousedown={(e) => { e.preventDefault(); selectSuggestion(kw); }}
            >{kw}</li>
        {/each}
    </ul>
{/if}

<style lang="scss">
    @use '../../styles/mixins' as *;

    .kw-suggestions {
        position: fixed;
        z-index: 1000;
        background: $bg-panel;
        border: 1px solid $border-focus;
        border-radius: $radius-md;
        list-style: none;
        padding: 3px 0;
        box-shadow: 0 6px 20px var(--shadow);
        max-height: 300px;
        overflow-y: auto;
        @include scrollbar;
    }

    .kw-suggestion-item {
        padding: 5px 10px;
        font-size: $fs-small;
        color: $text-secondary;
        cursor: pointer;
        @include transition(background, color);

        &:hover, &--selected {
            background: $bg-input-focus;
            color: $text;
        }
    }
</style>
