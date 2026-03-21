import {getCurrentWindow} from '@tauri-apps/api/window';

export interface ThemeInfo {
    id: string;
    name: string;
}

export const themes: ThemeInfo[] = [
    {id: 'dark', name: 'Dark'},
    {id: 'light', name: 'Light'},
];

export const DEFAULT_THEME = 'light';

export function applyTheme(id: string) {
    document.documentElement.setAttribute('data-theme', id);
    getCurrentWindow().setTheme(id === 'light' ? 'light' : 'dark');
}
