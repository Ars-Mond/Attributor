import {settings} from './registry.svelte';
import ShortcutsPage from '$lib/shortcuts/ShortcutsPage.svelte';

export {settings};

settings.registerSection({
    id: 'general',
    label: 'General',
    order: 0
});
settings.registerSection({
    id: 'editor',
    label: 'Editor',
    order: 1
});
settings.registerSection({
    id: 'appearance',
    label: 'Appearance',
    order: 2
});

settings.register('general',
    {
        key: 'general.language',
        type: 'string',
        default: 'en',
        label: 'Language',
        options: [
            {
                value: 'en',
                label: 'English'
            },
            {
                value: 'ru',
                label: 'Русский'
            }
        ]
    }
);

settings.register('editor',
    {
        key: 'editor.autosave',
        type: 'boolean',
        default: false,
        label: 'Auto-save'
    }
);
settings.register('editor',
    {
        key: 'editor.autosave_delay',
        type: 'int',
        default: 1000,
        label: 'Auto-save delay (ms)',
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
        label: 'Max keywords per file',
        min: 1,
        max: 200
    }
);

settings.register('appearance',
    {
        key: 'appearance.font_size',
        type: 'int',
        default: 14,
        label: 'Font size',
        min: 10,
        max: 24
    }
);

settings.registerSection({
    id: 'caching',
    label: 'Caching',
    order: 3
});
settings.register('caching',
    {
        key: 'cache.photo',
        type: 'boolean',
        default: false,
        label: 'Photo caching',
        description: 'Show the viewed photo via a cached thumbnail instead of the original (off shows the original directly). Note: the first run takes longer while thumbnails are generated; with a large number of photos the app may briefly become unresponsive, and the cache increases the disk space used.'
    }
);
settings.register('caching',
    {
        key: 'cache.smallThumbnails',
        type: 'boolean',
        default: false,
        label: 'Cache small thumbnails',
        description: 'Show list previews via cached small thumbnails (off shows the original directly).'
    }
);
settings.register('caching',
    {
        key: 'cache.lazy',
        type: 'boolean',
        default: false,
        label: 'Lazy caching',
        description: 'Generate thumbnails on display instead of when a folder is opened.'
    }
);
settings.register('caching',
    {
        key: 'cache.currentFolderOnly',
        type: 'boolean',
        default: true,
        label: 'Current folder only',
        description: 'Cache only the current folder; do not recurse into subfolders.'
    }
);

settings.registerSection({
    id: 'shortcuts',
    label: 'Shortcuts',
    order: 99,
    component: ShortcutsPage
});
