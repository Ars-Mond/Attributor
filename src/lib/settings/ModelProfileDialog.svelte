<script lang="ts">
    import {t} from '$lib/i18n';
    import ModelCombobox from '$lib/ai/ModelCombobox.svelte';
    import {ai} from '$lib/ai/availability.svelte';
    import {profileProvider, type ModelProfile} from '$lib/ai/ai';
    import {PROFILE_PROVIDERS, PROVIDERS, profileProviderInfo, type ProfileProviderId} from '$lib/ai/providers';

    let {profile, isNew, takenModels = [], closeOnBackdrop = false, onSave, onCancel}: {
        profile: ModelProfile;
        isNew: boolean;
        takenModels?: {provider: ProfileProviderId; modelId: string}[];   // used by OTHER profiles — disallow duplicates
        closeOnBackdrop?: boolean;   // whether clicking the empty backdrop dismisses the dialog
        onSave: (p: ModelProfile) => void;
        onCancel: () => void;
    } = $props();

    // Seed local editable state once from the prop (the dialog is recreated per open via {#if editing}).
    // Read inside a closure so it's a deliberate one-time snapshot, not a reactive dependency.
    const seed = (() => profile)();
    let name = $state(seed.name);
    let provider = $state<ProfileProviderId>(profileProvider(seed));
    let modelId = $state(seed.modelId);
    let prompt = $state(seed.prompt);
    let think = $state(seed.think == null ? '' : String(seed.think));
    let keepAlive = $state(seed.keepAlive ?? '');
    let optionsText = $state(JSON.stringify(seed.options ?? {}, null, 2));
    let optionsError = $state<string | null>(null);
    let modelError = $state<string | null>(null);

    const info = $derived(profileProviderInfo(provider));

    // Model suggestions come from the selected provider; fetch its list on first use (the active provider is prefetched).
    // A Base profile suggests the models already listed by any provider.
    const fetched = new Set<ProfileProviderId>();
    $effect(() => {
        const id = provider;
        if (id !== 'base' && !fetched.has(id) && ai.modelsOf(id).length === 0) {
            fetched.add(id);
            void ai.refreshModels(id);
        }
    });
    const listedModels = $derived(provider === 'base' ? [...new Set(PROVIDERS.flatMap(p => ai.modelsOf(p.id)))] : ai.modelsOf(provider));
    // Keep alive is an Ollama setting; a Base profile carries it for when Ollama runs the request.
    const showKeepAlive = $derived(provider === 'ollama' || provider === 'base');

    // Thinking values differ per provider; a value the new provider does not offer is cleared.
    function changeProvider(id: ProfileProviderId) {
        provider = id;
        if (think && !profileProviderInfo(id).thinkOptions.includes(think)) think = '';
        modelError = null;
    }

    function parseThink(v: string): boolean | string | null {
        if (v === '') return null;
        if (v === 'true') return true;
        if (v === 'false') return false;
        return v;
    }

    function save() {
        const id = modelId.trim();
        if (!id) {
            modelError = t('settings.modelProfile.modelId.hint');
            return;
        }
        if (takenModels.some(m => m.provider === provider && m.modelId === id)) {
            modelError = t('settings.modelProfile.duplicateModel');
            return;
        }
        modelError = null;

        let options: Record<string, number | string | boolean> = {};
        const txt = optionsText.trim();
        if (txt) {
            try {
                options = JSON.parse(txt);
                optionsError = null;
            } catch (e) {
                optionsError = e instanceof Error ? e.message : String(e);
                return;
            }
        }
        onSave({
            ...profile,
            name: name.trim(),
            provider,
            modelId: modelId.trim(),
            prompt,
            think: parseThink(think),
            keepAlive: showKeepAlive ? keepAlive.trim() || null : null,
            options
        });
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') onCancel();
    }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="overlay" role="presentation" onclick={() => {if (closeOnBackdrop) onCancel();}} onkeydown={() => {}}>
    <div class="dialog" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
        <div class="dlg-header">
            <span class="dlg-title">{t(isNew ? 'settings.modelProfile.createTitle' : 'settings.modelProfile.editTitle')}</span>
        </div>

        <div class="dlg-body">
            <label class="md-field">
                <span class="md-label">{t('settings.modelProfile.name')}</span>
                <input class="md-input" type="text" bind:value={name} />
            </label>
            <label class="md-field">
                <span class="md-label">{t('settings.modelProfile.provider')}</span>
                <select class="md-input" value={provider} onchange={(e) => changeProvider(e.currentTarget.value as ProfileProviderId)}>
                    {#each PROFILE_PROVIDERS as p (p.id)}
                        <option value={p.id}>{p.id === 'base' ? t('settings.modelProfile.provider.base') : p.label}</option>
                    {/each}
                </select>
                <p class="md-desc">{t('settings.modelProfile.provider.hint')}</p>
            </label>
            <div class="md-field">
                <span class="md-label">{t('settings.modelProfile.modelId')}</span>
                {#key provider}
                    <ModelCombobox
                        value={modelId}
                        known={info.known}
                        installed={listedModels}
                        installedLabel={provider === 'ollama' ? 'ai.model.installed' : 'ai.model.available'}
                        includeBase={true}
                        placeholder={info.known[0]?.id ?? 'base'}
                        onChange={(v) => {modelId = v; modelError = null;}}
                    />
                {/key}
                <p class="md-desc">{t('settings.modelProfile.modelId.hint')}</p>
                {#if modelError}<p class="md-desc md-desc--err">{modelError}</p>{/if}
            </div>
            <label class="md-field">
                <span class="md-label">{t('settings.modelProfile.prompt')}</span>
                <textarea class="md-input md-textarea" rows={5} bind:value={prompt}></textarea>
            </label>
            <label class="md-field">
                <span class="md-label">{t('settings.modelProfile.think')}</span>
                <select class="md-input" bind:value={think}>
                    <option value="">—</option>
                    {#each info.thinkOptions as v (v)}<option value={v}>{v}</option>{/each}
                </select>
            </label>
            {#if showKeepAlive}
                <label class="md-field">
                    <span class="md-label">{t('settings.modelProfile.keepAlive')}</span>
                    <input class="md-input" type="text" bind:value={keepAlive} placeholder="5m" />
                    {#if provider === 'base'}<p class="md-desc">{t('settings.modelProfile.keepAlive.baseHint')}</p>{/if}
                </label>
            {/if}
            <label class="md-field">
                <span class="md-label">{t('settings.modelProfile.options')}</span>
                <textarea class="md-input md-textarea" rows={4} bind:value={optionsText}></textarea>
                <p class="md-desc">{t(info.optionsHint)}</p>
                {#if optionsError}<p class="md-desc md-desc--err">{optionsError}</p>{/if}
            </label>
        </div>

        <div class="dlg-footer">
            <button class="md-btn" onclick={onCancel}>{t('common.cancel')}</button>
            <button class="md-btn md-btn--primary" onclick={save}>{t('common.save')}</button>
        </div>
    </div>
</div>

<style lang="scss">
    @use 'styles/mixins' as *;

    .overlay {
        position: fixed;
        inset: 0;
        background: var(--overlay-bg);
        backdrop-filter: blur(3px);
        @include flex(row, center, center);
        z-index: 550; // above SettingsDialog (500), below the progress overlay (600)
    }

    .dialog {
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        width: 560px;
        max-width: calc(100vw - 48px);
        max-height: 86vh;
        @include flex(column, flex-start, stretch);
        box-shadow: 0 12px 40px var(--shadow-heavy);
        overflow: hidden;
    }

    .dlg-header {
        padding: 14px 18px;
        border-bottom: 1px solid $border;
    }

    .dlg-title {
        font-size: $fs-regular;
        font-weight: 600;
        color: $text;
    }

    .dlg-body {
        @include flex(column, flex-start, stretch);
        gap: 12px;
        padding: 16px 18px;
        overflow-y: auto;
        @include scrollbar;
    }

    .md-field {
        @include flex(column, flex-start, stretch);
        gap: 5px;
    }

    .md-label {
        font-size: $fs-small;
        font-weight: 500;
        color: $text;
    }

    .md-input {
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

    .md-textarea {
        resize: vertical;
        font-family: monospace;
        font-size: $fs-footnote1;
    }

    .md-desc {
        font-size: $fs-footnote1;
        color: $text-muted;
        line-height: 1.5;
        margin: 0;

        &--err {color: $required-color;}
    }

    .dlg-footer {
        @include flex(row, flex-end, center);
        gap: 8px;
        padding: 10px 18px;
        border-top: 1px solid $border;
        background: $bg-surface;
    }

    .md-btn {
        @include btn-reset;
        @include transition(background, color, border-color);
        padding: 5px 14px;
        white-space: nowrap;
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

        &--primary {
            border-color: $accent;
            color: $text;
        }
    }
</style>
