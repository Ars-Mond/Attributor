import {load, type Store} from '@tauri-apps/plugin-store';

const FILE = 'ui-state.json';
let _store: Store | null = null;

async function getStore(): Promise<Store> {
    return (_store ??= await load(FILE, {autoSave: false, defaults: {}}));
}

export interface AppState {
    lastFolder: string;
    lastFile: string;
    leftPanelWidth: number;
    rightPanelWidth: number;
    windowMaximized: boolean;
    windowWidth: number;
    windowHeight: number;
    descriptionHeight: number;
    stockKeywordsOpen: boolean;
    optionalOpen: boolean;
}

export async function loadAppState(): Promise<Partial<AppState>> {
    try {
        const s = await getStore();
        const keys: (keyof AppState)[] = [
            'lastFolder', 'lastFile',
            'leftPanelWidth', 'rightPanelWidth',
            'windowMaximized', 'windowWidth', 'windowHeight',
            'descriptionHeight', 'stockKeywordsOpen', 'optionalOpen',
        ];
        const values = await Promise.all(keys.map(k => s.get(k)));
        return Object.fromEntries(
            keys.map((k, i) => [k, values[i]]).filter(([, v]) => v !== undefined)
        ) as Partial<AppState>;
    } catch (e) {
        console.warn('loadAppState failed:', e);
        return {};
    }
}

export async function saveAppState(patch: Partial<AppState>): Promise<void> {
    try {
        const s = await getStore();
        await Promise.all(Object.entries(patch).map(([k, v]) => s.set(k, v)));
        await s.save();
    } catch (e) {
        console.warn('saveAppState failed:', e);
    }
}
