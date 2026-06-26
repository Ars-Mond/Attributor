import {shortcuts} from './registry.svelte';

export {shortcuts};


shortcuts.registerLayer({
    id: 'global',
    priority: 0,
    autoActivate: () => true
});
shortcuts.registerLayer({
    id: 'editor',
    priority: 10
});
shortcuts.registerLayer({
    id: 'input',
    priority: 50,
    suppressBelow: true,
    autoActivate: () => ['INPUT', 'TEXTAREA'].includes(document.activeElement?.tagName ?? ''),
});
shortcuts.registerLayer({
    id: 'dialog',
    priority: 100,
    suppressBelow: true
});

// `label`/`section` hold i18n message keys; ShortcutsPage resolves them via t() at render time.
shortcuts.registerAction({
    id: 'file.open_folder',
    label: 'shortcuts.action.openFolder',
    section: 'shortcuts.section.file',
    defaultBinding: 'Ctrl+O',
    handler: () => {}
});
shortcuts.registerAction({
    id: 'file.settings',
    label: 'shortcuts.action.settings',
    section: 'shortcuts.section.file',
    defaultBinding: 'Ctrl+,',
    handler: () => {}
});
shortcuts.registerAction({
    id: 'editor.save',
    label: 'shortcuts.action.save',
    section: 'shortcuts.section.editor',
    defaultBinding: 'Ctrl+S',
    handler: () => {}
});
shortcuts.registerAction({
    id: 'editor.copy_kw',
    label: 'shortcuts.action.copyKeywords',
    section: 'shortcuts.section.editor',
    defaultBinding: 'Ctrl+Shift+C',
    handler: () => {}
});
shortcuts.registerAction({
    id: 'editor.paste_kw',
    label: 'shortcuts.action.pasteKeywords',
    section: 'shortcuts.section.editor',
    defaultBinding: 'Ctrl+Shift+V',
    handler: () => {}
});
