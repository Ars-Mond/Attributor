<script lang="ts">
    import ConfirmDialog from './ConfirmDialog.svelte';
    import type {DialogButton} from '../types';

    let {
        filename,
        onDiscard,
        onSave,
        onCancel,
    }: {
        filename: string;
        onDiscard: () => void;
        onSave: () => void | Promise<void>;
        onCancel: () => void;
    } = $props();

    const buttons: DialogButton[] = [
        {
            label: 'Cancel',
            onClick: onCancel,
        },
        {
            label: 'Discard',
            onClick: onDiscard,
            border: 'var(--required-color)',
            color: 'var(--required-color)',
            hoverBg: 'var(--required-alpha-08)',
            hoverBorder: 'var(--required-color)',
            hoverColor: 'var(--required-color)',
        },
        {
            label: 'Save',
            onClick: onSave,
            bg: 'var(--accent)',
            color: '#fff',
            border: 'var(--accent)',
            hoverBg: 'var(--accent-hover)',
            hoverBorder: 'var(--accent-hover)',
            activeBg: 'var(--accent-active)',
            activeBorder: 'var(--accent-active)',
        },
    ];
</script>

<ConfirmDialog
    title="Unsaved Changes"
    body="{filename} has unsaved changes."
    icon="warning"
    {buttons}
    onClose={onCancel}
/>
