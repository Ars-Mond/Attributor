<script lang="ts">
    import {onMount} from 'svelte';
    import {openUrl} from '@tauri-apps/plugin-opener';
    import {settings} from './index';
    import {t} from '$lib/i18n';
    import type {MessageKey} from '$lib/i18n/types';
    import {pullModel, installOllama, cancelAi, setApiKey, chatgptLogin, chatgptLogout} from '$lib/ai/ai';
    import {OFFERED_MODELS} from '$lib/ai/models';
    import {PROVIDERS, activeModelKey, baseUrlKey, type ProviderId} from '$lib/ai/providers';
    import {progress} from '$lib/progress.svelte';
    import {ai} from '$lib/ai/availability.svelte';
    import ModelCombobox from '$lib/ai/ModelCombobox.svelte';
    import type {SettingSectionProps} from './SettingsSection';

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    let {section, resetSection}: SettingSectionProps = $props();

    // Where ChatGPT users see and cap the plan usage of connected apps.
    const CHATGPT_USAGE_URL = 'https://chatgpt.com/settings/usage';

    let installing = $state(false);
    let installError = $state<string | null>(null);
    let toDownload = $state(OFFERED_MODELS[0]?.id ?? '');
    let keyDraft = $state('');
    let keyError = $state<string | null>(null);
    let accountError = $state<string | null>(null);

    const provider = $derived(ai.provider);
    const info = $derived(ai.info);
    const urlKey = $derived(baseUrlKey(provider));
    const baseUrl = $derived(urlKey ? settings.subscribe<string>(urlKey)() : '');
    const activeModel = $derived(settings.subscribe<string>(activeModelKey(provider))());
    const responseFormat = $derived(settings.subscribe<string>('ai.responseFormat')());
    const answerFormatLabel = $derived(settings.subscribe<string>('ai.answerFormatLabel')());
    const usesKey = $derived(info.auth === 'key' || info.auth === 'optionalKey');
    const hasKey = $derived(ai.hasKey(provider));

    function message(e: unknown): string {
        return e instanceof Error ? e.message : String(e);
    }

    // Status line: wording depends on what the provider can report (installed/running, credentials, reachability).
    // `detail` is the backend's reason for a failed check, shown under the status line.
    const statusView = $derived.by((): {text: string; ok: boolean; bad: boolean; detail: string | null} => {
        const s = ai.status;
        const view = (key: MessageKey, ok: boolean, bad: boolean, detail: string | null = null) => ({text: t(key), ok, bad, detail});
        if (ai.checking) return view('settings.ai.checking', false, false);
        if (info.auth === 'key' && !hasKey) return view('settings.ai.status.noKey', false, true);
        if (info.auth === 'oauth' && !ai.account) return view('settings.ai.status.signedOut', false, true);
        if (!s) return view('settings.ai.status.unknown', false, false);
        if (s.reachable) {
            if (info.kind === 'cloud') return view('settings.ai.status.connected', true, false);
            if (s.version) return {text: t('settings.ai.status.reachable', {version: s.version}), ok: true, bad: false, detail: null};
            return view('settings.ai.status.running', true, false);
        }
        if (s.installed) return view('settings.ai.status.installedNotRunning', false, false);
        if (provider === 'ollama') return view('settings.ai.status.notInstalled', false, true);
        if (info.kind === 'cloud') return view('settings.ai.status.failed', false, true, s.detail);
        return view('settings.ai.status.notReachable', false, true, s.detail);
    });

    async function selectProvider(id: string) {
        settings.set('ai.provider', id);
        keyDraft = '';
        keyError = null;
        accountError = null;
        installError = null;
        await ai.init(id as ProviderId);
    }

    // "Check" updates status (and refreshes the model list if it is now reachable).
    async function check() {
        await ai.refresh();
        if (ai.reachable) await ai.refreshModels();
    }

    async function saveKey() {
        keyError = null;
        try {
            await setApiKey(provider, keyDraft);
            keyDraft = '';
            await ai.refreshKey();
            await check();
        } catch (e) {
            keyError = t('settings.ai.apiKey.failed', {error: message(e)});
        }
    }

    async function removeKey() {
        keyError = null;
        try {
            await setApiKey(provider, '');
            await ai.refreshKey();
            await ai.refresh();
        } catch (e) {
            keyError = t('settings.ai.apiKey.failed', {error: message(e)});
        }
    }

    // Sign-in runs in the browser; the blocking overlay's Cancel aborts the wait for the redirect.
    async function signIn(newAccount = false) {
        accountError = null;
        const handle = progress.run({
            label: t('settings.ai.account.signingIn'),
            blocking: true,
            cancelable: true,
            onCancel: () => {cancelAi();}
        });
        try {
            await chatgptLogin(newAccount);
            await ai.refreshAccount();
            await check();
        } catch (e) {
            const msg = message(e);
            if (msg !== 'cancelled') accountError = t('settings.ai.account.failed', {error: msg});
        } finally {
            handle.done();
        }
    }

    async function signOut() {
        accountError = null;
        try {
            await chatgptLogout();
        } catch (e) {
            accountError = message(e);
        }
        await ai.refreshAccount();
        await ai.refresh();
    }

    async function install() {
        installing = true;
        installError = null;
        try {
            await installOllama();
            await ai.init('ollama');
        } catch (e) {
            installError = message(e);
        } finally {
            installing = false;
        }
    }

    async function download() {
        if (!toDownload) return;
        const handle = progress.run({
            label: t('settings.ai.download.progress', {model: toDownload}),
            total: 100,
            blocking: true,
            cancelable: true,
            onCancel: () => {cancelAi();}
        });
        try {
            await pullModel(toDownload, (p) => {
                if (p.total && p.completed != null) {
                    handle.update({value: Math.round((Number(p.completed) / Number(p.total)) * 100), total: 100, label: p.status});
                } else {
                    handle.update({label: p.status});
                }
            });
            await ai.refreshModels('ollama');
        } catch (e) {
            console.error('ollama pull failed:', e);
        } finally {
            handle.done();
        }
    }

    // Data is prefetched at app startup; only fetch here if that has not happened yet (no wait normally).
    onMount(() => {if (!ai.status) ai.init();});
