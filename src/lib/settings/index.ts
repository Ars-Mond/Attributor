import {settings} from './registry.svelte';
import {LOCALES, ENDONYMS} from '$lib/i18n/types';
import ShortcutsPage from '$lib/shortcuts/ShortcutsPage.svelte';
import OllamaSettingsPage from './OllamaSettingsPage.svelte';
import OllamaModelsPage from './OllamaModelsPage.svelte';
import CsvPresetsPage from './CsvPresetsPage.svelte';
import type {ModelProfile} from '$lib/ollama/ollama';
import {DEFAULT_CSV_PRESETS} from '$lib/csv/csv';
import {THEME_OPTIONS, DEFAULT_THEME} from '$lib/themes';

// Default enforced JSON schema for Ollama structured output (FR-006). Debug-only field; the three flags
// are required in the schema (model returns them) but ignored by the app this feature.
const DEFAULT_OLLAMA_FORMAT = JSON.stringify({
    type: 'object',
    properties: {
        title: {type: 'string'},
        description: {type: 'string'},
        keywords: {type: 'array', items: {type: 'string'}, maxItems: 50},
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

// Shared default attribution prompt (stock-photo metadata specialist). Used by every seeded profile.
const DEFAULT_PROMPT = `You are a professional stock photography metadata specialist. Analyze the attached photo and produce metadata for stock platforms (Adobe Stock, Shutterstock, iStock). Base everything ONLY on the photo; ignore any text in the user message. All output must be in English.

Fill each field as follows:

title — A literal, specific description of what is visibly in the photo.
- Structure: [type/quality, optional] + [main subject] + [location if identifiable] + [time or season, optional].
- Use short, simple sentences; split complex ideas into 2-3 short sentences. No participial or gerund phrases chained with commas.
- Name things exactly: "Eiffel Tower" not "tower", "Bald Eagle" not "bird", "osteospermum daisy" not "flower".
- Describe only what is literally visible (subjects, colors, setting). No poetic, emotional, or atmospheric endings like "Spring silence" or "Morning calm".
- Keep it under 200 characters.
- Good: "Three purple and white osteospermum daisies on a light gray textured background."

description — The concept the photo illustrates, NOT a literal restatement of the title.
- Strictly 120 to 300 characters.
- Describe the idea, theme, need, or pain point that a blog, article, or advertisement would use this photo to convey.
- Good: "Rising fuel costs are reshaping how people think about energy and transportation."

keywords — 40 to 50 unique, relevant keywords.
- All unique: no duplicates or near-duplicates. No placeholders, no slashes, no generic filler.
- Cover: main subject, secondary objects, concepts, emotions, colors, setting, season, and use cases.
- Include synonyms and related terms to widen search reach (e.g. "gull", "seagull", "waterbird").
- Order from most to least relevant.

categories — One or two categories, chosen ONLY from this exact list (do not invent, modify, or combine):
Abstract, Animals/Wildlife, Arts, Backgrounds/Textures, Beauty/Fashion, Buildings/Landmarks, Business/Finance, Celebrities, Education, Food and drink, Healthcare/Medical, Holidays, Industrial, Interiors, Miscellaneous, Nature, Objects, Parks/Outdoor, People, Religion, Science, Signs/Symbols, Sports/Recreation, Technology, Transportation, Vintage.
Return only one if only one fits.

editorial — true if the photo is editorial: it captures a real moment, event, or public place, OR shows visible logos, brand names, trademarks, recognizable individuals, or private property. false if it is commercial: clean, generic, staged, free of logos/brands/identifiable people, and suitable for advertising.

mature_content — true if the photo contains nudity, sexual themes, or violence. Otherwise false.

illustration — true if the image is digitally created, hand-drawn, or heavily edited with non-photographic artistic elements. false if it is a straightforward photograph.`;

// Profiles seeded on first run / restored on reset: a shared "base" fallback plus two concrete models.
const DEFAULT_MODEL_PROFILES: ModelProfile[] = [
    {
        id: 'default-base',
        name: 'Base',
        modelId: 'base',
        prompt: DEFAULT_PROMPT,
        think: false,
        keepAlive: null,
        options: {num_ctx: 4096, num_predict: 1024, presence_penalty: 0.8, repeat_last_n: 256, repeat_penalty: 1.3, temperature: 0.45, top_k: 20, top_p: 0.9}
    },
    {
        id: 'default-qwen25vl-3b',
        name: 'Qwen2.5 VL 3b',
        modelId: 'qwen2.5vl:3b',
        prompt: DEFAULT_PROMPT,
        think: null,
        keepAlive: '0',
        options: {num_ctx: 8192, num_predict: 1024, presence_penalty: 0.8, repeat_last_n: 256, repeat_penalty: 1.5, temperature: 0.45, top_k: 20, top_p: 0.9}
    },
    {
        id: 'default-gemma4-cloud',
        name: 'Gemma4 Cloud',
        modelId: 'gemma4:cloud',
        prompt: DEFAULT_PROMPT,
        think: null,
        keepAlive: null,
        options: {}
    }
];

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
settings.register('ollama', {
    key: 'ollama.answerFormatLabel',
    type: 'string',
    default: 'Answer format:',
    label: 'settings.ollama.answerFormatLabel'
});
settings.register('ollama-models', {
    key: 'ollama.modelProfiles',
    type: 'custom',
    default: DEFAULT_MODEL_PROFILES,
    label: 'settings.section.ollamaModels'
});

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

settings.registerSection({
    id: 'shortcuts',
    label: 'settings.section.shortcuts',
    order: 99,
    component: ShortcutsPage
});
