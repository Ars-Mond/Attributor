// Reactive Ollama availability used to enable/disable the attribute action (FR-007/009).
// `available` = the daemon is reachable AND an active model is selected.
import {settings} from '$lib/settings';
import {ollamaStatus} from './ollama';

let reachable = $state(false);
let checking = $state(false);

export const ollama = {
    get reachable() {return reachable;},
    get checking() {return checking;},
    // Reactive: tracks both the reachability state and the active-model setting.
    get available() {return reachable && !!settings.subscribe<string>('ollama.activeModel')();},

    async refresh(): Promise<void> {
        checking = true;
        try {
            const status = await ollamaStatus();
            reachable = status.reachable;
        } catch {
            reachable = false;
        } finally {
            checking = false;
        }
    }
};
