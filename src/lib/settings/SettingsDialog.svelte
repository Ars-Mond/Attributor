<script lang="ts">
    import {onDestroy} from 'svelte';
    import {settings} from './index';
    import type {SettingDescriptor} from './types';
    import type {SettingsSection} from './SettingsSection';

    let {open, onClose}: {open: boolean; onClose: () => void} = $props();

    const allSections = settings.getAllSections();
    let activeSectionId = $state(allSections[0]?.id ?? '');
    const activeSection = $derived<SettingsSection | undefined>(
        allSections.find(s => s.id === activeSectionId) ?? allSections[0]
    );

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') onClose();
    }

    // ── Int stepper ───────────────────────────────────────────────────────

    let holdTimeout: ReturnType<typeof setTimeout> | null = null;
    let nextTickId: ReturnType<typeof setTimeout> | null = null;

    function stopStepper() {
        if (holdTimeout !== null) { clearTimeout(holdTimeout); holdTimeout = null; }
        if (nextTickId !== null) { clearTimeout(nextTickId); nextTickId = null; }
    }

    function applyStep(key: string, dir: 1 | -1, d: SettingDescriptor) {
        const step = (d.step ?? 1) * dir;
        const current = settings.get<number>(key);
        let next = current + step;
        if (d.min !== undefined) next = Math.max(d.min, next);
        if (d.max !== undefined) next = Math.min(d.max, next);
        settings.set(key, next);
    }

    function startStepper(key: string, dir: 1 | -1, d: SettingDescriptor) {
        stopStepper();
        applyStep(key, dir, d);
        holdTimeout = setTimeout(() => {
            holdTimeout = null;
            const autoStart = Date.now();
            function tick() {
                applyStep(key, dir, d);
                const elapsed = Date.now() - autoStart;
                const progress = Math.min(elapsed / 5000, 1);
                // Interval decreases linearly from 1000ms to 200ms over 5 seconds
                const interval = Math.round(1000 - progress * 800);
                nextTickId = setTimeout(tick, interval);
            }
            // First auto-repeat fires 1 second after hold is detected
            nextTickId = setTimeout(tick, 1000);
        }, 300);
    }

    onDestroy(stopStepper);
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="overlay" role="presentation" onclick={onClose} onkeydown={() => {}}>
        <div
            class="dialog"
            role="dialog"
            aria-modal="true"
            aria-label="Settings"
            tabindex="-1"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.key !== 'Escape' && e.stopPropagation()}
        >
            <div class="dlg-header">
                <span class="dlg-title">Settings</span>
                <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>
            </div>

            <div class="dlg-body">
                <nav class="sidebar">
                    {#each allSections as section}
                        <button
                            class="section-btn"
                            class:active={activeSectionId === section.id}
                            onclick={() => { activeSectionId = section.id; }}
                        >{section.label}</button>
                    {/each}
                </nav>

                <div class="fields-panel">
                    {#if activeSection}
                        {#if activeSection.component}
                            {@const SectionComp = activeSection.component}
                            <SectionComp
                                section={activeSection}
                                resetSection={() => settings.resetSection(activeSection.id)}
                            />
                        {/if}

                        {#each activeSection.fields as descriptor (descriptor.key || descriptor.label)}
                            {#if descriptor.type === 'custom' && descriptor.render}
                                {@const FieldComp = descriptor.render}
                                <FieldComp
                                    {descriptor}
                                    get={() => settings.get(descriptor.key)}
                                    set={(v) => settings.set(descriptor.key, v)}
                                    resetToDefault={() => settings.reset(descriptor.key)}
                                />
                            {:else}
                                <div class="field">
                                    {#if descriptor.type === 'boolean'}
                                        <label class="field-label field-label--inline">
                                            <input
                                                type="checkbox"
                                                class="field-checkbox"
                                                checked={settings.get<boolean>(descriptor.key)}
                                                onchange={(e) => settings.set(descriptor.key, e.currentTarget.checked)}
                                            />
                                            {descriptor.label}
                                        </label>
                                    {:else}
                                        <label class="field-label" for="setting-{descriptor.key}">
                                            {descriptor.label}
                                        </label>

                                        {#if descriptor.type === 'int'}
                                            <div class="stepper">
                                                <button
                                                    class="step-btn"
                                                    aria-label="Decrease"
                                                    onpointerdown={() => startStepper(descriptor.key, -1, descriptor)}
                                                    onpointerup={stopStepper}
                                                    onpointerleave={stopStepper}
                                                    onpointercancel={stopStepper}
                                                >−</button>
                                                <input
                                                    id="setting-{descriptor.key}"
                                                    type="number"
                                                    class="field-input stepper-input"
                                                    value={settings.get<number>(descriptor.key)}
                                                    min={descriptor.min}
                                                    max={descriptor.max}
                                                    step={descriptor.step ?? 1}
                                                    onchange={(e) => {
                                                        const v = parseInt(e.currentTarget.value, 10);
                                                        if (!isNaN(v)) {
                                                            let clamped = v;
                                                            if (descriptor.min !== undefined) clamped = Math.max(descriptor.min, clamped);
                                                            if (descriptor.max !== undefined) clamped = Math.min(descriptor.max, clamped);
                                                            settings.set(descriptor.key, clamped);
                                                        }
                                                    }}
                                                />
                                                <button
                                                    class="step-btn"
                                                    aria-label="Increase"
                                                    onpointerdown={() => startStepper(descriptor.key, 1, descriptor)}
                                                    onpointerup={stopStepper}
                                                    onpointerleave={stopStepper}
                                                    onpointercancel={stopStepper}
                                                >+</button>
                                            </div>

                                        {:else if descriptor.type === 'float'}
                                            <input
                                                id="setting-{descriptor.key}"
                                                type="number"
                                                class="field-input"
                                                value={settings.get<number>(descriptor.key)}
                                                min={descriptor.min}
                                                max={descriptor.max}
                                                step={descriptor.step ?? 0.1}
                                                onchange={(e) => {
                                                    const v = parseFloat(e.currentTarget.value);
                                                    if (!isNaN(v)) settings.set(descriptor.key, v);
                                                }}
                                            />

                                        {:else if descriptor.options}
                                            <select
                                                id="setting-{descriptor.key}"
                                                class="field-input field-select"
                                                onchange={(e) => settings.set(descriptor.key, e.currentTarget.value)}
                                            >
                                                {#each descriptor.options as opt}
                                                    <option
                                                        value={opt.value}
                                                        selected={opt.value === settings.get<string>(descriptor.key)}
                                                    >{opt.label}</option>
                                                {/each}
                                            </select>

                                        {:else}
                                            <input
                                                id="setting-{descriptor.key}"
                                                type="text"
                                                class="field-input"
                                                value={settings.get<string>(descriptor.key)}
                                                oninput={(e) => settings.set(descriptor.key, e.currentTarget.value)}
                                            />
                                        {/if}
                                    {/if}

                                    {#if descriptor.description}
                                        <p class="field-desc">{descriptor.description}</p>
                                    {/if}
                                </div>
                            {/if}
                        {/each}
                    {/if}
                </div>
            </div>

            <div class="dlg-footer">
                <button
                    class="reset-btn"
                    onclick={() => settings.resetSection(activeSection?.id ?? '')}
                >Reset to defaults</button>
            </div>
        </div>
    </div>
{/if}

<style lang="scss">
    @use 'styles/mixins' as *;

    .overlay {
        position: fixed;
        inset: 0;
        background: var(--overlay-bg);
        backdrop-filter: blur(3px);
        @include flex(row, center, center);
        z-index: 500;
    }

    .dialog {
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        width: 860px;
        max-width: calc(100vw - 48px);
        height: 90vh;
        max-height: 90vh;
        @include flex(column, flex-start, stretch);
        box-shadow: 0 12px 40px var(--shadow-heavy);
        overflow: hidden;
    }

    // ── Header ──

    .dlg-header {
        @include flex(row, space-between, center);
        padding: 14px 18px;
        border-bottom: 1px solid $border;
        flex-shrink: 0;
    }

    .dlg-title {
        font-size: $fs-regular;
        font-weight: 600;
        color: $text;
    }

    .close-btn {
        @include btn-reset;
        @include transition(color);
        color: $text-muted;
        font-size: $fs-small;
        width: 24px;
        height: 24px;
        @include flex(row, center, center);
        border-radius: $radius-sm;

        &:hover {
            color: $text;
        }
    }

    // ── Body ──

    .dlg-body {
        @include flex(row, flex-start, stretch);
        flex: 1;
        min-height: 0;
        overflow: hidden;
    }

    // ── Sidebar ──

    .sidebar {
        width: 140px;
        flex-shrink: 0;
        @include flex(column, flex-start, stretch);
        padding: 8px;
        border-right: 1px solid $border;
        background: $bg-surface;
        gap: 2px;
        overflow-y: auto;
        @include scrollbar;
    }

    .section-btn {
        @include btn-reset;
        @include transition(background, color);
        display: block;
        width: 100%;
        text-align: left;
        padding: 7px 10px;
        border-radius: $radius-sm;
        font-size: $fs-small;
        color: $text-secondary;

        &:hover {
            background: var(--hover-bg);
            color: $text;
        }

        &.active {
            background: var(--hover-bg-strong);
            color: $text;
            font-weight: 500;
        }
    }

    // ── Fields panel ──

    .fields-panel {
        flex: 1;
        min-width: 0;
        padding: 16px 20px;
        overflow-y: auto;
        @include flex(column, flex-start, stretch);
        gap: 0;
        @include scrollbar;
    }

    .field {
        @include flex(column, flex-start, stretch);
        gap: 6px;
        padding: 14px 0;
        border-bottom: 1px solid $border;

        &:first-child {
            padding-top: 2px;
        }

        &:last-child {
            border-bottom: none;
        }
    }

    // ── Labels ──

    .field-label {
        font-size: $fs-small;
        font-weight: 500;
        color: $text;

        &--inline {
            @include flex(row, flex-start, center);
            gap: 8px;
            cursor: pointer;
            font-weight: 400;
        }
    }

    .field-desc {
        font-size: $fs-footnote1;
        color: $text-muted;
        line-height: 1.5;
    }

    // ── Inputs ──

    .field-input {
        background: $bg-input;
        border: 1px solid $border;
        border-radius: $radius-sm;
        color: $text;
        font-size: $fs-small;
        font-family: $font-base;
        padding: 5px 8px;
        @include transition(border-color, background);
        align-self: flex-start;

        &:focus {
            outline: none;
            border-color: $border-focus;
            background: $bg-input-focus;
        }
    }

    .field-select {
        cursor: pointer;
        padding-right: 24px;
        appearance: none;
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%23888' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: right 8px center;
    }

    .field-checkbox {
        width: 14px;
        height: 14px;
        flex-shrink: 0;
        cursor: pointer;
        accent-color: $accent;
    }

    // ── Int stepper ──

    .stepper {
        @include flex(row, flex-start, center);
        gap: 4px;
    }

    .stepper-input {
        width: 80px;
        text-align: center;

        // Hide native spinners — we have custom +/- buttons
        &::-webkit-inner-spin-button,
        &::-webkit-outer-spin-button {
            -webkit-appearance: none;
        }
        appearance: textfield;
        -moz-appearance: textfield;
    }

    .step-btn {
        @include btn-reset;
        @include flex(row, center, center);
        @include transition(background, color, border-color);
        width: 26px;
        height: 26px;
        flex-shrink: 0;
        border: 1px solid $border;
        border-radius: $radius-sm;
        background: $bg-surface;
        color: $text-secondary;
        font-size: $fs-regular;
        line-height: 1;
        user-select: none;
        -webkit-user-select: none;

        &:hover {
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }
    }

    // ── Footer ──

    .dlg-footer {
        @include flex(row, flex-end, center);
        padding: 10px 18px;
        border-top: 1px solid $border;
        background: $bg-surface;
        flex-shrink: 0;
    }

    .reset-btn {
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
