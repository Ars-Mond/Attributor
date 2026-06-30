// CSV export presets (feature 009): types, defaults, and validation helpers. Mirrors the Rust types
// in src-tauri/src/csv/mod.rs across the IPC boundary (camelCase).

export type AppValueType =
    | 'none'
    | 'fileName'
    | 'title'
    | 'description'
    | 'keywords'
    | 'category'
    | 'releaseFilename'
    | 'editorial'
    | 'matureContent'
    | 'illustration';

export type Delimiter = 'comma' | 'semicolon' | 'tab';
export type BoolFormat = 'yesNo' | 'trueFalse';

/** One column in a preset. `defaultValue` is used only for `none`; `boolFormat` only for bool types. */
export interface CsvField {
    csvColumn: string;
    valueType: AppValueType;
    defaultValue: string;
    boolFormat: BoolFormat;
}

/** One stock's export configuration. */
export interface CsvPreset {
    id: string;
    name: string;
    identifier: string;
    delimiter: Delimiter;
    fields: CsvField[];
}

/** Result of one export run. */
export interface ExportSummary {
    filesWritten: number;
    photosExported: number;
    skipped: number;
}

// The fixed value-type set in display order (FR-023).
export const VALUE_TYPES: AppValueType[] = [
    'none', 'fileName', 'title', 'description', 'keywords', 'category', 'releaseFilename', 'editorial', 'matureContent', 'illustration',
];

export const DELIMITERS: Delimiter[] = ['comma', 'semicolon', 'tab'];
export const BOOL_FORMATS: BoolFormat[] = ['yesNo', 'trueFalse'];

const BOOL_TYPES: AppValueType[] = ['editorial', 'matureContent', 'illustration'];

/** Whether a value type renders a boolean (shows the bool-format control). */
export function isBoolType(t: AppValueType): boolean {
    return BOOL_TYPES.includes(t);
}

/** Whether a value type is the constant placeholder (shows the default-value input). */
export function isNoneType(t: AppValueType): boolean {
    return t === 'none';
}

// Seed for the registered `csv.presets` key: empty. The user creates presets explicitly.
export const DEFAULT_CSV_PRESETS: CsvPreset[] = [];

function uid(): string {
    return 'c' + Math.random().toString(36).slice(2, 10);
}

export function createEmptyField(): CsvField {
    return {csvColumn: '', valueType: 'title', defaultValue: '', boolFormat: 'yesNo'};
}

export function createEmptyPreset(): CsvPreset {
    return {id: uid(), name: '', identifier: '', delimiter: 'comma', fields: [createEmptyField()]};
}

// Characters illegal in a file name on any supported OS, and reserved Windows device names. Kept in
// sync with the defensive guard in src-tauri/src/csv/mod.rs (research R9) so a preset is portable.
// Internal hyphens/spaces are allowed; leading/trailing dots-or-spaces and control chars are not.
const ILLEGAL_FILENAME = /[<>:"/\\|?*]/;
const RESERVED_DEVICE = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\.|$)/i;

/** Whether `id` is safe to use as a `<id>.csv` file name (FR-020). */
export function isValidIdentifier(id: string): boolean {
    if (!id) return false;
    if (ILLEGAL_FILENAME.test(id)) return false;
    for (let i = 0; i < id.length; i++) {
        if (id.charCodeAt(i) < 0x20) return false; // reject control characters (mirrors the Rust guard)
    }
    if (id !== id.replace(/^[.\s]+|[.\s]+$/g, '')) return false; // no leading/trailing dot or space
    if (RESERVED_DEVICE.test(id)) return false;
    return true;
}
