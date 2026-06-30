<script lang="ts">
    import {onMount, onDestroy} from 'svelte';
    import MarkdownPopup from '$reusable/MarkdownPopup.svelte';
    import {shortcuts} from '$lib/shortcuts';
    import {t, locale} from '$lib/i18n';

    let {onClose}: {onClose: () => void} = $props();

    let source = $state('');

    onMount(async () => {
        shortcuts.activateLayer('dialog');
        try {
            let res = await fetch(`/Help.${locale()}.md`);
            if (!res.ok) res = await fetch('/Help.en.md');
            source = await res.text();
        } catch {
            source = t('dialog.help.loadError');
        }
    });

    onDestroy(() => {
        shortcuts.deactivateLayer('dialog');
    });
</script>

<MarkdownPopup
    {source}
    width={860}
    height="90%"
    {onClose}
    buttons={[
        {
            label: t('common.close'),
            onClick: onClose,
            bg: 'var(--accent)',
            color: '#fff',
            border: 'var(--accent)',
            hoverBg: 'var(--accent-hover)',
            hoverBorder: 'var(--accent-hover)',
        },
    ]}
/>
