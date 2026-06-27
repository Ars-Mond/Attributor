// Shared reactive Ollama state: installed/running status + the list of installed models.
// Prefetched once at app startup so the settings page shows suggestions instantly (no wait).
import {settings} from '$lib/settings';
import {ollamaStatus, listModels, type OllamaStatus} from './ollama';

let status = $state<OllamaStatus | null>(null);
let installedModels = $state<string[]>([]);
let checking = $state(false);

export const ollama = {
    get status() {return status;},
    get installed() {return status?.installed ?? false;},
    get reachable() {return status?.reachable ?? false;},
    get version() {return status?.version ?? null;},
    get checking() {return checking;},
    get installedModels() {return installedModels;},
    // Reactive: attribution is available when Ollama is installed AND an active model is selected.
    get available() {return (status?.installed ?? false) && !!settings.subscribe<string>('ollama.activeModel')();},

    /** Re-check installed/running status (the settings "Check" button + startup). */
    async refresh(): Promise<void> {
        checking = true;
        try {
            status = await ollamaStatus();
        } catch {
            status = {installed: false, reachable: false, version: null};
        } finally {
            checking = false;
        }
    },

    /** Re-fetch the installed-models list (the settings "Refresh" button + startup). */
    async refreshModels(): Promise<void> {
        try {
            installedModels = (await listModels()).map(m => m.name);
        } catch {
            installedModels = [];
        }
    },

    /** Startup prefetch: status + installed models. */
    async init(): Promise<void> {
        await this.refresh();
        await this.refreshModels();
    }
};
