import {settings} from './registry.svelte';
import {LOCALES, ENDONYMS} from '$lib/i18n/types';
import ShortcutsPage from '$lib/shortcuts/ShortcutsPage.svelte';

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

settings.registerSection({
    id: 'shortcuts',
    label: 'settings.section.shortcuts',
    order: 99,
    component: ShortcutsPage
});
