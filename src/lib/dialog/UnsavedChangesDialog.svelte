<script lang="ts">
    import ConfirmDialog from './ConfirmDialog.svelte';
    import type {DialogButton} from '$lib/types';
    import {t} from '$lib/i18n';

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

    const buttons: DialogButton[] = $derived([
        {
            label: t('common.cancel'),
            onClick: () => onCancel(),
        },
        {
            label: t('common.discard'),
            onClick: () => onDiscard(),
            border: 'var(--required-color)',
            color: 'var(--required-color)',
            hoverBg: 'var(--required-alpha-08)',
            hoverBorder: 'var(--required-color)',
            hoverColor: 'var(--required-color)',
        },
        {
            label: t('common.save'),
            onClick: () => onSave(),
            bg: 'var(--accent)',
            color: '#fff',
            border: 'var(--accent)',
            hoverBg: 'var(--accent-hover)',
            hoverBorder: 'var(--accent-hover)',
            activeBg: 'var(--accent-active)',
            activeBorder: 'var(--accent-active)',
        },
    ]);
</script>

<ConfirmDialog
    title={t('dialog.unsavedChanges.title')}
    body={t('dialog.unsavedChanges.body', {filename})}
    icon="warning"
    {buttons}
    onClose={onCancel}
/>
