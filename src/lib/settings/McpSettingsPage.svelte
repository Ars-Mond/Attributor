<script lang="ts">
    import {onMount} from 'svelte';
    import {writeText} from '@tauri-apps/plugin-clipboard-manager';
    import {revealItemInDir} from '@tauri-apps/plugin-opener';
    import {t, locale} from '$lib/i18n';
    import type {MessageKey} from '$lib/i18n/types';
    import {mcp, type McpActivity, type McpClient, type McpClientStatus, type McpConfigChange, type McpView} from '$lib/mcp/mcp';
    import type {SettingSectionProps} from './SettingsSection';

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    let {section, resetSection}: SettingSectionProps = $props();

    // How often the open page re-reads the server state and the activity log.
    const POLL_MS = 2000;
    const ACTIVITY_SHOWN = 50;

    type ClientState = 'unknown' | 'notFound' | 'found' | 'installed' | 'outdated' | 'error';

    const CLIENTS: {id: McpClient; label: string; hint: MessageKey}[] = [
        {id: 'claude-desktop', label: 'Claude Desktop', hint: 'settings.mcp.client.hint.claudeDesktop'},
        {id: 'claude-code', label: 'Claude Code', hint: 'settings.mcp.client.hint.claudeCode'},
        {id: 'codex', label: 'ChatGPT / Codex', hint: 'settings.mcp.client.hint.codex'}
    ];

    let view = $state<McpView | null>(null);
    let activity = $state<McpActivity[]>([]);
    let clients = $state<Partial<Record<McpClient, McpClientStatus>>>({});
    let busy = $state<Partial<Record<McpClient, boolean>>>({});
    let clientErrors = $state<Partial<Record<McpClient, string | null>>>({});
    let copied = $state<string | null>(null);
    // `error` reports the user's last action; `pollError` the background refresh, which clears itself once a refresh works again.
    let error = $state<string | null>(null);
    let pollError = $state<string | null>(null);
    let port = $state<number | null>(null);
    let showToken = $state(false);
    // A settings change in flight: its controls are disabled so one change is never sent twice.
    let changing = $state(false);

    function message(e: unknown): string {
        return e instanceof Error ? e.message : String(e);
    }

    const statusView = $derived.by((): {text: string; ok: boolean; bad: boolean} => {
        if (!view) return {text: '', ok: false, bad: false};
        if (view.running) return {text: t('settings.mcp.status.running', {url: view.url}), ok: true, bad: false};
        if (view.error) return {text: t('settings.mcp.status.error', {error: view.error}), ok: false, bad: true};
        return {text: t('settings.mcp.status.stopped'), ok: false, bad: false};
    });

    function clientState(s: McpClientStatus | undefined): ClientState {
        if (!s) return 'unknown';
        if (s.error) return 'error';
        if (s.configured) return s.upToDate ? 'installed' : 'outdated';
        return s.detected ? 'found' : 'notFound';
    }

    async function poll() {
        try {
            view = await mcp.getConfig();
            activity = (await mcp.activity()).reverse();
            pollError = null;
        } catch (e) {
            pollError = message(e);
        }
    }

    async function loadClients() {
        try {
            for (const s of await mcp.clients()) clients[s.client] = s;
        } catch (e) {
            error = message(e);
        }
    }

    onMount(async () => {
        await poll();
        port = view?.port ?? null;
        await loadClients();
    });

    $effect(() => {
        const timer = setInterval(poll, POLL_MS);
        return () => clearInterval(timer);
    });

    async function change(patch: McpConfigChange) {
        if (changing) return;
        changing = true;
        error = null;
        try {
            view = await mcp.setConfig(patch);
            port = view.port;
            await loadClients();
        } catch (e) {
            error = message(e);
        } finally {
            changing = false;
        }
    }

    function applyPort() {
        if (port === null || !Number.isInteger(port) || port < 1024 || port > 65535) {
            error = t('settings.mcp.port.invalid');
            return;
        }
        if (port !== view?.port) void change({port});
    }

    async function regenerate() {
        if (changing) return;
        changing = true;
        error = null;
        try {
            view = await mcp.regenerateToken();
            await loadClients();
        } catch (e) {
            error = message(e);
        } finally {
            changing = false;
        }
    }

    async function copy(text: string, what: string) {
        try {
            await writeText(text);
            copied = what;
            setTimeout(() => {if (copied === what) copied = null;}, 1500);
        } catch (e) {
            error = message(e);
        }
    }

    async function runClient(id: McpClient, action: 'install' | 'uninstall') {
        busy[id] = true;
        clientErrors[id] = null;
        try {
            clients[id] = await mcp[action](id);
        } catch (e) {
            clientErrors[id] = message(e);
        } finally {
            busy[id] = false;
        }
    }

    async function copySnippet(id: McpClient) {
        try {
            await copy(await mcp.snippet(id), id);
        } catch (e) {
            clientErrors[id] = message(e);
        }
    }

    async function reveal(id: McpClient) {
        const file = clients[id]?.files[0];
        if (!file) return;
        try {
            await revealItemInDir(file);
        } catch (e) {
            clientErrors[id] = message(e);
        }
    }

    async function clearActivity() {
        await mcp.clearActivity().catch((e: unknown) => {error = message(e);});
        activity = [];
    }

    function time(at: number): string {
        return new Date(at).toLocaleTimeString(locale());
    }
