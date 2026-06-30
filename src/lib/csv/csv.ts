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

// Compact builder for a default field (all default fields use the yes/no bool format).
function col(csvColumn: string, valueType: AppValueType, defaultValue = ''): CsvField {
    return {csvColumn, valueType, defaultValue, boolFormat: 'yesNo'};
}

// Presets seeded on first run / restored on reset — ready-to-use layouts for the major stocks.
// An existing user's saved presets always win over these defaults (only fresh installs get them).
export const DEFAULT_CSV_PRESETS: CsvPreset[] = [
    {
        id: 'default-shutterstock',
        name: 'Shutterstock',
        identifier: 'shutterstock',
        delimiter: 'comma',
        fields: [
            col('Filename', 'fileName'),
            col('Description', 'description'),
            col('Keywords', 'keywords'),
            col('Categories', 'category'),
            col('Illustration', 'illustration'),
            col('Mature Content', 'matureContent'),
            col('Editorial', 'editorial'),
        ],
    },
    {
        id: 'default-istock',
        name: 'iStock',
        identifier: 'i_stock',
        delimiter: 'comma',
        fields: [
            col('file name', 'fileName'),
            col('description', 'description'),
            col('country', 'none'),
            col('title', 'title'),
            col('keywords', 'keywords'),
            col('poster timecode', 'none'),
            col('date created', 'none'),
            col('shot speed', 'none'),
        ],
    },
    {
        id: 'default-adobe-stock',
        name: 'Adobe Stock',
        identifier: 'adobe_stock',
        delimiter: 'comma',
        fields: [
            col('Filename', 'fileName'),
            col('Title', 'title'),
            col('Keywords', 'keywords'),
            col('Category', 'category'),
            col('Releases', 'releaseFilename'),
        ],
    },
    {
        id: 'default-envato',
        name: 'Envato',
        identifier: 'envato',
        delimiter: 'comma',
        fields: [
            col('Filename', 'fileName'),
            col('Title', 'title'),
            col('Description', 'description'),
            col('Keywords', 'keywords'),
            col('Category', 'category'),
            col('Price: Single Use License ($USD)', 'none', '$0.00'),
            col('Price: Multi-use License ($USD)*', 'none', '$0.00'),
            col('Recognisable people?', 'none', 'No'),
            col('Recognisable buildings?', 'none', 'No'),
            col('Releases', 'releaseFilename'),
            col('Is Motion Graphics?', 'none'),
            col('AudioJungle Track (IDs)', 'none'),
            col('Color', 'none'),
            col('Pace', 'none'),
            col('Movement', 'none'),
            col('Composition', 'none'),
            col('Setting', 'none'),
            col('No. of People', 'none'),
            col('Gender', 'none'),
            col('Age', 'none'),
            col('Ethnicity', 'none'),
            col('Alpha Channel', 'none'),
            col('Looped', 'none'),
            col('Source Audio', 'none'),
        ],
    },
];

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