</script>

<div class="ai-page">
    <div class="ob-field">
        <span class="ob-label">{t('settings.ai.provider')}</span>
        <select class="ob-input" value={provider} onchange={(e) => selectProvider(e.currentTarget.value)}>
            {#each PROVIDERS as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
        </select>
        <p class="ob-desc">{t(`settings.ai.provider.description.${info.kind}`)}</p>
    </div>

    <div class="ob-field">
        <span class="ob-label">{t('settings.ai.status')}</span>
        <div class="ob-row">
            <span class="ob-status" class:ok={statusView.ok} class:bad={statusView.bad}>{statusView.text}</span>
            <button class="ob-btn" onclick={check} disabled={ai.checking}>{t('settings.ai.check')}</button>
            {#if provider === 'ollama' && ai.status && !ai.installed}
                <button class="ob-btn" onclick={install} disabled={installing}>
                    {installing ? t('settings.ai.installing') : t('settings.ai.install')}
                </button>
            {/if}
        </div>
        {#if statusView.detail}<p class="ob-desc">{statusView.detail}</p>{/if}
        {#if installError}<p class="ob-desc ob-desc--err">{t('settings.ai.installFailed', {error: installError})}</p>{/if}
    </div>

    {#if urlKey}
        <div class="ob-field">
            <span class="ob-label">{t('settings.ai.baseUrl')}</span>
            <input class="ob-input" type="text" value={baseUrl} onchange={(e) => settings.set(urlKey, e.currentTarget.value.trim())} />
            {#if info.kind === 'local'}<p class="ob-desc">{t('settings.ai.baseUrl.description')}</p>{/if}
            {#if info.baseUrlHint}<p class="ob-desc">{t(info.baseUrlHint)}</p>{/if}
        </div>
    {/if}

    {#if usesKey}
        <div class="ob-field">
            <span class="ob-label">{t(info.auth === 'key' ? 'settings.ai.apiKey' : 'settings.ai.apiKey.optional')}</span>
            {#if hasKey}
                <div class="ob-row">
                    <span class="ob-status ok">{t('settings.ai.apiKey.saved')}</span>
                    <button class="ob-btn" onclick={removeKey}>{t('settings.ai.apiKey.remove')}</button>
                </div>
            {/if}
            <div class="ob-row">
                <input
                    class="ob-input ob-grow"
                    type="password"
                    autocomplete="off"
                    placeholder={t(hasKey ? 'settings.ai.apiKey.replace' : 'settings.ai.apiKey.placeholder')}
                    bind:value={keyDraft}
                    onkeydown={(e) => {if (e.key === 'Enter' && keyDraft.trim()) saveKey();}}
                />
                <button class="ob-btn" onclick={saveKey} disabled={!keyDraft.trim()}>{t('common.save')}</button>
                {#if info.keyUrl}
                    <button class="ob-btn" onclick={() => openUrl(info.keyUrl!)}>{t('settings.ai.apiKey.get')}</button>
                {/if}
            </div>
            {#if info.keyHint}<p class="ob-desc">{t(info.keyHint)}</p>{/if}
            <p class="ob-desc">{t('settings.ai.apiKey.description')}</p>
            {#if keyError}<p class="ob-desc ob-desc--err">{keyError}</p>{/if}
        </div>
    {/if}

    {#if info.auth === 'oauth'}
        <div class="ob-field">
            <span class="ob-label">{t('settings.ai.account')}</span>
            <div class="ob-row">
                {#if ai.account}
                    <span class="ob-status ok">{t('settings.ai.account.signedIn', {email: ai.account.email ?? '—'})}</span>
                    <button class="ob-btn" onclick={() => openUrl(CHATGPT_USAGE_URL)}>{t('settings.ai.account.manageUsage')}</button>
                    <button class="ob-btn" onclick={signOut}>{t('settings.ai.account.signOut')}</button>
                {:else}
                    <button class="ob-btn" onclick={() => signIn()}>{t('settings.ai.account.signIn')}</button>
                    <button class="ob-btn" onclick={() => signIn(true)}>{t('settings.ai.account.otherAccount')}</button>
                {/if}
            </div>
            <p class="ob-desc">{t('settings.ai.account.description')}</p>
            {#if accountError}<p class="ob-desc ob-desc--err">{accountError}</p>{/if}
        </div>
    {/if}

    <div class="ob-field">
        <span class="ob-label">{t('settings.ai.activeModel')}</span>
        <div class="ob-row">
            <div class="ob-grow">
                {#key provider}
                    <ModelCombobox
                        value={activeModel}
                        known={info.known}
                        installed={ai.installedModels}
                        installedLabel={provider === 'ollama' ? 'ai.model.installed' : 'ai.model.available'}
                        placeholder={t('settings.ai.activeModel.none')}
                        onChange={(v) => settings.set(activeModelKey(provider), v.trim())}
                    />
                {/key}
            </div>
            <button class="ob-btn" onclick={() => ai.refreshModels()}>{t('settings.ai.refresh')}</button>
        </div>
        {#if ai.installedModels.length === 0}
            <p class="ob-desc">{t(provider === 'ollama' ? 'settings.ai.noModels.ollama' : 'settings.ai.noModels.other')}</p>
        {/if}
    </div>

    {#if provider === 'ollama'}
        <div class="ob-field">
            <span class="ob-label">{t('settings.ai.download')}</span>
            <div class="ob-row">
                <select class="ob-input" bind:value={toDownload}>
                    {#each OFFERED_MODELS as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
                </select>
                <button class="ob-btn" onclick={download} disabled={!ai.installed}>{t('settings.ai.download.button')}</button>
            </div>
        </div>
    {/if}

    <div class="ob-field">
        <span class="ob-label">{t('settings.ai.answerFormatLabel')}</span>
        <input class="ob-input" type="text" value={answerFormatLabel} onchange={(e) => settings.set('ai.answerFormatLabel', e.currentTarget.value)} />
        <p class="ob-desc">{t('settings.ai.answerFormatLabel.description')}</p>
    </div>

    <div class="ob-field">
        <span class="ob-label">{t('settings.ai.responseFormat')}</span>
        <textarea class="ob-input ob-textarea" rows={8} value={responseFormat} onchange={(e) => settings.set('ai.responseFormat', e.currentTarget.value)}></textarea>
        <p class="ob-desc">{t('settings.ai.responseFormat.description')}</p>
    </div>
</div>

<style lang="scss">
    @use 'styles/mixins' as *;

    .ai-page {
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

    .ob-grow {
        flex: 1;
        min-width: 0;
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
        white-space: nowrap;
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
        overflow-wrap: anywhere;

        &--err {color: $required-color;}
    }
</style>
