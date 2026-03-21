<script lang="ts">
    import '../styles/global.scss';

    let { children } = $props();

    function blockContextMenu(e: MouseEvent) {
        e.preventDefault();
    }
</script>

<svelte:document oncontextmenu={blockContextMenu} />

{@render children()}
