<script lang="ts">
    import {settings} from './index';
    import {t} from '$lib/i18n';
    import ModelProfileDialog from './ModelProfileDialog.svelte';
    import {ai} from '$lib/ai/availability.svelte';
    import {profileProvider, type ModelProfile} from '$lib/ai/ai';
    import {profileProviderInfo} from '$lib/ai/providers';
    import type {SettingSectionProps} from './SettingsSection';

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    let {section, resetSection}: SettingSectionProps = $props();

    const profiles = $derived(settings.subscribe<ModelProfile[]>('ai.modelProfiles')() ?? []);

    let editing = $state<ModelProfile | null>(null);
    let isNew = $state(false);

    // provider + modelId pairs used by OTHER profiles than the one being edited — passed to the dialog for dedup.
    const takenModels = $derived(editing ? profiles.filter(p => p.id !== editing!.id).map(p => ({provider: profileProvider(p), modelId: p.modelId})) : []);

    // ── Reordering (pointer-based, ghost + placeholder, like the keyword chips) ──
    let listEl = $state<HTMLUListElement | undefined>(undefined);
    let dragFrom = $state<number | null>(null);
    let dropAt = $state<number | null>(null);   // insert index among the rows that are not dragged
    let placeholderHeight = $state(0);

    // While dragging, the dragged row leaves the list and an empty slot marks where it will land.
    type Row = {key: string; profile: ModelProfile | null};
    const rows = $derived.by((): Row[] => {
        const list: Row[] = profiles.map(p => ({key: p.id, profile: p}));
        if (dragFrom === null || dropAt === null) return list;
        list.splice(dragFrom, 1);
        list.splice(dropAt, 0, {key: '__placeholder__', profile: null});
        return list;
    });

    function startDrag(e: PointerEvent, index: number) {
        if (dragFrom !== null || profiles.length < 2 || e.button !== 0) return;
        e.preventDefault();
        const rowEl = (e.currentTarget as HTMLElement).closest<HTMLElement>('.mp-row');
        if (!rowEl || !listEl) return;
        const rect = rowEl.getBoundingClientRect();
        const offsetY = e.clientY - rect.top;
        // Midlines of the other rows, measured once: the drop index is how many of them lie above the pointer.
        const midlines = [...listEl.querySelectorAll<HTMLElement>('.mp-row')]
            .filter(el => el !== rowEl)
            .map(el => {const r = el.getBoundingClientRect(); return r.top + r.height / 2;});

        const ghost = rowEl.cloneNode(true) as HTMLElement;
        Object.assign(ghost.style, {
            position: 'fixed',
            left: `${rect.left}px`,
            top: `${rect.top}px`,
            width: `${rect.width}px`,
            margin: '0',
            pointerEvents: 'none',
            zIndex: '9999',
            cursor: 'grabbing',
            boxShadow: '0 4px 16px rgba(0,0,0,0.35)',
            opacity: '0.95'
        });
        document.body.appendChild(ghost);
        document.body.style.cursor = 'grabbing';

        placeholderHeight = rect.height;
        dragFrom = index;
        dropAt = index;

        const onMove = (ev: PointerEvent) => {
            ghost.style.top = `${ev.clientY - offsetY}px`;
            const next = midlines.filter(y => y < ev.clientY).length;
            if (next !== dropAt) dropAt = next;
        };
        const finish = (commit: boolean) => {
            ghost.remove();
            document.body.style.cursor = '';
            if (commit && dragFrom !== null && dropAt !== null && dropAt !== dragFrom) {
                const next = [...profiles];
                const [moved] = next.splice(dragFrom, 1);
                next.splice(dropAt, 0, moved);
                settings.set('ai.modelProfiles', next);
            }
            dragFrom = null;
            dropAt = null;
            document.removeEventListener('pointermove', onMove);
            document.removeEventListener('pointerup', onUp);
            document.removeEventListener('pointercancel', onCancel);
        };
        const onUp = () => finish(true);
        const onCancel = () => finish(false);
        document.addEventListener('pointermove', onMove);
        document.addEventListener('pointerup', onUp);
        document.addEventListener('pointercancel', onCancel);
    }

    function uid(): string {
        return 'p' + Math.random().toString(36).slice(2, 10);
    }

    function create() {
        editing = {id: uid(), name: '', provider: ai.provider, modelId: '', prompt: '', think: null, keepAlive: null, options: {}};
        isNew = true;
    }

    function edit(p: ModelProfile) {
        editing = {...p, provider: profileProvider(p), options: {...p.options}};
        isNew = false;
    }

    /** `name` with a number appended or incremented ("Base" → "Base 2", "Base 2" → "Base 3"), skipping names other profiles use.
     *  A trailing number counts as a copy number only when the name without it exists, so "Llama 3" becomes "Llama 3 2". */
    function nextName(name: string): string {
        const names = new Set(profiles.map(p => p.name));
        const m = /^(.*\S)\s+(\d+)$/.exec(name);
        const [stem, first] = m && names.has(m[1]) ? [m[1], Number(m[2]) + 1] : [name, 2];
        let n = first;
        while (names.has(`${stem} ${n}`)) n++;
        return `${stem} ${n}`;
    }

    // A copy opens as a new profile: it is saved only once its provider/model pair is unique, like any new profile.
    function duplicate(p: ModelProfile) {
        editing = {...p, id: uid(), name: nextName(p.name || p.modelId), provider: profileProvider(p), options: {...p.options}};
        isNew = true;
    }

    function remove(p: ModelProfile) {
        settings.set('ai.modelProfiles', profiles.filter(x => x.id !== p.id));
    }

    function save(p: ModelProfile) {
        const exists = profiles.some(x => x.id === p.id);
        settings.set('ai.modelProfiles', exists ? profiles.map(x => (x.id === p.id ? p : x)) : [...profiles, p]);
        editing = null;
    }
