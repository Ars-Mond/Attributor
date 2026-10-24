import {settings} from './registry.svelte';
import {LOCALES, ENDONYMS} from '$lib/i18n/types';
import ShortcutsPage from '$lib/shortcuts/ShortcutsPage.svelte';
import AiSettingsPage from './AiSettingsPage.svelte';
import ModelProfilesPage from './ModelProfilesPage.svelte';
import CsvPresetsPage from './CsvPresetsPage.svelte';
import McpSettingsPage from './McpSettingsPage.svelte';
import type {ModelProfile} from '$lib/ai/ai';
import {DEFAULT_MODEL_PROFILES, DEFAULT_RESPONSE_FORMAT} from '$lib/ai/defaults';
import {activeModelKey, baseUrlKey, DEFAULT_PROVIDER, PROVIDERS} from '$lib/ai/providers';
import {DEFAULT_CSV_PRESETS} from '$lib/csv/csv';
import {THEME_OPTIONS, DEFAULT_THEME} from '$lib/themes';

export {settings};

// Section/setting `label` and `description` hold i18n message keys; SettingsDialog resolves them via
// t() at render time so they react to a language switch. Language option labels stay endonyms.

settings.registerSection({
    id: 'general',
    label: 'settings.section.general',
    order: 0
});
settings.registerSection({
    id: 'editor',
    label: 'settings.section.editor',
    order: 1
});
settings.registerSection({
    id: 'appearance',
    label: 'settings.section.appearance',
    order: 2
});

settings.register('general',
    {
        key: 'general.language',
        type: 'string',
        default: 'en',
        label: 'settings.general.language.label',
        // Driven by the i18n source of truth: adding a Locale here makes it selectable automatically.
        options: LOCALES.map(l => ({value: l, label: ENDONYMS[l]}))
    }
);
settings.register('general',
    {
        key: 'general.nestedFolders',
        type: 'boolean',
        default: false,
        label: 'settings.general.nestedFolders.label',
        description: 'settings.general.nestedFolders.description'
    }
);
settings.register('general',
    {
        key: 'general.checkUpdatesOnStartup',
        type: 'boolean',
        default: true,
        label: 'settings.general.checkUpdates.label',
        description: 'settings.general.checkUpdates.description'
    }
);

settings.register('editor',
    {
        key: 'editor.autosave',
        type: 'boolean',
        default: false,
        label: 'settings.editor.autosave.label'
    }
);
settings.register('editor',
    {
        key: 'editor.autosave_delay',
        type: 'int',
        default: 1000,
        label: 'settings.editor.autosaveDelay.label',
        min: 200,
        max: 10000,
        step: 100
    }
);
settings.register('appearance',
    {
        key: 'appearance.theme',
        type: 'string',
        default: DEFAULT_THEME,
        label: 'settings.appearance.theme.label',
        options: THEME_OPTIONS,
        localizeOptions: true
    }
);
settings.register('appearance',
    {
        key: 'appearance.font_size',
        type: 'int',
        default: 14,
        label: 'settings.appearance.fontSize.label',
        min: 10,
        max: 24
    }
);

settings.registerSection({
    id: 'caching',
    label: 'settings.section.caching',
    order: 3
});
settings.register('caching',
    {
        key: 'cache.photo',
        type: 'boolean',
        default: false,
        label: 'settings.caching.photo.label',
        description: 'settings.caching.photo.description'
    }
);
settings.register('caching',
    {
        key: 'cache.smallThumbnails',
        type: 'boolean',
        default: false,
        label: 'settings.caching.smallThumbnails.label',
        description: 'settings.caching.smallThumbnails.description'
    }
);
settings.register('caching',
    {
        key: 'cache.lazy',
        type: 'boolean',
        default: false,
        label: 'settings.caching.lazy.label',
        description: 'settings.caching.lazy.description'
    }
);

// AI attribution: custom section pages own their UI; the registered keys persist via the store (component XOR fields).
settings.registerSection({
    id: 'ai',
    label: 'settings.section.ai',
    order: 4,
    component: AiSettingsPage
});
settings.registerSection({
    id: 'model-profiles',
    label: 'settings.section.modelProfiles',
    order: 5,
    component: ModelProfilesPage
});
settings.register('ai', {
    key: 'ai.provider',
    type: 'string',
    default: DEFAULT_PROVIDER,
    label: 'settings.ai.provider'
});
// Each provider keeps its own address and active model, so switching providers loses nothing. Ollama's keys keep their original names.
for (const p of PROVIDERS) {
    const urlKey = baseUrlKey(p.id);
    if (urlKey) {
        settings.register('ai', {key: urlKey, type: 'string', default: p.defaultBaseUrl ?? '', label: 'settings.ai.baseUrl'});
    }
    settings.register('ai', {key: activeModelKey(p.id), type: 'string', default: '', label: 'settings.ai.activeModel'});
}
settings.register('ai', {
    key: 'ai.responseFormat',
    type: 'string',
    default: DEFAULT_RESPONSE_FORMAT,
    label: 'settings.ai.responseFormat'
});
settings.register('ai', {
    key: 'ai.answerFormatLabel',
    type: 'string',
    default: 'Answer format:',
    label: 'settings.ai.answerFormatLabel'
});
settings.register('model-profiles', {
    key: 'ai.modelProfiles',
    type: 'custom',
    default: DEFAULT_MODEL_PROFILES,
    label: 'settings.section.modelProfiles'
});
// Settings saved before providers existed lived under Ollama keys; profiles from then belong to Ollama.
settings.registerMigration('ollama.responseFormat', 'ai.responseFormat');
settings.registerMigration('ollama.answerFormatLabel', 'ai.answerFormatLabel');
settings.registerMigration('ollama.modelProfiles', 'ai.modelProfiles', value =>
    Array.isArray(value) ? value.map((p: ModelProfile) => ({...p, provider: p.provider ?? 'ollama'})) : value
);

// CSV export: custom section page owns its UI; the preset array persists via the store.
settings.registerSection({
    id: 'csv',
    label: 'settings.section.csv',
    order: 6,
    component: CsvPresetsPage
});
settings.register('csv', {
    key: 'csv.presets',
    type: 'custom',
    default: DEFAULT_CSV_PRESETS,
    label: 'settings.section.csv'
});

// MCP server: its settings live in the backend (mcp.json), so the page registers no keys and the settings tools cannot reach them.
settings.registerSection({
    id: 'mcp',
    label: 'settings.section.mcp',
    order: 7,
    component: McpSettingsPage
});

settings.registerSection({
    id: 'shortcuts',
    label: 'settings.section.shortcuts',
    order: 99,
    component: ShortcutsPage
});
