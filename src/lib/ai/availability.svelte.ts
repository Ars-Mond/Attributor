// Shared reactive AI state per provider: status, the models the provider reports, whether an API key is stored, and the ChatGPT account.
// Prefetched for the active provider at app startup so the settings page shows suggestions instantly (no wait).
import {settings} from '$lib/settings';
import {
    chatgptAccount, hasApiKey, listModels, providerStatus,
    type ChatGptAccount, type ProviderStatus
} from './ai';
import {activeModelKey, providerInfo, type ProviderId} from './providers';

let statuses = $state<Partial<Record<ProviderId, ProviderStatus>>>({});
let models = $state<Partial<Record<ProviderId, string[]>>>({});
let keys = $state<Partial<Record<ProviderId, boolean>>>({});
let account = $state<ChatGptAccount | null>(null);
let checking = $state<Partial<Record<ProviderId, boolean>>>({});

function current(): ProviderId {
    return providerInfo(settings.subscribe<string>('ai.provider')()).id;
}

export const ai = {
    /** The provider selected for attribution (reactive). */
    get provider(): ProviderId {return current();},
    get info() {return providerInfo(current());},
    get label() {return providerInfo(current()).label;},
    get status() {return statuses[current()] ?? null;},
    get installed() {return statuses[current()]?.installed ?? false;},
    get reachable() {return statuses[current()]?.reachable ?? false;},
    get version() {return statuses[current()]?.version ?? null;},
    get checking() {return checking[current()] ?? false;},
    get installedModels() {return models[current()] ?? [];},
    get account() {return account;},

    statusOf(id: ProviderId): ProviderStatus | null {return statuses[id] ?? null;},
    modelsOf(id: ProviderId): string[] {return models[id] ?? [];},
    hasKey(id: ProviderId): boolean {return keys[id] ?? false;},

    /** Reactive: attribution is available when the active provider is usable and has an active model.
     *  Ollama needs to be installed (inference auto-starts it); local servers are tried on use; cloud providers need a key or a ChatGPT sign-in. */
    get available() {
        const id = current();
        if (!settings.subscribe<string>(activeModelKey(id))()) return false;
        switch (providerInfo(id).auth) {
            case 'key': return keys[id] ?? false;
            case 'oauth': return account !== null;
            default: return id === 'ollama' ? statuses[id]?.installed ?? false : true;
        }
    },

    /** Re-check the provider's status (the settings "Check" button + startup). */
    async refresh(id: ProviderId = current()): Promise<void> {
        checking[id] = true;
        try {
            statuses[id] = await providerStatus(id);
        } catch (e) {
            statuses[id] = {installed: false, reachable: false, version: null, detail: String(e)};
        } finally {
            checking[id] = false;
        }
    },

    /** Re-fetch the models the provider reports (the settings "Refresh" button + startup). */
    async refreshModels(id: ProviderId = current()): Promise<void> {
        try {
            models[id] = (await listModels(id)).map(m => m.name);
        } catch {
            models[id] = [];
        }
    },

    /** Re-read whether an API key is stored for the provider. */
    async refreshKey(id: ProviderId = current()): Promise<void> {
        if (providerInfo(id).auth !== 'key' && providerInfo(id).auth !== 'optionalKey') return;
        try {
            keys[id] = await hasApiKey(id);
        } catch {
            keys[id] = false;
        }
    },

    /** Re-read the ChatGPT account. */
    async refreshAccount(): Promise<void> {
        try {
            account = await chatgptAccount();
        } catch {
            account = null;
        }
    },

    /** Credentials first (the status and model list depend on them), then status and models. */
    async init(id: ProviderId = current()): Promise<void> {
        await this.refreshKey(id);
        if (providerInfo(id).auth === 'oauth') await this.refreshAccount();
        await this.refresh(id);
        await this.refreshModels(id);
    }
};
