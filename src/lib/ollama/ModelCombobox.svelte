<script lang="ts">
    import {t} from '$lib/i18n';
    import {KNOWN_MODELS, type ModelKind} from './models';

    let {
        value,
        installed = [],
        includeBase = false,
        placeholder = '',
        onChange
    }: {
        value: string;
        installed?: string[];
        includeBase?: boolean;       // include the special "base" fallback id in suggestions
        placeholder?: string;
        onChange: (v: string) => void;
    } = $props();

    // Internal input state, seeded once from the incoming value (read in a closure — deliberate snapshot).
    let text = $state((() => value)());
    let open = $state(false);
    let inputEl = $state<HTMLInputElement | undefined>(undefined);

    interface Suggestion {
        id: string;
        installed: boolean;
        kind: ModelKind | 'base';
    }

    const all = $derived.by((): Suggestion[] => {
        const map = new Map<string, Suggestion>();
        if (includeBase) map.set('base', {id: 'base', installed: false, kind: 'base'});
        for (const m of KNOWN_MODELS) map.set(m.id, {id: m.id, installed: installed.includes(m.id), kind: m.kind});
        for (const id of installed) if (!map.has(id)) map.set(id, {id, installed: true, kind: 'local'});
        return [...map.values()];
    });

    const filtered = $derived.by(() => {
        const q = text.trim().toLowerCase();
        const list = q ? all.filter(s => s.id.toLowerCase().includes(q)) : all;
        // installed first, then local, then cloud; base stays on top.
        const rank = (s: Suggestion) => (s.kind === 'base' ? -1 : s.installed ? 0 : s.kind === 'local' ? 1 : 2);
        return [...list].sort((a, b) => rank(a) - rank(b) || a.id.localeCompare(b.id));
    });

    function update(v: string) {
        text = v;
        onChange(v);
    }

    function pick(id: string) {
        update(id);
        open = false;
        inputEl?.blur();
    }

    function tagLabel(s: Suggestion): string {
        if (s.kind === 'base') return 'base';
        if (s.installed) return t('ollama.model.installed');
        return s.kind === 'cloud' ? t('ollama.model.cloud') : t('ollama.model.local');
    }
</script>

<div class="combobox">
    <input
        bind:this={inputEl}
        class="cb-input"
        type="text"
        {placeholder}
        value={text}
        oninput={(e) => {update(e.currentTarget.value); open = true;}}
        onfocus={() => open = true}
        onblur={() => setTimeout(() => (open = false), 120)}
    />
    {#if open && filtered.length > 0}
        <ul class="cb-list">
            {#each filtered as s (s.id)}
                <li>
                    <!-- onmousedown (not click) so selection wins the race against the input's blur. -->
                    <button
                        type="button"
                        class="cb-item"
                        class:cb-item--installed={s.installed}
                        class:cb-item--local={!s.installed && s.kind === 'local'}
                        class:cb-item--base={s.kind === 'base'}
                        onmousedown={(e) => {e.preventDefault(); pick(s.id);}}
                    >
                        <span class="cb-id">{s.id}</span>
                        <span class="cb-tag">{tagLabel(s)}</span>
                    </button>
                </li>
            {/each}
        </ul>
    {/if}
</div>

<style lang="scss">
    @use 'styles/mixins' as *;

    .combobox {
        position: relative;
    }

    .cb-input {
        width: 100%;
        background: $bg-input;
        border: 1px solid $border;
        border-radius: $radius-sm;
        color: $text;
        font-size: $fs-small;
        font-family: $font-base;
        padding: 5px 8px;
        @include transition(border-color, background);

        &:focus {
            outline: none;
            border-color: $border-focus;
            background: $bg-input-focus;
        }
    }

    .cb-list {
        position: absolute;
        top: calc(100% + 2px);
        left: 0;
        right: 0;
        z-index: 10;
        max-height: 220px;
        overflow-y: auto;
        @include scrollbar;
        list-style: none;
        margin: 0;
        padding: 4px;
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-sm;
        box-shadow: 0 8px 24px var(--shadow-heavy);
    }

    .cb-item {
        @include btn-reset;
        @include flex(row, space-between, center);
        gap: 8px;
        width: 100%;
        padding: 5px 8px;
        border-radius: $radius-sm;
        font-size: $fs-small;
        color: $text;
        cursor: pointer;
        text-align: left;

        &:hover {background: var(--hover-bg-strong);}

        // Background tints (status colors, like the file-status dots). Cloud models stay default.
        &--local {background: rgba(96, 165, 250, 0.12);}       // local (not installed) → blue tint
        &--installed {background: rgba(74, 222, 128, 0.14);}   // installed → green tint (wins over local)
        &--base {background: var(--hover-bg);}                  // base fallback → neutral
    }

    .cb-id {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .cb-tag {
        flex-shrink: 0;
        font-size: $fs-footnote2;
        color: $text-muted;
        text-transform: lowercase;
    }
</style>
