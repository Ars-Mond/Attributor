import {invoke} from '@tauri-apps/api/core';
import {warn} from '@tauri-apps/plugin-log';
import {settings} from '$lib/settings';
import {normalizeLocale} from './store.svelte';

export {LOCALES, DEFAULT_LOCALE, ENDONYMS} from './types';
export type {Locale, Messages, MessageKey, PluralKey, PluralForms, MessageParams} from './types';
export {pluralCategory} from './plural';
export {t, tn, locale, setLocale, normalizeLocale} from './store.svelte';

let initialized = false;

// One-time startup init. MUST run after settings.load() and before the window is shown, so the first
// painted frame is already localized. On a first-ever launch (no saved language) it auto-detects the OS
// language; an explicit saved choice always wins. Idempotent.
export async function initLocale(): Promise<void> {
    if (initialized) return;
    initialized = true;
    if (settings.wasPersisted('general.language')) return;
    try {
        const tag = await invoke<string>('detect_os_locale');
        settings.set('general.language', normalizeLocale(tag));
    } catch (e) {
        warn(`i18n: OS locale detection failed, falling back to default: ${e}`);
    }
}
