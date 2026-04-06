import {settings} from './registry.svelte';

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
