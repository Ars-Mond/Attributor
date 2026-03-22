<script lang="ts">
    import '../styles/global.scss';
    import {attachConsole, error as logError} from '@tauri-apps/plugin-log';
    import {onMount, onDestroy} from 'svelte';

    let { children } = $props();

    let detach: (() => void) | null = null;

    function handleError(e: ErrorEvent) {
        logError(`[uncaught] ${e.message} at ${e.filename}:${e.lineno}:${e.colno}`);
    }

    function handleRejection(e: PromiseRejectionEvent) {
        logError(`[unhandled rejection] ${e.reason}`);
    }

    onMount(async () => {
        detach = await attachConsole();
        window.addEventListener('error', handleError);
        window.addEventListener('unhandledrejection', handleRejection);
    });

    onDestroy(() => {
        detach?.();
        window.removeEventListener('error', handleError);
        window.removeEventListener('unhandledrejection', handleRejection);
    });

    function blockContextMenu(e: MouseEvent) {
        e.preventDefault();
    }
</script>

<svelte:document oncontextmenu={blockContextMenu} />

{@render children()}
