<script lang="ts">
    interface Props {
        imageSrc: string | null;
        goneMessage: string | null;
        onDismissGone: () => void;
    }

    let {imageSrc, goneMessage, onDismissGone}: Props = $props();
</script>

<div class="viewer">
    {#if imageSrc}
        <img class="image" src={imageSrc} alt="Preview" />
    {:else}
        <div class="placeholder">
            <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <circle cx="8.5" cy="8.5" r="1.5" />
                <path d="M21 15l-5-5L5 21" />
            </svg>
            <p>No image open</p>
        </div>
    {/if}

    {#if goneMessage}
        <div class="gone-toast" role="alert">
            <svg class="gone-icon" viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd"/>
            </svg>
            <span>
                <strong>{goneMessage}</strong> was moved or deleted externally.
            </span>
            <button class="gone-close" onclick={onDismissGone} aria-label="Dismiss">×</button>
        </div>
    {/if}
</div>

<style lang="scss">
    @use '../styles/mixins' as *;

    .viewer {
        @include flex(row, center, center, 1);
        overflow: hidden;
        background: $bg-app;
        min-width: 0;
        min-height: 0;
        position: relative;
    }

    .image {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        display: block;
    }

    .placeholder {
        @include flex(column, center, center);
        gap: 12px;
        color: $text-muted;

        svg {opacity: 0.4;}
        p {font-size: $fs-regular;}
    }

    .gone-toast {
        position: absolute;
        bottom: 20px;
        left: 50%;
        transform: translateX(-50%);
        @include flex(row, flex-start, center);
        gap: 10px;
        padding: 10px 14px;
        background: #2a1f10;
        border: 1px solid #92400e;
        border-radius: $radius-md;
        color: #fcd34d;
        font-size: $fs-small;
        max-width: 420px;
        width: max-content;
        box-shadow: 0 4px 20px var(--shadow);
        animation: toast-in 0.2s ease;

        strong {color: #fde68a;}
    }

    .gone-icon {
        width: 18px;
        height: 18px;
        flex-shrink: 0;
        color: #f59e0b;
    }

    .gone-close {
        @include btn-reset;
        margin-left: auto;
        padding: 0 2px;
        font-size: 18px;
        line-height: 1;
        color: #92400e;
        opacity: 0.7;
        flex-shrink: 0;
        @include transition(opacity);

        &:hover {opacity: 1;}
    }

    @keyframes toast-in {
        from {opacity: 0; transform: translateX(-50%) translateY(8px);}
        to {opacity: 1; transform: translateX(-50%) translateY(0);}
    }
</style>
