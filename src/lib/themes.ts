import {getCurrentWindow} from '@tauri-apps/api/window';

export type ThemeId = 'system' | 'light' | 'dark';

// Selectable themes for the Appearance settings dropdown. `label` holds an i18n message key
// (resolved via t() at render time); `system` follows the OS color scheme.
export const THEME_OPTIONS: {value: ThemeId; label: string}[] = [
    {value: 'system', label: 'theme.system'},
    {value: 'light', label: 'theme.light'},
    {value: 'dark', label: 'theme.dark'},
];

export const DEFAULT_THEME: ThemeId = 'system';

// The `appearance.font_size` value that maps to 100% (no scaling). The CSS typographic scale
// (`$fs-*` in _mixins.scss) is multiplied by `--font-scale`, so this keeps the default look intact.
const FONT_BASELINE = 14;

// Resolve `system` to a concrete scheme using the OS preference; pass `light`/`dark` through.
function resolveTheme(id: string): 'light' | 'dark' {
    if (id === 'light' || id === 'dark') return id;
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function applyTheme(id: string): void {
    const resolved = resolveTheme(id);
    document.documentElement.setAttribute('data-theme', resolved);
    getCurrentWindow().setTheme(resolved).catch(() => {});
}

// Drive the global UI font scale from the `appearance.font_size` setting.
export function applyFontScale(value: number): void {
    const scale = (value > 0 ? value : FONT_BASELINE) / FONT_BASELINE;
    document.documentElement.style.setProperty('--font-scale', String(scale));
}
