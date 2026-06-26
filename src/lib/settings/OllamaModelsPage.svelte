<script lang="ts">
    import {onMount} from 'svelte';
    import {settings} from './index';
    import {t} from '$lib/i18n';
    import OllamaModelDialog from './OllamaModelDialog.svelte';
    import {listModels, type ModelProfile} from '$lib/ollama/ollama';
    import {OFFERED_MODELS} from '$lib/ollama/models';
    import type {SettingSectionProps} from './SettingsSection';

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    let {section, resetSection}: SettingSectionProps = $props();

    const profiles = $derived(settings.subscribe<ModelProfile[]>('ollama.modelProfiles')() ?? []);

    // Suggestions for the model-id field: offered models + already-installed models.
    let installed = $state<string[]>([]);
    const modelOptions = $derived([...new Set([...OFFERED_MODELS.map(m => m.id), ...installed])]);

    onMount(async () => {
        try {installed = (await listModels()).map(m => m.name);}
        catch {installed = [];}
    });

    let editing = $state<ModelProfile | null>(null);
    let isNew = $state(false);

    function uid(): string {
        return 'p' + Math.random().toString(36).slice(2, 10);
    }

    function create() {
        editing = {id: uid(), name: '', modelId: '', prompt: '', think: null, keepAlive: null, options: {}};
        isNew = true;
    }

    function edit(p: ModelProfile) {
        editing = {...p, options: {...p.options}};
        isNew = false;
    }

    function remove(p: ModelProfile) {
        settings.set('ollama.modelProfiles', profiles.filter(x => x.id !== p.id));
    }

    function save(p: ModelProfile) {
        const exists = profiles.some(x => x.id === p.id);
        settings.set('ollama.modelProfiles', exists ? profiles.map(x => (x.id === p.id ? p : x)) : [...profiles, p]);
        editing = null;
    }
</script>

<div class="models-page">
    <div class="mp-toolbar">
        <button class="mp-btn" onclick={create}>{t('common.create')}</button>
    </div>

    {#if profiles.length === 0}
        <p class="mp-desc">{t('settings.ollamaModels.empty')}</p>
    {:else}
        <ul class="mp-list">
            {#each profiles as p (p.id)}
                <li class="mp-row">
                    <span class="mp-name">{p.name || p.modelId || '—'}</span>
                    <span class="mp-model">{p.modelId}</span>
                    <button class="mp-btn" onclick={() => edit(p)}>{t('common.edit')}</button>
                    <button class="mp-btn mp-btn--danger" onclick={() => remove(p)}>{t('common.delete')}</button>
                </li>
            {/each}
        </ul>
    {/if}
</div>

{#if editing}
    <OllamaModelDialog profile={editing} {isNew} {modelOptions} onSave={save} onCancel={() => (editing = null)} />
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
