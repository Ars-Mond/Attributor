import {load, type Store} from '@tauri-apps/plugin-store';
import type {SettingDescriptor, SettingsSectionConfig} from './types';
import {SettingsSection} from './SettingsSection';

let _store: Store | null = null;

async function getStore(): Promise<Store> {
    return (_store ??= await load('settings.json', {autoSave: false, defaults: {}}));
}

class SettingsRegistry {
    #sections = new Map<string, SettingsSection>();
    #descriptors = new Map<string, SettingDescriptor>();
    #values = $state<Record<string, unknown>>({});
    #saveTimer: ReturnType<typeof setTimeout> | null = null;

    registerSection(config: SettingsSectionConfig): void {
        if (!this.#sections.has(config.id)) {
            this.#sections.set(config.id, new SettingsSection(config, this.#sections.size));
        }
    }

    register<T>(sectionId: string, descriptor: SettingDescriptor<T>): void {
        // Auto-create section if not explicitly registered
        if (!this.#sections.has(sectionId)) {
            this.registerSection({id: sectionId, label: sectionId});
        }
        const section = this.#sections.get(sectionId)!;
        section.fields.push(descriptor as SettingDescriptor);

        if (descriptor.key) {
            this.#descriptors.set(descriptor.key, descriptor as SettingDescriptor);
            if (!(descriptor.key in this.#values)) {
                this.#values[descriptor.key] = descriptor.default;
            }
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

    reset(key: string): void {
        if (!key) return;
        const d = this.#descriptors.get(key);
        if (d) {
            this.#values[key] = d.default;
            this.#scheduleSave();
        }
    }

    resetSection(sectionId: string): void {
        const section = this.#sections.get(sectionId);
        if (!section) return;
        for (const d of section.fields) {
            if (d.key) this.#values[d.key] = d.default;
        }
        this.#scheduleSave();
    }

    getSection(id: string): SettingsSection | undefined {
        return this.#sections.get(id);
    }

    getAllSections(): SettingsSection[] {
        return [...this.#sections.values()].sort((a, b) => a.order - b.order);
    }

    async load(): Promise<void> {
        try {
            const s = await getStore();
            const stored = await s.get<Record<string, unknown>>('settings');
            if (stored && typeof stored === 'object') {
                for (const [k, v] of Object.entries(stored)) {
                    if (k && this.#descriptors.has(k)) {
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
                if (k) data[k] = this.#values[k];
            }
            await s.set('settings', data);
            await s.save();
        } catch (e) {
            console.warn('settings.save failed:', e);
        }
    }
}

export const settings = new SettingsRegistry();