</script>

<div class="models-page">
    <div class="mp-toolbar">
        <button class="mp-btn" onclick={create}>{t('common.create')}</button>
    </div>

    {#if profiles.length === 0}
        <p class="mp-desc">{t('settings.modelProfiles.empty')}</p>
    {:else}
        <ul class="mp-list" bind:this={listEl}>
            {#each rows as row (row.key)}
                {#if row.profile === null}
                    <li class="mp-placeholder" style="height:{placeholderHeight}px"></li>
                {:else}
                    {@const p = row.profile}
                    <li class="mp-row">
                        <span
                            class="mp-handle"
                            role="button"
                            tabindex="-1"
                            title={t('settings.modelProfiles.reorder')}
                            aria-label={t('settings.modelProfiles.reorder')}
                            onpointerdown={(e) => startDrag(e, profiles.indexOf(p))}
                        >
                            <svg viewBox="0 0 10 16" fill="currentColor" aria-hidden="true">
                                <circle cx="3" cy="3" r="1.4"/><circle cx="7" cy="3" r="1.4"/>
                                <circle cx="3" cy="8" r="1.4"/><circle cx="7" cy="8" r="1.4"/>
                                <circle cx="3" cy="13" r="1.4"/><circle cx="7" cy="13" r="1.4"/>
                            </svg>
                        </span>
                        <span class="mp-name">{p.name || p.modelId || '—'}</span>
                        <span class="mp-provider">{profileProviderInfo(profileProvider(p)).label}</span>
                        <span class="mp-model">{p.modelId}</span>
                        <button class="mp-btn" onclick={() => duplicate(p)}>{t('common.duplicate')}</button>
                        <button class="mp-btn" onclick={() => edit(p)}>{t('common.edit')}</button>
                        <button class="mp-btn mp-btn--danger" onclick={() => remove(p)}>{t('common.delete')}</button>
                    </li>
                {/if}
            {/each}
        </ul>
    {/if}
</div>

{#if editing}
    <ModelProfileDialog
        profile={editing}
        {isNew}
        {takenModels}
        onSave={save}
        onCancel={() => (editing = null)}
    />
{/if}

<style lang="scss">
    @use 'styles/mixins' as *;

    .models-page {
        @include flex(column, flex-start, stretch);
        gap: 10px;
    }

    .mp-toolbar {
        @include flex(row, flex-start, center);
    }

    .mp-list {
        @include flex(column, flex-start, stretch);
        gap: 4px;
        list-style: none;
        margin: 0;
        padding: 0;
    }

    .mp-row {
        @include flex(row, flex-start, center);
        gap: 8px;
        padding: 8px 10px;
        border: 1px solid $border;
        border-radius: $radius-sm;
        background: $bg-surface;
    }

    .mp-handle {
        @include flex(row, center, center);
        flex-shrink: 0;
        width: 14px;
        height: 20px;
        margin-left: -4px;
        color: $text-muted;
        cursor: grab;
        touch-action: none;   // keep touch/pen drags from scrolling the page
        @include transition(color);

        &:hover {color: $text;}

        svg {width: 10px; height: 16px;}
    }

    .mp-placeholder {
        border: 1px dashed $border;
        border-radius: $radius-sm;
    }

    .mp-name {
        flex: 1;
        font-size: $fs-small;
        color: $text;
        font-weight: 500;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .mp-provider {
        flex-shrink: 0;
        padding: 1px 6px;
        border: 1px solid $border;
        border-radius: $radius-sm;
        font-size: $fs-footnote2;
        color: $text-muted;
        white-space: nowrap;
    }

    .mp-model {
        font-size: $fs-footnote1;
        color: $text-muted;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        max-width: 40%;
    }

    .mp-desc {
        font-size: $fs-footnote1;
        color: $text-muted;
        margin: 0;
    }

    .mp-btn {
        @include btn-reset;
        @include transition(background, color, border-color);
        padding: 4px 12px;
        white-space: nowrap;
        border: 1px solid $border;
        border-radius: $radius-sm;
        font-size: $fs-footnote1;
        font-family: $font-base;
        color: $text-secondary;
        background: transparent;
        cursor: pointer;
        flex-shrink: 0;

        &:hover {
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }

        &--danger:hover {
            background: var(--required-alpha-08);
            color: var(--required-color);
            border-color: var(--required-color);
        }
    }
</style>
