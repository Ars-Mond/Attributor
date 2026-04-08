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

shortcuts.registerAction({
    id: 'file.open_folder',
    label: 'Open Folder',
    section: 'File',
    defaultBinding: 'Ctrl+O',
    handler: () => {}
});
shortcuts.registerAction({
    id: 'file.settings',
    label: 'Settings',
    section: 'File',
    defaultBinding: 'Ctrl+,',
    handler: () => {}
});
shortcuts.registerAction({
    id: 'editor.save',
    label: 'Save',
    section: 'Editor',
    defaultBinding: 'Ctrl+S',
    handler: () => {}
});
shortcuts.registerAction({
    id: 'editor.copy_kw',
    label: 'Copy Keywords',
    section: 'Editor',
    defaultBinding: 'Ctrl+Shift+C',
    handler: () => {}
});
shortcuts.registerAction({
    id: 'editor.paste_kw',
    label: 'Paste Keywords',
    section: 'Editor',
    defaultBinding: 'Ctrl+Shift+V',
    handler: () => {}
});
