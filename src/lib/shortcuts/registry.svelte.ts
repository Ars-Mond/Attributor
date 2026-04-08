import {load, type Store} from '@tauri-apps/plugin-store';
import {debug, warn, trace} from '@tauri-apps/plugin-log';
import type {ActionDescriptor, LayerConfig} from './types';

let _store: Store | null = null;

async function getStore(): Promise<Store> {
    return (_store ??= await load('shortcuts.json', {autoSave: false, defaults: {}}));
}

export function normalizeBinding(e: KeyboardEvent): string {
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return '';
    const parts: string[] = [];
    if (e.ctrlKey)  parts.push('Ctrl');
    if (e.shiftKey) parts.push('Shift');
    if (e.altKey)   parts.push('Alt');
    if (e.metaKey)  parts.push('Meta');
    const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
    parts.push(key);
    return parts.join('+');
}

class ShortcutRegistry {
    #actions = new Map<string, ActionDescriptor>();
    #layers = new Map<string, LayerConfig>();
    #activeLayers = new Set<string>();
    #userBindings = $state<Record<string, string | null>>({});
    #saveTimer: ReturnType<typeof setTimeout> | null = null;

    registerLayer(config: LayerConfig): void {
        this.#layers.set(config.id, config);
        trace(`[shortcuts] layer registered: ${config.id} (priority=${config.priority})`);
    }

    activateLayer(id: string): void {
        this.#activeLayers.add(id);
        debug(`[shortcuts] layer activated: ${id}`);
    }

    deactivateLayer(id: string): void {
        this.#activeLayers.delete(id);
        debug(`[shortcuts] layer deactivated: ${id}`);
    }

    registerAction(descriptor: ActionDescriptor): void {
        this.#actions.set(descriptor.id, {...descriptor});
        trace(`[shortcuts] action registered: ${descriptor.id} binding=${descriptor.defaultBinding ?? 'none'}`);
    }

    setHandler(actionId: string, handler: () => void): void {
        const action = this.#actions.get(actionId);
        if (action) {
            action.handler = handler;
            trace(`[shortcuts] handler set: ${actionId}`);
        } else {
            warn(`[shortcuts] setHandler: unknown action "${actionId}"`);
        }
    }

    getUserBinding(actionId: string): string | null {
        return this.#userBindings[actionId] ?? null;
    }

    setUserBinding(actionId: string, binding: string | null): void {
        if (binding === null) {
            const copy = {...this.#userBindings};
            delete copy[actionId];
            this.#userBindings = copy;
            debug(`[shortcuts] binding reset to default: ${actionId}`);
        } else {
            this.#userBindings = {...this.#userBindings, [actionId]: binding};
            debug(`[shortcuts] user binding set: ${actionId} → ${binding}`);
        }
        this.#scheduleSave();
    }

    getEffectiveBinding(actionId: string): string | null {
        const user = this.#userBindings[actionId];
        if (user !== undefined) return user;
        return this.#actions.get(actionId)?.defaultBinding ?? null;
    }

    getConflict(binding: string, excludeActionId?: string): ActionDescriptor | null {
        for (const action of this.#actions.values()) {
            if (action.id === excludeActionId) continue;
            if (this.getEffectiveBinding(action.id) === binding) return action;
        }
        return null;
    }

    handleKeyDown(e: KeyboardEvent): boolean {
        const binding = normalizeBinding(e);
        if (!binding) return false;

        const sortedLayers = [...this.#layers.values()].sort((a, b) => b.priority - a.priority);

        for (const layer of sortedLayers) {
            const isActive = this.#activeLayers.has(layer.id) || (layer.autoActivate?.() === true);
            if (!isActive) continue;

            for (const action of this.#actions.values()) {
                if (this.getEffectiveBinding(action.id) === binding) {
                    debug(`[shortcuts] fired: ${action.id} (${binding}) via layer "${layer.id}"`);
                    action.handler();
                    return true;
                }
            }

            if (layer.suppressBelow) {
                trace(`[shortcuts] suppressed by layer "${layer.id}" for binding ${binding}`);
                return false;
            }
        }

        return false;
    }

    getSections(): string[] {
        const seen = new Set<string>();
        const result: string[] = [];
        for (const action of this.#actions.values()) {
            if (!seen.has(action.section)) {
                seen.add(action.section);
                result.push(action.section);
            }
        }
        return result;
    }

    getActionsBySection(section: string): ActionDescriptor[] {
        return [...this.#actions.values()].filter(a => a.section === section);
    }

    resetAll(): void {
        this.#userBindings = {};
        debug('[shortcuts] all user bindings reset');
        this.#scheduleSave();
    }

    async load(): Promise<void> {
        try {
            const s = await getStore();
            const stored = await s.get<Record<string, string | null>>('shortcuts');
            if (stored && typeof stored === 'object') {
                const bindings: Record<string, string | null> = {};
                for (const [k, v] of Object.entries(stored)) {
                    if (this.#actions.has(k)) {
                        bindings[k] = v;
                    }
                }
                this.#userBindings = bindings;
                debug(`[shortcuts] loaded ${Object.keys(bindings).length} user binding(s)`);
            } else {
                debug('[shortcuts] no stored bindings found, using defaults');
            }
        } catch (e) {
            warn(`[shortcuts] load failed: ${e}`);
        }
    }

    async save(): Promise<void> {
        await this.#doSave();
    }

    #scheduleSave(): void {
        if (this.#saveTimer) clearTimeout(this.#saveTimer);
        this.#saveTimer = setTimeout(() => {
            this.#saveTimer = null;
            this.#doSave().catch(e => warn(`[shortcuts] auto-save failed: ${e}`));
        }, 300);
    }

    async #doSave(): Promise<void> {
        try {
            const s = await getStore();
            await s.set('shortcuts', {...this.#userBindings});
            await s.save();
            debug(`[shortcuts] saved ${Object.keys(this.#userBindings).length} user binding(s)`);
        } catch (e) {
            warn(`[shortcuts] save failed: ${e}`);
        }
    }
}

export const shortcuts = new ShortcutRegistry();
