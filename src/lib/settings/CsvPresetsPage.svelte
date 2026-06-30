<script lang="ts">
    import {settings} from './index';
    import {t} from '$lib/i18n';
    import CsvPresetDialog from './CsvPresetDialog.svelte';
    import {createEmptyPreset, type CsvPreset} from '$lib/csv/csv';
    import type {SettingSectionProps} from './SettingsSection';

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    let {section, resetSection}: SettingSectionProps = $props();

    const presets = $derived(settings.subscribe<CsvPreset[]>('csv.presets')() ?? []);

    let editing = $state<CsvPreset | null>(null);
    let isNew = $state(false);

    // identifiers used by OTHER presets than the one being edited — passed to the dialog for dedup.
    const takenIdentifiers = $derived(editing ? presets.filter(p => p.id !== editing!.id).map(p => p.identifier) : []);

    function create() {
        editing = createEmptyPreset();
        isNew = true;
    }

    function edit(p: CsvPreset) {
        editing = {...p, fields: p.fields.map(f => ({...f}))};
        isNew = false;
    }

    function remove(p: CsvPreset) {
        settings.set('csv.presets', presets.filter(x => x.id !== p.id));
    }

    function save(p: CsvPreset) {
        const exists = presets.some(x => x.id === p.id);
        settings.set('csv.presets', exists ? presets.map(x => (x.id === p.id ? p : x)) : [...presets, p]);
        editing = null;
    }
</script>

<div class="presets-page">
    <div class="mp-toolbar">
        <button class="mp-btn" onclick={create}>{t('common.create')}</button>
    </div>

    {#if presets.length === 0}
        <p class="mp-desc">{t('settings.csv.empty')}</p>
    {:else}
        <ul class="mp-list">
            {#each presets as p (p.id)}
                <li class="mp-row">
                    <span class="mp-name">{p.name || p.identifier || '—'}</span>
                    <span class="mp-model">{p.identifier}.csv</span>
                    <button class="mp-btn" onclick={() => edit(p)}>{t('common.edit')}</button>
                    <button class="mp-btn mp-btn--danger" onclick={() => remove(p)}>{t('common.delete')}</button>
                </li>
            {/each}
        </ul>
    {/if}
</div>

{#if editing}
    <CsvPresetDialog
        preset={editing}
        {isNew}
        {takenIdentifiers}
        onSave={save}
        onCancel={() => (editing = null)}
    />
{/if}

<style lang="scss">
    @use 'styles/mixins' as *;

    .presets-page {
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

    .mp-name {
        flex: 1;
        font-size: $fs-small;
        color: $text;
        font-weight: 500;
        overflow: hidden;
        text-overflow: ellipsis;
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
