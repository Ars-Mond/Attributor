import {warn} from '@tauri-apps/plugin-log';
import {settings} from '$lib/settings';
import {catalog} from './catalog';
import {pluralCategory} from './plural';
import {DEFAULT_LOCALE, LOCALES, type Locale, type MessageKey, type MessageParams, type PluralForms, type PluralKey} from './types';

// Dev-only: warn once per key when the active locale lacks it and we fall back to the default.
const warnedMissing = new Set<string>();
function warnMissing(key: string, loc: Locale): void {
    if (import.meta.env.DEV && !warnedMissing.has(key)) {
        warnedMissing.add(key);
        warn(`i18n: missing key "${key}" for locale "${loc}", using default-language text`);
    }
}

// Normalize any string (a BCP-47 tag, a saved value) to a supported Locale, defaulting when unknown.
export function normalizeLocale(value: string | null | undefined): Locale {
    const sub = (value ?? '').toLowerCase().split(/[-_]/)[0];
    return (LOCALES as string[]).includes(sub) ? (sub as Locale) : DEFAULT_LOCALE;
}

// The active locale is derived from the persisted `general.language` setting. Because t()/tn() read
// this rune, any markup that calls them re-renders the instant the language changes — no restart.
const active = $derived(normalizeLocale(settings.subscribe<string>('general.language')()));

export function locale(): Locale {
    return active;
}

// Change the active language (and persist it). Unknown values normalize to the default.
export function setLocale(value: string): void {
    settings.set('general.language', normalizeLocale(value));
}

function interpolate(template: string, params?: MessageParams): string {
    if (!params) return template;
    return template.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m));
}

// Translate a key, interpolating {name} placeholders. Fallback: active -> default language -> the key.
export function t(key: MessageKey, params?: MessageParams): string {
    let raw = catalog[active][key];
    if (raw === undefined) {
        warnMissing(key, active);
        raw = catalog[DEFAULT_LOCALE][key];
    }
    if (raw === undefined) return key;
    return interpolate(raw, params);
}

// Plural-aware translate: pick the CLDR form for the active locale, exposing the count as {n}.
export function tn(key: PluralKey, count: number, params?: MessageParams): string {
    let forms: PluralForms | undefined = catalog[active][key];
    if (forms === undefined) {
        warnMissing(key, active);
        forms = catalog[DEFAULT_LOCALE][key];
    }
    if (!forms) return key;
    const cat = pluralCategory(active, count);
    const form = forms[cat] ?? forms.other ?? forms.many ?? forms.one;
    return interpolate(form, {n: count, ...params});
}
