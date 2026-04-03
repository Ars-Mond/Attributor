<script lang="ts">
    import {onMount} from 'svelte';
    import {getVersion} from '@tauri-apps/api/app';

    let {onClose}: {onClose: () => void} = $props();

    let version = $state('...');

    onMount(async () => {
        version = await getVersion();
    });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
    class="overlay"
    role="presentation"
    onclick={onClose}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
>
    <div
        class="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="about-title"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
    >
        <div class="header">
            <img src="/logo.png" alt="Attributor" class="logo" />
            <div>
                <p class="app-name" id="about-title">Attributor</p>
                <p class="app-version">Version {version}</p>
            </div>
        </div>

        <p class="description">
            Desktop application for editing XMP/EXIF metadata of stock photos.
            Supports batch editing of title, description, keywords, and categories
            directly embedded into JPEG, PNG, and WebP files.
        </p>

        <div class="meta">
            <span class="meta-row">
                <span class="meta-label">Identifier</span>
                <span class="meta-value">loc.am.attributor</span>
            </span>
            <span class="meta-row">
                <span class="meta-label">License</span>
                <span class="meta-value">AGPL-3.0</span>
            </span>
        </div>

        <div class="actions">
            <button class="btn-primary" onclick={onClose}>Close</button>
        </div>
    </div>
</div>

<style lang="scss">
    @use '../../styles/mixins' as *;

    .overlay {
        position: fixed;
        inset: 0;
        background: var(--overlay-bg);
        backdrop-filter: blur(3px);
        @include flex(row, center, center);
        z-index: 200;
    }

    .dialog {
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        padding: 24px;
        width: 360px;
        max-width: calc(100vw - 48px);
        @include flex(column, flex-start, stretch);
        gap: 16px;
        box-shadow: 0 12px 40px var(--shadow-heavy);
    }

    .header {
        @include flex(row, flex-start, center);
        gap: 14px;
    }

    .logo {
        width: 48px;
        height: 48px;
        flex-shrink: 0;
        border-radius: $radius-md;
    }

    .app-name {
        font-size: 18px;
        font-weight: 700;
        color: $text;
        line-height: 1.2;
    }

    .app-version {
        font-size: $fs-small;
        color: $text-muted;
        margin-top: 3px;
    }

    .description {
        font-size: $fs-small;
        color: $text-secondary;
        line-height: 1.6;
        border-top: 1px solid $border;
        padding-top: 14px;
    }

    .meta {
        @include flex(column, flex-start, stretch);
        gap: 6px;
        background: $bg-surface;
        border: 1px solid $border;
        border-radius: $radius-sm;
        padding: 10px 12px;
    }

    .meta-row {
        @include flex(row, flex-start, center);
        gap: 8px;
        font-size: $fs-small;
    }

    .meta-label {
        color: $text-muted;
        min-width: 70px;
        flex-shrink: 0;
    }

    .meta-value {
        color: $text-secondary;
        font-family: monospace;
        font-size: $fs-footnote1;
    }

    .actions {
        @include flex(row, flex-end, center);
        margin-top: 4px;
    }
</style>
