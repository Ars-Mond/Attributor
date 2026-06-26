// Reactive Ollama availability used to enable/disable the attribute action (FR-007/009).
// `available` = Ollama is INSTALLED AND an active model is selected. The daemon is auto-started on
// inference if it is not running, so the button does not require it to be running up front.
import {settings} from '$lib/settings';
import {ollamaStatus} from './ollama';

let installed = $state(false);
let reachable = $state(false);
let checking = $state(false);

export const ollama = {
    get installed() {return installed;},
    get reachable() {return reachable;},
    get checking() {return checking;},
    // Reactive: tracks the installed state and the active-model setting.
    get available() {return installed && !!settings.subscribe<string>('ollama.activeModel')();},

    async refresh(): Promise<void> {
        checking = true;
        try {
            const status = await ollamaStatus();
            installed = status.installed;
            reachable = status.reachable;
        } catch {
            installed = false;
            reachable = false;
        } finally {
            checking = false;
        }
    }
};
