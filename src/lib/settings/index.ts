import {settings} from './registry.svelte';

export {settings};

settings.register({key: 'general.language', type: 'string', default: 'en',
    label: 'Language', section: 'General',
    options: [{value: 'en', label: 'English'}, {value: 'ru', label: 'Русский'}]});

settings.register({key: 'editor.autosave', type: 'boolean', default: false,
    label: 'Auto-save', section: 'Editor'});
settings.register({key: 'editor.autosave_delay', type: 'int', default: 1000,
    label: 'Auto-save delay (ms)', section: 'Editor', min: 200, max: 10000, step: 100});
settings.register({key: 'editor.keyword_limit', type: 'int', default: 50,
    label: 'Max keywords per file', section: 'Editor', min: 1, max: 200});

settings.register({key: 'appearance.font_size', type: 'int', default: 14,
    label: 'Font size', section: 'Appearance', min: 10, max: 24});
