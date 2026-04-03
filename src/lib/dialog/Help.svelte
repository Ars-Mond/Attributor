<script lang="ts">
    import {onMount} from 'svelte';
    import MarkdownPopup from '../reusable/MarkdownPopup.svelte';

    let {onClose}: {onClose: () => void} = $props();

    let source = $state('');

    onMount(async () => {
        try {
            const res = await fetch('/Help.md');
            source = await res.text();
        } catch {
            source = '_Failed to load help content._';
        }
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
