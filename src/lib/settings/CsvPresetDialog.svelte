<script lang="ts">
    import {t} from '$lib/i18n';
    import type {MessageKey} from '$lib/i18n';
    import {
        VALUE_TYPES, DELIMITERS, BOOL_FORMATS, isBoolType, isNoneType, isValidIdentifier, createEmptyField,
        type CsvPreset, type CsvField, type AppValueType, type Delimiter, type BoolFormat,
    } from '$lib/csv/csv';

    let {preset, isNew, takenIdentifiers = [], closeOnBackdrop = false, onSave, onCancel}: {
        preset: CsvPreset;
        isNew: boolean;
        takenIdentifiers?: string[]; // identifiers used by OTHER presets — disallow duplicates
        closeOnBackdrop?: boolean;
        onSave: (p: CsvPreset) => void;
        onCancel: () => void;
    } = $props();

    // Typed key maps so dynamic labels stay compile-time checked (t expects a MessageKey).
    const VALUE_TYPE_LABEL: Record<AppValueType, MessageKey> = {
        none: 'settings.csv.valueType.none',
        fileName: 'settings.csv.valueType.fileName',
        title: 'settings.csv.valueType.title',
        description: 'settings.csv.valueType.description',
        keywords: 'settings.csv.valueType.keywords',
        category: 'settings.csv.valueType.category',
        editorial: 'settings.csv.valueType.editorial',
        matureContent: 'settings.csv.valueType.matureContent',
        illustration: 'settings.csv.valueType.illustration',
    };
    const DELIMITER_LABEL: Record<Delimiter, MessageKey> = {
        comma: 'settings.csv.delimiter.comma',
        semicolon: 'settings.csv.delimiter.semicolon',
        tab: 'settings.csv.delimiter.tab',
    };
    const BOOL_FORMAT_LABEL: Record<BoolFormat, MessageKey> = {
        yesNo: 'settings.csv.boolFormat.yesNo',
        trueFalse: 'settings.csv.boolFormat.trueFalse',
    };

    // One-time snapshot from the prop (the dialog is recreated per open via {#if editing}).
    const seed = (() => preset)();
    let name = $state(seed.name);
    let identifier = $state(seed.identifier);
    let delimiter = $state<Delimiter>(seed.delimiter);
    let fields = $state<CsvField[]>(seed.fields.map(f => ({...f})));
    let nameError = $state<string | null>(null);
    let identifierError = $state<string | null>(null);
    let fieldsError = $state<string | null>(null);

    function addField() {
        fields = [...fields, createEmptyField()];
        fieldsError = null;
    }

    function removeField(i: number) {
        fields = fields.filter((_, idx) => idx !== i);
    }

    function moveField(i: number, dir: -1 | 1) {
        const j = i + dir;
        if (j < 0 || j >= fields.length) return;
        const next = [...fields];
        [next[i], next[j]] = [next[j], next[i]];
        fields = next;
    }

    function save() {
        const nm = name.trim();
        const id = identifier.trim();
        if (!nm) {
            nameError = t('settings.csv.nameRequired');
            return;
        }
        nameError = null;
        if (!isValidIdentifier(id)) {
            identifierError = t('settings.csv.invalidIdentifier');
            return;
        }
        if (takenIdentifiers.some(x => x.toLowerCase() === id.toLowerCase())) {
            identifierError = t('settings.csv.duplicateIdentifier');
            return;
        }
        identifierError = null;
        if (fields.length === 0) {
            fieldsError = t('settings.csv.fieldsRequired');
            return;
        }
        fieldsError = null;
        onSave({...preset, name: nm, identifier: id, delimiter, fields: fields.map(f => ({...f}))});
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
            <span class="dlg-title">{t(isNew ? 'settings.csv.createTitle' : 'settings.csv.editTitle')}</span>
        </div>

        <div class="dlg-body">
            <label class="md-field">
                <span class="md-label">{t('settings.csv.name')}</span>
                <input class="md-input" type="text" bind:value={name} oninput={() => (nameError = null)} />
                {#if nameError}<p class="md-desc md-desc--err">{nameError}</p>{/if}
            </label>

            <label class="md-field">
                <span class="md-label">{t('settings.csv.identifier')}</span>
                <input class="md-input" type="text" bind:value={identifier} oninput={() => (identifierError = null)} />
                <p class="md-desc">{t('settings.csv.identifier.hint')}</p>
                {#if identifierError}<p class="md-desc md-desc--err">{identifierError}</p>{/if}
            </label>

            <label class="md-field">
                <span class="md-label">{t('settings.csv.delimiter')}</span>
                <select class="md-input" bind:value={delimiter}>
                    {#each DELIMITERS as d}
                        <option value={d}>{t(DELIMITER_LABEL[d])}</option>
                    {/each}
                </select>
            </label>

            <div class="md-field">
                <div class="cf-head">
                    <span class="md-label">{t('settings.csv.fields')}</span>
                    <button class="md-btn" onclick={addField}>{t('settings.csv.addField')}</button>
                </div>
                {#if fieldsError}<p class="md-desc md-desc--err">{fieldsError}</p>{/if}
                <ul class="cf-list">
                    {#each fields as field, i (i)}
                        <li class="cf-row">
                            <input
                                class="md-input cf-col"
                                type="text"
                                placeholder={t('settings.csv.field.column.placeholder')}
                                bind:value={field.csvColumn}
                            />
                            <select class="md-input cf-type" bind:value={field.valueType}>
                                {#each VALUE_TYPES as vt}
                                    <option value={vt}>{t(VALUE_TYPE_LABEL[vt])}</option>
                                {/each}
                            </select>
                            {#if isNoneType(field.valueType)}
                                <input
                                    class="md-input cf-extra"
                                    type="text"
                                    placeholder={t('settings.csv.field.defaultValue')}
                                    bind:value={field.defaultValue}
                                />
                            {:else if isBoolType(field.valueType)}
                                <select class="md-input cf-extra" bind:value={field.boolFormat}>
                                    {#each BOOL_FORMATS as bf}
                                        <option value={bf}>{t(BOOL_FORMAT_LABEL[bf])}</option>
                                    {/each}
                                </select>
                            {:else}
                                <span class="cf-extra cf-extra--empty"></span>
                            {/if}
                            <button class="cf-icon" title={t('settings.csv.field.moveUp')} onclick={() => moveField(i, -1)} disabled={i === 0}>↑</button>
                            <button class="cf-icon" title={t('settings.csv.field.moveDown')} onclick={() => moveField(i, 1)} disabled={i === fields.length - 1}>↓</button>
                            <button class="cf-icon cf-icon--danger" title={t('settings.csv.field.remove')} onclick={() => removeField(i)}>✕</button>
                        </li>
                    {/each}
                </ul>
            </div>
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
        width: 640px;
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

    .md-desc {
        font-size: $fs-footnote1;
        color: $text-muted;
        line-height: 1.5;
        margin: 0;

        &--err {color: $required-color;}
    }

    .cf-head {
        @include flex(row, space-between, center);
    }

    .cf-list {
        @include flex(column, flex-start, stretch);
        gap: 6px;
        list-style: none;
        margin: 6px 0 0;
        padding: 0;
    }

    .cf-row {
        @include flex(row, flex-start, center);
        gap: 6px;
    }

    .cf-col {
        flex: 1 1 0;
        min-width: 0;
    }

    .cf-type {
        flex: 0 0 130px;
    }

    .cf-extra {
        flex: 1 1 0;
        min-width: 0;

        &--empty {
            visibility: hidden;
        }
    }

    .cf-icon {
        @include btn-reset;
        @include transition(background, color, border-color);
        flex-shrink: 0;
        width: 28px;
        height: 28px;
        border: 1px solid $border;
        border-radius: $radius-sm;
        font-size: $fs-small;
        color: $text-secondary;
        background: transparent;
        cursor: pointer;

        &:hover:not(:disabled) {
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }

        &:disabled {
            opacity: 0.35;
            cursor: default;
        }

        &--danger:hover:not(:disabled) {
            background: var(--required-alpha-08);
            color: var(--required-color);
            border-color: var(--required-color);
        }
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
