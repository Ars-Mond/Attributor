<script lang="ts">
    import {t} from '$lib/i18n';
    import type {ModelProfile} from '$lib/ollama/ollama';

    let {profile, isNew, modelOptions, onSave, onCancel}: {
        profile: ModelProfile;
        isNew: boolean;
        modelOptions: string[];
        onSave: (p: ModelProfile) => void;
        onCancel: () => void;
    } = $props();

    // Seed local editable state once from the prop (the dialog is recreated per open via {#if editing}).
    // Read inside a closure so it's a deliberate one-time snapshot, not a reactive dependency.
    const seed = (() => profile)();
    let name = $state(seed.name);
    let modelId = $state(seed.modelId);
    let prompt = $state(seed.prompt);
    let think = $state(seed.think == null ? '' : String(seed.think));
    let keepAlive = $state(seed.keepAlive ?? '');
    let optionsText = $state(JSON.stringify(seed.options ?? {}, null, 2));
    let optionsError = $state<string | null>(null);

    function parseThink(v: string): boolean | string | null {
        if (v === '') return null;
        if (v === 'true') return true;
        if (v === 'false') return false;
        return v;
    }

    function save() {
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
            modelId: modelId.trim(),
            prompt,
            think: parseThink(think),
            keepAlive: keepAlive.trim() || null,
            options
        });
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') onCancel();
    }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="overlay" role="presentation" onclick={onCancel} onkeydown={() => {}}>
    <div class="dialog" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
        <div class="dlg-header">
            <span class="dlg-title">{t(isNew ? 'settings.ollamaModel.createTitle' : 'settings.ollamaModel.editTitle')}</span>
        </div>

        <div class="dlg-body">
            <label class="md-field">
                <span class="md-label">{t('settings.ollamaModel.name')}</span>
                <input class="md-input" type="text" bind:value={name} />
            </label>
            <label class="md-field">
                <span class="md-label">{t('settings.ollamaModel.modelId')}</span>
                <input class="md-input" type="text" list="ollama-model-options" bind:value={modelId} placeholder="llama3.2-vision:11b" />
                <datalist id="ollama-model-options">
                    {#each modelOptions as o (o)}<option value={o}></option>{/each}
                </datalist>
            </label>
            <label class="md-field">
                <span class="md-label">{t('settings.ollamaModel.prompt')}</span>
                <textarea class="md-input md-textarea" rows={5} bind:value={prompt}></textarea>
            </label>
            <label class="md-field">
                <span class="md-label">{t('settings.ollamaModel.think')}</span>
                <select class="md-input" bind:value={think}>
                    <option value="">—</option>
                    <option value="false">false</option>
                    <option value="true">true</option>
                    <option value="low">low</option>
                    <option value="medium">medium</option>
                    <option value="high">high</option>
                    <option value="max">max</option>
                </select>
            </label>
            <label class="md-field">
                <span class="md-label">{t('settings.ollamaModel.keepAlive')}</span>
                <input class="md-input" type="text" bind:value={keepAlive} placeholder="5m" />
            </label>
            <label class="md-field">
                <span class="md-label">{t('settings.ollamaModel.options')}</span>
                <textarea class="md-input md-textarea" rows={4} bind:value={optionsText}></textarea>
                <p class="md-desc">{t('settings.ollamaModel.options.description')}</p>
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
