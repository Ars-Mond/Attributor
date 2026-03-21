<script lang="ts">
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
</script>

<div
    class="overlay"
    role="presentation"
    onclick={onCancel}
    onkeydown={(e) => e.key === 'Escape' && onCancel()}
>
    <div
        class="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="ucd-title"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
    >
        <p class="dialog-title" id="ucd-title">Unsaved Changes</p>
        <p class="dialog-body">
            <span class="fname">{filename}</span> has unsaved changes.
        </p>
        <div class="actions">
            <button class="btn-ghost" onclick={onCancel}>Cancel</button>
            <button class="btn-ghost btn-discard" onclick={onDiscard}>Discard</button>
            <button class="btn-primary" onclick={onSave}>Save</button>
        </div>
    </div>
</div>

<style lang="scss">
    @use '../styles/mixins' as *;

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
        width: 340px;
        max-width: calc(100vw - 48px);
        @include flex(column, flex-start, stretch);
        gap: 14px;
        box-shadow: 0 12px 40px var(--shadow-heavy);
    }

    .dialog-title {
        font-size: $fs-regular;
        font-weight: 600;
        color: $text;
    }

    .dialog-body {
        font-size: $fs-small;
        color: $text-secondary;
        line-height: 1.5;
    }

    .fname {
        color: $text;
        font-weight: 500;
    }

    .actions {
        @include flex(row, flex-end, center);
        gap: 8px;
        margin-top: 6px;
    }

    .btn-discard {
        border-color: $required-color;
        color: $required-color;

        &:hover {
            background: var(--required-alpha-08);
            color: $required-color;
            border-color: $required-color;
        }
    }
</style>
