<script lang="ts">
    import {onMount} from 'svelte';
    import {settings} from './index';
    import {t} from '$lib/i18n';
    import {ollamaStatus, listModels, pullModel, installOllama, cancelOllama, type OllamaModel, type OllamaStatus} from '$lib/ollama/ollama';
    import {OFFERED_MODELS} from '$lib/ollama/models';
    import {progress} from '$lib/progress.svelte';
    import {ollama} from '$lib/ollama/availability.svelte';
    import type {SettingSectionProps} from './SettingsSection';

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    let {section, resetSection}: SettingSectionProps = $props();

    let status = $state<OllamaStatus | null>(null);
    let checking = $state(false);
    let installing = $state(false);
    let installError = $state<string | null>(null);
    let installed = $state<OllamaModel[]>([]);
    let toDownload = $state(OFFERED_MODELS[0]?.id ?? '');

    const baseUrl = $derived(settings.subscribe<string>('ollama.baseUrl')());
    const activeModel = $derived(settings.subscribe<string>('ollama.activeModel')());
    const responseFormat = $derived(settings.subscribe<string>('ollama.responseFormat')());

    async function refreshModels() {
        try {installed = await listModels();}
        catch {installed = [];}
    }

    async function check() {
        checking = true;
        try {
            status = await ollamaStatus();
            await ollama.refresh();
            if (status.reachable) await refreshModels();
        } catch {
            status = {installed: false, reachable: false, version: null};
        } finally {
            checking = false;
        }
    }

    async function install() {
        installing = true;
        installError = null;
        try {
            await installOllama();
            await check();
        } catch (e) {
            installError = e instanceof Error ? e.message : String(e);
        } finally {
            installing = false;
        }
    }

    async function download() {
        if (!toDownload) return;
        const handle = progress.run({
            label: t('settings.ollama.download.progress', {model: toDownload}),
            total: 100,
            blocking: true,
            cancelable: true,
            onCancel: () => {cancelOllama();}
        });
        try {
            await pullModel(toDownload, (p) => {
                if (p.total && p.completed != null) {
                    handle.update({value: Math.round((Number(p.completed) / Number(p.total)) * 100), total: 100, label: p.status});
                } else {
                    handle.update({label: p.status});
                }
            });
            await refreshModels();
        } catch (e) {
            console.error('ollama pull failed:', e);
        } finally {
            handle.done();
        }
    }

    onMount(check);
</script>

<div class="ollama-page">
    <div class="ob-field">
        <span class="ob-label">{t('settings.ollama.status')}</span>
        <div class="ob-row">
            <span class="ob-status" class:ok={status?.reachable} class:bad={status && !status.installed}>
                {#if checking}{t('settings.ollama.checking')}
                {:else if status?.reachable}{t('settings.ollama.status.reachable', {version: status.version ?? ''})}
                {:else if status?.installed}{t('settings.ollama.status.installedNotRunning')}
                {:else if status}{t('settings.ollama.status.notInstalled')}
                {:else}{t('settings.ollama.status.unknown')}{/if}
            </span>
            <button class="ob-btn" onclick={check} disabled={checking}>{t('settings.ollama.check')}</button>
            {#if status && !status.installed}
                <button class="ob-btn" onclick={install} disabled={installing}>
                    {installing ? t('settings.ollama.installing') : t('settings.ollama.install')}
                </button>
            {/if}
        </div>
        {#if installError}<p class="ob-desc ob-desc--err">{t('settings.ollama.installFailed', {error: installError})}</p>{/if}
    </div>

    <div class="ob-field">
        <span class="ob-label">{t('settings.ollama.baseUrl')}</span>
        <input class="ob-input" type="text" value={baseUrl} onchange={(e) => settings.set('ollama.baseUrl', e.currentTarget.value)} />
    </div>

    <div class="ob-field">
        <span class="ob-label">{t('settings.ollama.activeModel')}</span>
        <div class="ob-row">
            <select class="ob-input" value={activeModel} onchange={(e) => settings.set('ollama.activeModel', e.currentTarget.value)}>
                <option value="">{t('settings.ollama.activeModel.none')}</option>
                {#each installed as m (m.name)}<option value={m.name}>{m.name}</option>{/each}
            </select>
            <button class="ob-btn" onclick={refreshModels}>{t('settings.ollama.refresh')}</button>
        </div>
        {#if installed.length === 0}<p class="ob-desc">{t('settings.ollama.noModels')}</p>{/if}
    </div>

    <div class="ob-field">
        <span class="ob-label">{t('settings.ollama.download')}</span>
        <div class="ob-row">
            <select class="ob-input" bind:value={toDownload}>
                {#each OFFERED_MODELS as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
            </select>
            <button class="ob-btn" onclick={download} disabled={!status?.installed}>{t('settings.ollama.download.button')}</button>
        </div>
    </div>

    <div class="ob-field">
        <span class="ob-label">{t('settings.ollama.responseFormat')}</span>
        <textarea class="ob-input ob-textarea" rows={8} value={responseFormat} onchange={(e) => settings.set('ollama.responseFormat', e.currentTarget.value)}></textarea>
        <p class="ob-desc">{t('settings.ollama.responseFormat.description')}</p>
    </div>
</div>

<style lang="scss">
    @use 'styles/mixins' as *;

    .ollama-page {
        @include flex(column, flex-start, stretch);
        gap: 0;
    }

    .ob-field {
        @include flex(column, flex-start, stretch);
        gap: 6px;
        padding: 14px 0;
        border-bottom: 1px solid $border;

        &:last-child {border-bottom: none;}
    }

    .ob-label {
        font-size: $fs-small;
        font-weight: 500;
        color: $text;
    }

    .ob-row {
        @include flex(row, flex-start, center);
        gap: 8px;
        flex-wrap: wrap;
    }

    .ob-status {
        font-size: $fs-small;
        color: $text-muted;

        &.ok {color: #4ade80;}
        &.bad {color: $required-color;}
    }

    .ob-input {
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

    .ob-textarea {
        width: 100%;
        resize: vertical;
        font-family: monospace;
        font-size: $fs-footnote1;
    }

    .ob-btn {
        @include btn-reset;
        @include transition(background, color, border-color);
        padding: 5px 12px;
        border: 1px solid $border;
        border-radius: $radius-sm;
        font-size: $fs-small;
        font-family: $font-base;
        color: $text-secondary;
        background: transparent;
        cursor: pointer;

        &:hover:not(:disabled) {
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }

        &:disabled {opacity: 0.4; cursor: not-allowed;}
    }

    .ob-desc {
        font-size: $fs-footnote1;
        color: $text-muted;
        line-height: 1.5;
        margin: 0;

        &--err {color: $required-color;}
    }
</style>
