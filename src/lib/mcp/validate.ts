// Checks of setting values sent by MCP clients. The settings dialog constrains input through its controls; values arriving over MCP get the same limits here before they are stored.
import type {SettingDescriptor} from '$lib/settings/types';
import {DEFAULT_PROVIDER, PROFILE_PROVIDERS, PROVIDERS, profileProviderInfo, type ProfileProviderId} from '$lib/ai/providers';
import {BOOL_FORMATS, DELIMITERS, VALUE_TYPES, isValidIdentifier} from '$lib/csv/csv';

/** The checked value, or the reason it was refused. */
export type Checked = {ok: true; value: unknown} | {ok: false; error: string};

const ok = (value: unknown): Checked => ({ok: true, value});
const fail = (error: string): Checked => ({ok: false, error});

function isObject(v: unknown): v is Record<string, unknown> {
    return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function inRange(n: number, d: SettingDescriptor): string | null {
    if (d.min !== undefined && n < d.min) return `must be at least ${d.min}`;
    if (d.max !== undefined && n > d.max) return `must be at most ${d.max}`;
    return null;
}

/** Checks `value` against the setting's type, range, options and the extra rules of AI and CSV settings. */
export function checkSetting(d: SettingDescriptor, value: unknown): Checked {
    switch (d.type) {
        case 'boolean':
            return typeof value === 'boolean' ? ok(value) : fail('must be true or false');
        case 'int': {
            if (typeof value !== 'number' || !Number.isInteger(value)) return fail('must be a whole number');
            const range = inRange(value, d);
            return range ? fail(range) : ok(value);
        }
        case 'float': {
            if (typeof value !== 'number' || !Number.isFinite(value)) return fail('must be a number');
            const range = inRange(value, d);
            return range ? fail(range) : ok(value);
        }
        case 'string':
            return checkString(d, value);
        case 'custom':
            if (d.key === 'ai.modelProfiles') return checkProfiles(value);
            if (d.key === 'csv.presets') return checkCsvPresets(value);
            return fail('cannot be changed through MCP');
    }
}

function checkString(d: SettingDescriptor, value: unknown): Checked {
    if (typeof value !== 'string') return fail('must be a string');
    if (d.options && !d.options.some(o => o.value === value)) {
        return fail(`must be one of ${d.options.map(o => o.value).join(', ')}`);
    }
    if (d.key === 'ai.provider' && !PROVIDERS.some(p => p.id === value)) {
        return fail(`must be one of ${PROVIDERS.map(p => p.id).join(', ')}`);
    }
    if (d.key.endsWith('.baseUrl')) {
        const url = value.trim();
        return /^https?:\/\/\S+$/i.test(url) ? ok(url) : fail('must be an http:// or https:// address');
    }
    if (d.key.endsWith('.activeModel')) return ok(value.trim());
    if (d.key === 'ai.responseFormat') {
        try {
            if (!isObject(JSON.parse(value))) return fail('must be a JSON schema object');
        } catch {
            return fail('must be valid JSON');
        }
    }
    return ok(value);
}

/** Model profiles as the profile dialog saves them: a known owner, a thinking value the owner offers (true and false as booleans), one profile per owner and model id, trimmed names. */
function checkProfiles(value: unknown): Checked {
    if (!Array.isArray(value)) return fail('must be an array of model profiles');
    const owners = new Set<string>(PROFILE_PROVIDERS.map(p => p.id));
    const ids = new Set<string>();
    const models = new Set<string>();
    const out: Record<string, unknown>[] = [];
    for (const [i, p] of value.entries()) {
        const at = `profile ${i + 1}`;
        if (!isObject(p)) return fail(`${at} must be an object`);
        if (typeof p.id !== 'string' || !p.id) return fail(`${at}: id must be a non-empty string`);
        if (ids.has(p.id)) return fail(`${at}: id ${p.id} is used twice`);
        ids.add(p.id);
        if (typeof p.name !== 'string' || !p.name.trim()) return fail(`${at}: name must be a non-empty string`);
        if (p.provider !== undefined && (typeof p.provider !== 'string' || !owners.has(p.provider))) {
            return fail(`${at}: provider must be one of ${[...owners].join(', ')}`);
        }
        const owner = (p.provider as ProfileProviderId | undefined) ?? DEFAULT_PROVIDER;
        if (typeof p.modelId !== 'string' || !p.modelId.trim()) return fail(`${at}: modelId must be a non-empty string ("base" applies to every model)`);
        const modelId = p.modelId.trim();
        const modelKey = `${owner}\u0000${modelId}`;
        if (models.has(modelKey)) return fail(`${at}: ${owner} already has a profile for ${modelId}`);
        models.add(modelKey);
        if (typeof p.prompt !== 'string') return fail(`${at}: prompt must be a string`);
        const offered = profileProviderInfo(owner).thinkOptions;
        const think = p.think;
        const thinkOk = think === undefined || think === null
            || (typeof think === 'boolean' && offered.includes(String(think)))
            || (typeof think === 'string' && think !== 'true' && think !== 'false' && offered.includes(think));
        if (!thinkOk) return fail(`${at}: think must be null or one of ${offered.join(', ')} (true and false as booleans)`);
        if (p.keepAlive !== undefined && p.keepAlive !== null && typeof p.keepAlive !== 'string') return fail(`${at}: keepAlive must be a string or null`);
        if (!isObject(p.options) || !Object.values(p.options).every(v => ['number', 'string', 'boolean'].includes(typeof v))) {
            return fail(`${at}: options must be an object of numbers, strings and booleans`);
        }
        out.push({...p, name: p.name.trim(), modelId});
    }
    return ok(out);
}

/** CSV presets as the preset dialog saves them: trimmed names, identifiers that work as file names and differ regardless of case, known delimiters and field types. */
function checkCsvPresets(value: unknown): Checked {
    if (!Array.isArray(value)) return fail('must be an array of CSV presets');
    const ids = new Set<string>();
    const identifiers = new Set<string>();
    const out: Record<string, unknown>[] = [];
    for (const [i, p] of value.entries()) {
        const at = `preset ${i + 1}`;
        if (!isObject(p)) return fail(`${at} must be an object`);
        if (typeof p.id !== 'string' || !p.id || ids.has(p.id)) return fail(`${at}: id must be a unique non-empty string`);
        ids.add(p.id);
        if (typeof p.name !== 'string' || !p.name.trim()) return fail(`${at}: name must be a non-empty string`);
        const identifier = typeof p.identifier === 'string' ? p.identifier.trim() : '';
        if (!isValidIdentifier(identifier) || identifiers.has(identifier.toLowerCase())) {
            return fail(`${at}: identifier must be usable as a file name and differ from the others (ignoring case)`);
        }
        identifiers.add(identifier.toLowerCase());
        if (!DELIMITERS.includes(p.delimiter as never)) return fail(`${at}: delimiter must be one of ${DELIMITERS.join(', ')}`);
        if (!Array.isArray(p.fields) || p.fields.length === 0) return fail(`${at}: fields must be a non-empty array`);
        for (const [j, f] of p.fields.entries()) {
            const fat = `${at}, field ${j + 1}`;
            if (!isObject(f)) return fail(`${fat} must be an object`);
            if (typeof f.csvColumn !== 'string') return fail(`${fat}: csvColumn must be a string`);
            if (!VALUE_TYPES.includes(f.valueType as never)) return fail(`${fat}: valueType must be one of ${VALUE_TYPES.join(', ')}`);
            if (typeof f.defaultValue !== 'string') return fail(`${fat}: defaultValue must be a string`);
            if (!BOOL_FORMATS.includes(f.boolFormat as never)) return fail(`${fat}: boolFormat must be one of ${BOOL_FORMATS.join(', ')}`);
        }
        out.push({...p, name: p.name.trim(), identifier});
    }
    return ok(out);
}
