<script lang="ts">
    import {onMount, onDestroy} from 'svelte';
    import MarkdownPopup from '$reusable/MarkdownPopup.svelte';
    import {shortcuts} from '$lib/shortcuts';

    let {onClose}: {onClose: () => void} = $props();

    let source = $state('');

    onMount(async () => {
        shortcuts.activateLayer('dialog');
        try {
            const res = await fetch('/Help.md');
            source = await res.text();
        } catch {
            source = '_Failed to load help content._';
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
            label: 'Close',
            onClick: onClose,
            bg: 'var(--accent)',
            color: '#fff',
            border: 'var(--accent)',
            hoverBg: 'var(--accent-hover)',
            hoverBorder: 'var(--accent-hover)',
        },
    ]}
/>
