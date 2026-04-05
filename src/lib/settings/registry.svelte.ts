import {load, type Store} from '@tauri-apps/plugin-store';
import type {SettingDescriptor} from './types';

let _store: Store | null = null;

async function getStore(): Promise<Store> {
    return (_store ??= await load('settings.json', {autoSave: false, defaults: {}}));
}

class SettingsRegistry {
    #descriptors = new Map<string, SettingDescriptor>();
    #values = $state<Record<string, unknown>>({});
    #saveTimer: ReturnType<typeof setTimeout> | null = null;

    register<T>(descriptor: SettingDescriptor<T>): void {
        this.#descriptors.set(descriptor.key, descriptor as SettingDescriptor);
        // Keep any value already loaded via load() (called before register in edge cases)
        if (!(descriptor.key in this.#values)) {
            this.#values[descriptor.key] = descriptor.default;
        }
    }

    get<T>(key: string): T {
        const v = this.#values[key];
        if (v !== undefined) return v as T;
        return this.#descriptors.get(key)?.default as T;
    }

    set<T>(key: string, value: T): void {
        this.#values[key] = value;
        this.#scheduleSave();
    }

    /** Returns a getter function whose call in a `$derived` context tracks the value. */
    subscribe<T>(key: string): () => T {
        return () => this.get<T>(key);
    }

    async load(): Promise<void> {
        try {
            const s = await getStore();
            const stored = await s.get<Record<string, unknown>>('settings');
            if (stored && typeof stored === 'object') {
                for (const [k, v] of Object.entries(stored)) {
                    if (this.#descriptors.has(k)) {
                        this.#values[k] = v;
                    }
                }
            }
        } catch (e) {
            console.warn('settings.load failed:', e);
        }
    }

    async save(): Promise<void> {
        await this.#doSave();
    }

    getSections(): string[] {
        const seen = new Set<string>();
        const result: string[] = [];
        for (const d of this.#descriptors.values()) {
            if (!seen.has(d.section)) {
                seen.add(d.section);
                result.push(d.section);
            }
        }
        return result;
    }

    getBySection(section: string): SettingDescriptor[] {
        return [...this.#descriptors.values()].filter(d => d.section === section);
    }

    #scheduleSave(): void {
        if (this.#saveTimer) clearTimeout(this.#saveTimer);
        this.#saveTimer = setTimeout(() => {
            this.#saveTimer = null;
            this.#doSave().catch(e => console.warn('settings auto-save failed:', e));
        }, 300);
    }

    async #doSave(): Promise<void> {
        try {
            const s = await getStore();
            const data: Record<string, unknown> = {};
            for (const k of this.#descriptors.keys()) {
                data[k] = this.#values[k];
            }
            await s.set('settings', data);
            await s.save();
        } catch (e) {
            console.warn('settings.save failed:', e);
        }
    }
}

export const settings = new SettingsRegistry();
