import {settings} from './registry.svelte';
import {LOCALES, ENDONYMS} from '$lib/i18n/types';
import ShortcutsPage from '$lib/shortcuts/ShortcutsPage.svelte';
import OllamaSettingsPage from './OllamaSettingsPage.svelte';
import OllamaModelsPage from './OllamaModelsPage.svelte';

// Default enforced JSON schema for Ollama structured output (FR-006). Debug-only field; the three flags
// are required in the schema (model returns them) but ignored by the app this feature.
const DEFAULT_OLLAMA_FORMAT = JSON.stringify({
    type: 'object',
    properties: {
        title: {type: 'string'},
        description: {type: 'string'},
        keywords: {type: 'array', items: {type: 'string'}},
        categories: {
            type: 'array',
            items: {
                type: 'string',
                enum: ['Abstract', 'Animals/Wildlife', 'Arts', 'Backgrounds/Textures', 'Beauty/Fashion', 'Buildings/Landmarks', 'Business/Finance', 'Celebrities', 'Education', 'Food and drink', 'Healthcare/Medical', 'Holidays', 'Industrial', 'Interiors', 'Miscellaneous', 'Nature', 'Objects', 'Parks/Outdoor', 'People', 'Religion', 'Science', 'Signs/Symbols', 'Sports/Recreation', 'Technology', 'Transportation', 'Vintage']
            },
            maxItems: 2
        },
        editorial: {type: 'boolean'},
        mature_content: {type: 'boolean'},
        illustration: {type: 'boolean'}
    },
    required: ['title', 'description', 'keywords', 'categories', 'editorial', 'mature_content', 'illustration']
}, null, 2);

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
settings.register('editor',
    {
        key: 'editor.keyword_limit',
        type: 'int',
        default: 50,
        label: 'settings.editor.keywordLimit.label',
        min: 1,
        max: 200
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

// Ollama: custom section pages own their UI; the registered keys persist via the store (component XOR fields).
settings.registerSection({
    id: 'ollama',
    label: 'settings.section.ollama',
    order: 4,
    component: OllamaSettingsPage
});
settings.registerSection({
    id: 'ollama-models',
    label: 'settings.section.ollamaModels',
    order: 5,
    component: OllamaModelsPage
});
settings.register('ollama', {
    key: 'ollama.baseUrl',
    type: 'string',
    default: 'http://localhost:11434',
    label: 'settings.ollama.baseUrl'
});
settings.register('ollama', {
    key: 'ollama.activeModel',
    type: 'string',
    default: '',
    label: 'settings.ollama.activeModel'
});
settings.register('ollama', {
    key: 'ollama.responseFormat',
    type: 'string',
    default: DEFAULT_OLLAMA_FORMAT,
    label: 'settings.ollama.responseFormat'
});
settings.register('ollama-models', {
    key: 'ollama.modelProfiles',
    type: 'custom',
    default: [],
    label: 'settings.section.ollamaModels'
});

settings.registerSection({
    id: 'shortcuts',
    label: 'settings.section.shortcuts',
    order: 99,
    component: ShortcutsPage
});