</script>

<div class="mcp-page">
    <p class="ob-desc ob-intro">{t('settings.mcp.intro')}</p>

    {#if view}
        <div class="ob-field">
            <span class="ob-label">{t('settings.mcp.server')}</span>
            <label class="ob-check">
                <input type="checkbox" checked={view.enabled} disabled={changing} onchange={(e) => change({enabled: e.currentTarget.checked})} />
                {t('settings.mcp.enable')}
            </label>
            <span class="ob-status" class:ok={statusView.ok} class:bad={statusView.bad}>{statusView.text}</span>
            <p class="ob-desc">{t('settings.mcp.appOpen')}</p>
        </div>

        <div class="ob-field">
            <span class="ob-label">{t('settings.mcp.address')}</span>
            <div class="ob-row">
                <input
                    class="ob-input ob-port"
                    type="number"
                    min="1024"
                    max="65535"
                    disabled={changing}
                    bind:value={port}
                    onchange={applyPort}
                />
                <code class="ob-code">{view.url}</code>
                <button class="ob-btn" onclick={() => copy(view!.url, 'url')}>{copied === 'url' ? t('settings.mcp.copied') : t('common.copy')}</button>
            </div>
            <p class="ob-desc">{t('settings.mcp.port.description')}</p>
        </div>

        <div class="ob-field">
            <span class="ob-label">{t('settings.mcp.token')}</span>
            <div class="ob-row">
                <input class="ob-input ob-grow ob-mono" type={showToken ? 'text' : 'password'} readonly value={view.token} />
                <button class="ob-btn" onclick={() => {showToken = !showToken;}}>{t(showToken ? 'settings.mcp.token.hide' : 'settings.mcp.token.show')}</button>
                <button class="ob-btn" onclick={() => copy(view!.token, 'token')}>{copied === 'token' ? t('settings.mcp.copied') : t('common.copy')}</button>
                <button class="ob-btn" disabled={changing} onclick={regenerate}>{t('settings.mcp.token.regenerate')}</button>
            </div>
            <p class="ob-desc">{t('settings.mcp.token.description')}</p>
        </div>

        <div class="ob-field">
            <span class="ob-label">{t('settings.mcp.permissions')}</span>
            <label class="ob-check">
                <input type="checkbox" checked={view.allowAiSettings} disabled={changing} onchange={(e) => change({allowAiSettings: e.currentTarget.checked})} />
                {t('settings.mcp.allowAi')}
            </label>
            <p class="ob-desc">{t('settings.mcp.allowAi.description')}</p>
        </div>

        <div class="ob-field">
            <span class="ob-label">{t('settings.mcp.clients')}</span>
            <p class="ob-desc">{t('settings.mcp.clients.description')}</p>
            {#if !view.enabled}<p class="ob-desc ob-desc--warn">{t('settings.mcp.client.needsServer')}</p>{/if}
            {#each CLIENTS as c (c.id)}
                {@const s = clients[c.id]}
                {@const state = clientState(s)}
                <div class="client">
                    <div class="ob-row">
                        <span class="client-name">{c.label}</span>
                        <span class="ob-status" class:ok={state === 'installed'} class:bad={state === 'error'} class:warn={state === 'outdated'}>
                            {t(`settings.mcp.client.state.${state}`)}
                        </span>
                    </div>
                    <div class="ob-row">
                        <button class="ob-btn" disabled={busy[c.id] || state === 'installed'} onclick={() => runClient(c.id, 'install')}>
                            {t(s?.configured ? 'settings.mcp.client.update' : 'settings.mcp.client.install')}
                        </button>
                        {#if s?.configured}
                            <button class="ob-btn" disabled={busy[c.id]} onclick={() => runClient(c.id, 'uninstall')}>{t('settings.mcp.client.remove')}</button>
                            <button class="ob-btn" onclick={() => reveal(c.id)}>{t('settings.mcp.client.open')}</button>
                        {/if}
                        <button class="ob-btn" onclick={() => copySnippet(c.id)}>{copied === c.id ? t('settings.mcp.copied') : t('settings.mcp.client.copy')}</button>
                    </div>
                    {#each s?.files ?? [] as file (file)}<p class="ob-desc ob-mono">{file}</p>{/each}
                    <p class="ob-desc">{t(c.hint)}</p>
                    {#if s?.error}<p class="ob-desc ob-desc--err">{s.error}</p>{/if}
                    {#if clientErrors[c.id]}<p class="ob-desc ob-desc--err">{clientErrors[c.id]}</p>{/if}
                </div>
            {/each}
        </div>

        <div class="ob-field">
            <div class="ob-row ob-between">
                <span class="ob-label">{t('settings.mcp.activity')}</span>
                {#if activity.length > 0}<button class="ob-btn" onclick={clearActivity}>{t('settings.mcp.activity.clear')}</button>{/if}
            </div>
            {#if activity.length === 0}
                <p class="ob-desc">{t('settings.mcp.activity.empty')}</p>
            {:else}
                <ul class="activity">
                    {#each activity.slice(0, ACTIVITY_SHOWN) as a, i (i)}
                        <li class:failed={!a.ok}>
                            <span class="activity-time">{time(a.at)}</span>
                            <span class="activity-tool">{a.tool}</span>
                            <span class="activity-client">{a.client ?? ''}</span>
                            <span class="activity-ms">{t('settings.mcp.activity.duration', {ms: a.durationMs})}</span>
                            {#if a.message}<span class="activity-msg">{a.message}</span>{/if}
                        </li>
                    {/each}
                </ul>
            {/if}
        </div>
    {/if}

    {#if error}<p class="ob-desc ob-desc--err">{error}</p>{/if}
    {#if pollError}<p class="ob-desc ob-desc--err">{pollError}</p>{/if}
</div>

<style lang="scss">
    @use 'styles/mixins' as *;

    .mcp-page {
        @include flex(column, flex-start, stretch);
        gap: 0;
    }

    .ob-intro {
        padding-bottom: 6px;
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

    .ob-between {
        justify-content: space-between;
    }

    .ob-grow {
        flex: 1;
        min-width: 0;
    }

    .ob-check {
        @include flex(row, flex-start, center);
        gap: 8px;
        font-size: $fs-small;
        color: $text;
        cursor: pointer;

        input {
            width: 14px;
            height: 14px;
            flex-shrink: 0;
            cursor: pointer;
            accent-color: $accent;
        }
    }

    .ob-status {
        font-size: $fs-small;
        color: $text-muted;

        &.ok {color: #4ade80;}
        &.warn {color: #fbbf24;}
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

    .ob-port {
        width: 90px;
    }

    .ob-mono {
        font-family: monospace;
    }

    .ob-code {
        font-family: monospace;
        font-size: $fs-footnote1;
        color: $text-secondary;
        overflow-wrap: anywhere;
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
        &--warn {color: #fbbf24;}
    }

    .client {
        @include flex(column, flex-start, stretch);
        gap: 6px;
        padding: 10px 12px;
        border: 1px solid $border;
        border-radius: $radius-sm;
    }

    .client-name {
        font-size: $fs-small;
        font-weight: 500;
        color: $text;
    }

    .activity {
        list-style: none;
        margin: 0;
        padding: 0;
        max-height: 260px;
        overflow-y: auto;
        font-size: $fs-footnote1;

        li {
            @include flex(row, flex-start, baseline);
            flex-wrap: wrap;
            gap: 8px;
            padding: 3px 0;
            border-bottom: 1px solid $border;
            color: $text-secondary;

            &:last-child {border-bottom: none;}
            &.failed .activity-tool {color: $required-color;}
        }
    }

    .activity-time,
    .activity-ms {
        color: $text-muted;
        font-variant-numeric: tabular-nums;
    }

    .activity-tool {
        font-family: monospace;
        color: $text;
    }

    .activity-client {
        color: $text-muted;
    }

    .activity-msg {
        flex-basis: 100%;
        color: $required-color;
        overflow-wrap: anywhere;
    }
</style>
