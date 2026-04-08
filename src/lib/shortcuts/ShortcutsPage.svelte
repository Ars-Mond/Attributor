<script lang="ts">
    import {shortcuts, normalizeBinding} from './registry.svelte';
    import {debug} from '@tauri-apps/plugin-log';
    import type {ActionDescriptor} from './types';
    import type {SettingSectionProps} from '$lib/settings/SettingsSection';

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    let {section, resetSection}: SettingSectionProps = $props();

    let listeningId = $state<string | null>(null);
    let conflictFor = $state<string | null>(null);
    let conflictWith = $state<ActionDescriptor | null>(null);
    let pendingBinding = $state<string | null>(null);

    function startListening(actionId: string) {
        debug(`[shortcuts] listening for binding: ${actionId}`);
        listeningId = actionId;
        conflictFor = null;
        conflictWith = null;
        pendingBinding = null;
    }

    function handleCapture(e: KeyboardEvent) {
        if (!listeningId) return;
        e.preventDefault();
        e.stopPropagation();
        if (e.key === 'Escape') {
            debug(`[shortcuts] listening cancelled: ${listeningId}`);
            listeningId = null;
            return;
        }
        const binding = normalizeBinding(e);
        if (!binding) return;
        const conflict = shortcuts.getConflict(binding, listeningId);
        if (conflict) {
            debug(`[shortcuts] binding conflict: ${binding} already used by "${conflict.id}"`);
            conflictFor = listeningId;
            conflictWith = conflict;
            pendingBinding = binding;
            listeningId = null;
        } else {
            shortcuts.setUserBinding(listeningId, binding);
            shortcuts.save();
            listeningId = null;
        }
    }

    $effect(() => {
        if (listeningId) {
            document.addEventListener('keydown', handleCapture, true);
            return () => document.removeEventListener('keydown', handleCapture, true);
        }
    });

    function handleReassign() {
        if (!conflictFor || !conflictWith || !pendingBinding) return;
        debug(`[shortcuts] reassign: clearing "${conflictWith.id}", setting "${conflictFor}" → ${pendingBinding}`);
        shortcuts.setUserBinding(conflictWith.id, null);
        shortcuts.setUserBinding(conflictFor, pendingBinding);
        shortcuts.save();
        conflictFor = null;
        conflictWith = null;
        pendingBinding = null;
    }

    function handleCancelConflict() {
        conflictFor = null;
        conflictWith = null;
        pendingBinding = null;
    }
</script>

<div class="shortcuts-page">
    {#each shortcuts.getSections() as sectionName}
        <div class="shortcut-section">
            <h3 class="section-header">{sectionName}</h3>
            {#each shortcuts.getActionsBySection(sectionName) as action (action.id)}
                {@const effective = shortcuts.getEffectiveBinding(action.id)}
                {@const isModified = shortcuts.getUserBinding(action.id) !== null}
                {@const isListening = listeningId === action.id}
                {@const hasConflict = conflictFor === action.id}

                <div class="shortcut-row">
                    <span class="action-label">{action.label}</span>
                    <button
                        class="binding-btn"
                        class:listening={isListening}
                        class:modified={isModified}
                        onclick={() => startListening(action.id)}
                    >{isListening ? 'Press keys...' : (effective ?? '—')}</button>
                    <button
                        class="reset-action-btn"
                        disabled={!isModified}
                        onclick={() => { shortcuts.setUserBinding(action.id, null); shortcuts.save(); }}
                        aria-label="Reset to default"
                    >↺</button>
                </div>

                {#if hasConflict && conflictWith}
                    <div class="conflict-row">
                        <span class="conflict-msg">Already used by: {conflictWith.label}</span>
                        <button class="conflict-btn" onclick={handleReassign}>Reassign</button>
                        <button class="conflict-btn" onclick={handleCancelConflict}>Cancel</button>
                    </div>
                {/if}
            {/each}
        </div>
    {/each}

    <div class="page-footer">
        <button class="reset-all-btn" onclick={() => { shortcuts.resetAll(); shortcuts.save(); }}>
            Reset all shortcuts
        </button>
    </div>
</div>

<style lang="scss">
    @use 'styles/mixins' as *;

    .shortcuts-page {
        @include flex(column, flex-start, stretch);
        gap: 0;
        width: 100%;
    }

    .shortcut-section {
        @include flex(column, flex-start, stretch);
        gap: 0;
        margin-bottom: 8px;
    }

    .section-header {
        font-size: $fs-footnote1;
        font-weight: 600;
        color: $text-muted;
        text-transform: uppercase;
        letter-spacing: 0.06em;
        margin: 0;
        padding: 10px 0 6px;
        border-bottom: 1px solid $border;
    }

    .shortcut-row {
        @include flex(row, flex-start, center);
        gap: 8px;
        padding: 8px 0;
        border-bottom: 1px solid $border;

        &:last-of-type {
            border-bottom: none;
        }
    }

    .action-label {
        flex: 1;
        font-size: $fs-small;
        color: $text;
    }

    .binding-btn {
        @include btn-reset;
        @include transition(border-color, background, opacity);
        min-width: 140px;
        padding: 5px 10px;
        text-align: center;
        border: 1px solid $border;
        border-radius: $radius-sm;
        background: $bg-input;
        color: $text;
        font-size: $fs-small;
        font-family: $font-base;
        cursor: pointer;

        &:hover {
            border-color: $text-muted;
            background: $bg-input-focus;
        }

        &.listening {
            border-color: $border-focus;
            background: $bg-input-focus;
            color: $text-muted;
            animation: pulse 1s ease-in-out infinite;
            cursor: default;
        }

        &.modified {
            border-color: $accent;
        }
    }

    @keyframes pulse {
        0%, 100% { opacity: 1; }
        50% { opacity: 0.55; }
    }

    .reset-action-btn {
        @include btn-reset;
        @include transition(color, opacity);
        width: 24px;
        height: 24px;
        @include flex(row, center, center);
        font-size: $fs-regular;
        color: $text-muted;
        flex-shrink: 0;

        &:hover:not(:disabled) {
            color: $text;
        }

        &:disabled {
            opacity: 0.3;
            cursor: default;
        }
    }

    .conflict-row {
        @include flex(row, flex-start, center);
        gap: 8px;
        padding: 6px 0 8px;
        border-bottom: 1px solid $border;
    }

    .conflict-msg {
        flex: 1;
        font-size: $fs-footnote1;
        color: $text-muted;
    }

    .conflict-btn {
        @include btn-reset;
        @include transition(background, color, border-color);
        padding: 3px 10px;
        border: 1px solid $border;
        border-radius: $radius-sm;
        font-size: $fs-footnote1;
        font-family: $font-base;
        color: $text-secondary;
        background: transparent;
        cursor: pointer;

        &:hover {
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }
    }

    .page-footer {
        @include flex(row, flex-start, center);
        padding-top: 16px;
        margin-top: 8px;
    }

    .reset-all-btn {
        @include btn-reset;
        @include transition(background, color, border-color);
        padding: 5px 14px;
        border: 1px solid $border;
        border-radius: $radius-sm;
        font-size: $fs-small;
        font-family: $font-base;
        color: $text-secondary;
        background: transparent;
        cursor: pointer;

        &:hover {
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }
    }
</style>
