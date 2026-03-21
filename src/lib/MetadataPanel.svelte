<script lang="ts">
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import {writeText, readText} from "@tauri-apps/plugin-clipboard-manager";
    import KeywordSuggestions from "./KeywordSuggestions.svelte";
    import {loadAppState, saveAppState} from "./store";
    import type {Metadata, ReadResult} from "./types";

    // ── Bindable props ─────────────────────────────────────────────────────

    let {
        isDirty = $bindable(false),
        onPathChange,
    }: {
        isDirty?: boolean;
        onPathChange?: (newPath: string) => void;
    } = $props();

    // ── Form state ─────────────────────────────────────────────────────────

    let filepath = $state("");          // full OS path (internal, not editable)
    let filename = $state("");          // stem only — shown and editable
    let title = $state("");
    let description = $state("");
    let keywordInput = $state("");
    let keywords = $state<string[]>([]);
    let categories = $state("");
    let releaseFilename = $state("");
    let autoSave = $state(false);
    let saveAttempted = $state(false);

    // ── UI preferences (persisted) ─────────────────────────────────────────

    let descriptionEl: HTMLTextAreaElement | undefined;
    let stockKeywordsOpen = $state(false);
    let optionalOpen = $state(false);
    let uiLoaded = $state(false);

    onMount(async () => {
        const s = await loadAppState();
        // Apply height imperatively — avoids reactive style conflicts with browser resize handle
        if (s.descriptionHeight && descriptionEl) {
            descriptionEl.style.height = `${s.descriptionHeight}px`;
        }
        if (s.stockKeywordsOpen !== undefined) stockKeywordsOpen = s.stockKeywordsOpen;
        if (s.optionalOpen !== undefined) optionalOpen = s.optionalOpen;
        uiLoaded = true;
    });

    // Persist textarea height via ResizeObserver — writes directly to store, no reactive state
    $effect(() => {
        if (!descriptionEl) return;
        let saveTimer: ReturnType<typeof setTimeout> | null = null;
        const obs = new ResizeObserver(entries => {
            if (!uiLoaded) return;
            const h = Math.round(entries[0].contentRect.height);
            if (h > 0) {
                if (saveTimer) clearTimeout(saveTimer);
                saveTimer = setTimeout(() => saveAppState({descriptionHeight: h}), 300);
            }
        });
        obs.observe(descriptionEl);
        return () => {
            obs.disconnect();
            if (saveTimer) clearTimeout(saveTimer);
        };
    });

    // Persist spoiler states
    $effect(() => {
        if (!uiLoaded) return;
        saveAppState({stockKeywordsOpen, optionalOpen});
    });

    // ── Snapshot (dirty tracking) ──────────────────────────────────────────
    // Purely in-memory: taken when a file is opened. No files are created.
    // isDirty = current fields ≠ snapshot fields.

    interface Snapshot {
        filename: string;
        title: string;
        description: string;
        keywords: string[];
        categories: string;
        releaseFilename: string;
    }

    let snapshot = $state<Snapshot | null>(null);

    function captureSnapshot(): Snapshot {
        return {
            filename,
            title,
            description,
            keywords: [...keywords],
            categories,
            releaseFilename,
        };
    }

    const isDirtyComputed = $derived(
        snapshot !== null && (
            filename !== snapshot.filename ||
            title !== snapshot.title ||
            description !== snapshot.description ||
            JSON.stringify(keywords) !== JSON.stringify(snapshot.keywords) ||
            categories !== snapshot.categories ||
            releaseFilename !== snapshot.releaseFilename
        )
    );

    $effect(() => {
        isDirty = isDirtyComputed;
    });

    // ── Auto-save ──────────────────────────────────────────────────────────

    $effect(() => {
        // Track all editable fields so the effect re-runs on every keystroke
        filename;
        title;
        description;
        keywords;
        categories;
        releaseFilename;

        if (!autoSave || !isDirtyComputed || hasErrors) return;

        const timer = setTimeout(() => {
            doSave().catch(() => {
            });
        }, 1000);
        return () => clearTimeout(timer);
    });

    // ── File status ────────────────────────────────────────────────────────

    const fileStatus = $derived(
        snapshot === null ? 'none' :
            isDirtyComputed ? 'edit' :
                'open'
    );

    // Display path below the status row
    const displayPath = $derived(filepath);

    // ── Validation ─────────────────────────────────────────────────────────

    const validationErrors = $derived(
        !filepath
            ? ['No file selected']
            : (
                [
                    !filename.trim() && 'Filename is required',
                    !title.trim() && 'Title is required',
                    !description.trim() && 'Description is required',
                    keywords.length === 0 && 'At least one keyword is required',
                ] as (string | false)[]
            ).filter((v): v is string => !!v)
    );

    const hasErrors = $derived(validationErrors.length > 0);

    // ── Exported interface ─────────────────────────────────────────────────

    /** Load a file: reset fields, read existing XMP, then take snapshot. */
    export async function loadFile(path: string): Promise<void> {
        filepath = path;
        filename = extractStem(path);
        title = '';
        description = '';
        keywordInput = '';
        keywords = [];
        categories = '';
        releaseFilename = '';
        saveAttempted = false;
        snapshot = null; // clear dirty state immediately

        try {
            const meta = await invoke<ReadResult>('read_metadata', {path});
            title = meta.title;
            description = meta.description;
            keywords = meta.keywords;
            categories = meta.categories;
            releaseFilename = meta.releaseFilename;
        } catch (e) {
            // File has no XMP yet — leave fields empty
            console.warn('read_metadata failed:', e);
        }

        snapshot = captureSnapshot();
    }

    /** Clear everything — called when the open file is deleted or renamed externally. */
    export function clear(): void {
        filepath = "";
        filename = "";
        title = "";
        description = "";
        keywordInput = "";
        keywords = [];
        categories = "";
        releaseFilename = "";
        snapshot = null;
        saveAttempted = false;
    }

    /** Revert all fields to the last snapshot. */
    export function reset(): void {
        if (!snapshot) return;
        filename = snapshot.filename;
        title = snapshot.title;
        description = snapshot.description;
        keywords = [...snapshot.keywords];
        categories = snapshot.categories;
        releaseFilename = snapshot.releaseFilename;
        saveAttempted = false;
    }

    /**
     * Save metadata (and rename if filename changed).
     * Throws if validation fails. Returns the final file path.
     */
    export async function save(): Promise<string> {
        saveAttempted = true;
        if (hasErrors) throw new Error('Validation failed');
        return await doSave();
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    function extractStem(path: string): string {
        const base = path.replace(/\\/g, '/').split('/').pop() ?? '';
        const dot = base.lastIndexOf('.');
        return dot > 0 ? base.slice(0, dot) : base;
    }

    async function doSave(): Promise<string> {
        const metadata: Metadata = {
            filepath,
            // Strip extension if user accidentally typed it
            filename: filename.trim().replace(/\.[^/.]+$/, ''),
            title,
            description,
            keywords,
            categories,
            releaseFilename,
        };

        const newPath = await invoke<string>('save_metadata', {metadata});

        const prevFilepath = filepath;
        filepath = newPath;
        // Derive new stem from returned path in case Rust sanitized it
        filename = extractStem(newPath);
        snapshot = captureSnapshot();

        if (newPath !== prevFilepath) {
            onPathChange?.(newPath);
        }

        return newPath;
    }

    async function handleSave() {
        saveAttempted = true;
        if (hasErrors) return;
        await doSave();
    }

    // ── Preset keywords ────────────────────────────────────────────────────

    const presets: Record<string, string[]> = {
        Nature: [
            "nature", "landscape", "sky", "water", "forest", "mountain",
            "sunset", "ocean", "river", "flower", "tree", "grass", "cloud",
        ],
        People: [
            "portrait", "woman", "man", "child", "family", "people",
            "lifestyle", "crowd", "face", "smile",
        ],
        Urban: [
            "city", "architecture", "building", "street", "urban",
            "skyline", "road", "bridge",
        ],
        Concepts: [
            "business", "technology", "abstract", "vintage", "minimal",
            "creative", "design", "background", "texture", "pattern",
        ],
        Animals: [
            "dog", "cat", "bird", "wildlife", "animal", "pet", "horse",
        ],
        Seasons: [
            "winter", "summer", "spring", "autumn", "snow", "rain", "fog",
        ],
    };

    // ── Keyword logic ──────────────────────────────────────────────────────

    function addKeyword(word: string) {
        const trimmed = word.trim().toLowerCase();
        if (trimmed && !keywords.includes(trimmed)) keywords = [...keywords, trimmed];
    }

    function removeKeyword(word: string) {
        keywords = keywords.filter((k) => k !== word);
    }

    // ── Keyword suggestions ────────────────────────────────────────────────

    let inputEl: HTMLInputElement | undefined;
    let suggestionsComp: KeywordSuggestions | undefined;

    function handleKeywordKeydown(e: KeyboardEvent) {
        if (suggestionsComp?.handleKeydown(e)) return;
        if (e.key === 'Enter') {
            e.preventDefault();
            addKeyword(keywordInput);
            keywordInput = '';
        }
    }

    function handleKeywordInput() {
        if (keywordInput.includes(', ')) {
            const parts = keywordInput.split(', ');
            for (let i = 0; i < parts.length - 1; i++) addKeyword(parts[i]);
            keywordInput = parts[parts.length - 1];
        }
    }

    // ── Keyword clipboard ──────────────────────────────────────────────────

    async function copyKeywords() {
        if (!keywords.length) return;
        await writeText(keywords.join(', ') + ', ');
    }

    async function pasteKeywords() {
        const text = await readText();
        if (!text) return;
        const parts = text.split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
        for (const word of parts) addKeyword(word);
    }

    // ── Keyword drag & drop (pointer-based, ghost + placeholder) ──────────

    let dragFromIndex = $state<number | null>(null);
    let dropInsert = $state<number | null>(null);  // insertion point in 'without' array
    let ghostWidth = $state(0);
    let chipsEl: HTMLElement | undefined;

    // Chip rects captured once at drag start (in 'without' order, i.e. excluding dragged chip).
    // Using frozen rects prevents the feedback loop that causes row-boundary flickering:
    // placeholder movement changes layout → recalc changes dropInsert → layout changes → loop.
    let frozenRects: DOMRect[] = [];

    // During drag: keywords minus dragged chip, with null placeholder at drop position.
    // Outside drag: same as keywords (null never appears).
    const dragDisplay = $derived.by((): (string | null)[] => {
        if (dragFromIndex === null || dropInsert === null) return keywords;
        const arr: (string | null)[] = keywords.filter((_, i) => i !== dragFromIndex);
        arr.splice(Math.min(dropInsert, arr.length), 0, null);
        return arr;
    });

    function startChipDrag(e: PointerEvent, kwIndex: number) {
        if ((e.target as HTMLElement).closest('.chip-remove')) return;
        if (dragFromIndex !== null || keywords.length < 2) return;
        e.preventDefault();

        const chipEl = e.currentTarget as HTMLElement;
        const rect = chipEl.getBoundingClientRect();
        const ox = e.clientX - rect.left;
        const oy = e.clientY - rect.top;
        ghostWidth = rect.width;

        // Capture chip rects BEFORE state changes (before placeholder appears in DOM).
        // Filter out the dragged chip so indices match the 'without' array.
        if (chipsEl) {
            frozenRects = [...chipsEl.querySelectorAll<HTMLElement>('.chip')]
                .filter((_, i) => i !== kwIndex)
                .map(c => c.getBoundingClientRect());
        }

        // Create ghost clone that follows the cursor
        const ghost = chipEl.cloneNode(true) as HTMLElement;
        Object.assign(ghost.style, {
            position: 'fixed',
            left: `${rect.left}px`,
            top: `${rect.top}px`,
            width: `${rect.width}px`,
            margin: '0',
            pointerEvents: 'none',
            zIndex: '9999',
            cursor: 'grabbing',
            boxShadow: '0 4px 16px rgba(0,0,0,0.55)',
            opacity: '0.9',
        });
        document.body.appendChild(ghost);

        dragFromIndex = kwIndex;
        dropInsert = kwIndex;

        const onMove = (ev: PointerEvent) => {
            ghost.style.left = `${ev.clientX - ox}px`;
            ghost.style.top = `${ev.clientY - oy}px`;
            const next = calcDropInsert(ev.clientX, ev.clientY);
            if (next !== dropInsert) dropInsert = next;
        };

        const onUp = () => {
            ghost.remove();
            if (dropInsert !== null && dragFromIndex !== null && dropInsert !== dragFromIndex) {
                const arr = [...keywords];
                const [moved] = arr.splice(dragFromIndex, 1);  // arr is now 'without'
                arr.splice(dropInsert, 0, moved);
                keywords = arr;
            }
            dragFromIndex = null;
            dropInsert = null;
            ghostWidth = 0;
            frozenRects = [];
            document.removeEventListener('pointermove', onMove);
            document.removeEventListener('pointerup', onUp);
        };

        document.addEventListener('pointermove', onMove);
        document.addEventListener('pointerup', onUp);
    }

    // Returns insertion index in the 'without' array using frozen chip rects.
    // Rows are determined by Y range with midpoint boundary → no oscillation.
    function calcDropInsert(x: number, y: number): number {
        const rects = frozenRects;
        if (!rects.length) return 0;

        const chipH = rects[0].height;

        // Group chips into rows by comparing top values
        const rows: { top: number; bottom: number; indices: number[] }[] = [];
        for (let i = 0; i < rects.length; i++) {
            const r = rects[i];
            const row = rows.find(row => Math.abs(row.top - r.top) < chipH * 0.5);
            if (row) {
                row.indices.push(i);
                row.bottom = Math.max(row.bottom, r.bottom);
            } else {
                rows.push({top: r.top, bottom: r.bottom, indices: [i]});
            }
        }
        rows.sort((a, b) => a.top - b.top);

        // Pick the row: use midpoint between adjacent rows as hard boundary.
        // Because frozenRects never change, these midpoints are stable → no flicker.
        let targetRow = rows[rows.length - 1];
        for (let r = 0; r < rows.length - 1; r++) {
            const midY = (rows[r].bottom + rows[r + 1].top) / 2;
            if (y <= midY) {
                targetRow = rows[r];
                break;
            }
        }

        // Within the row, split by each chip's horizontal midpoint
        for (const i of targetRow.indices) {
            if (x < rects[i].left + rects[i].width / 2) return i;
        }
        return targetRow.indices[targetRow.indices.length - 1] + 1;
    }
</script>

<aside class="panel">
    <div class="panel-content">
        <h2 class="panel-title">Metadata</h2>

        <!-- ── File info ── -->
        <div class="file-info">
            <div class="file-status-row">
                <span class="status-dot status-dot--{fileStatus}"></span>
                <span class="status-label status-label--{fileStatus}">{fileStatus}</span>
                {#if filename}
                    <span class="file-basename">{filename}</span>
                {/if}
            </div>
            {#if displayPath}
                <span class="file-path">{displayPath}</span>
            {/if}
        </div>

        <!-- ── Required fields ── -->
        <section class="field-group">
            <p class="group-label">Required</p>

            <label class="field">
                <span class="field-label">
                    Filename <span class="required">*</span>
                    <span class="hint">— rename on save</span>
                </span>
                <input
                    class="input"
                    class:input--invalid={saveAttempted && !filename.trim()}
                    type="text"
                    placeholder="mountain_sunset"
                    bind:value={filename}
                />
            </label>

            <label class="field">
                <span class="field-label">Title <span class="required">*</span></span>
                <input
                    class="input"
                    class:input--invalid={saveAttempted && !title.trim()}
                    type="text"
                    placeholder="A stunning mountain sunset"
                    bind:value={title}
                />
            </label>

            <label class="field">
                <span class="field-label">Description <span class="required">*</span></span>
                <textarea
                    bind:this={descriptionEl}
                    class="input textarea"
                    class:input--invalid={saveAttempted && !description.trim()}
                    placeholder="Describe the image in detail..."
                    rows={4}
                    bind:value={description}
                ></textarea>
            </label>

            <div class="field">
                <span class="field-label">
                    Keywords <span class="required">*</span>
                    <span class="hint">— Enter or ", " to add</span>
                </span>
                <input
                    bind:this={inputEl}
                    class="input"
                    class:input--invalid={saveAttempted && keywords.length === 0}
                    type="text"
                    placeholder="mountain, sunset, nature..."
                    bind:value={keywordInput}
                    onkeydown={handleKeywordKeydown}
                    oninput={handleKeywordInput}
                    onblur={() => suggestionsComp?.handleBlur()}
                />
                <div class="keyword-actions">
                    <button
                        class="kw-action-btn"
                        onclick={copyKeywords}
                        disabled={keywords.length === 0}
                        title="Copy keywords to clipboard"
                    >
                        <svg viewBox="0 0 16 16" fill="currentColor">
                            <path d="M4 2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V2zm2-1a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V2a1 1 0 0 0-1-1H6z"/>
                            <path d="M2 5a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-1h-1v1H2V6h1V5H2z"/>
                        </svg>
                        Copy
                    </button>
                    <button
                        class="kw-action-btn"
                        onclick={pasteKeywords}
                        title="Paste keywords from clipboard"
                    >
                        <svg viewBox="0 0 16 16" fill="currentColor">
                            <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z"/>
                            <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z"/>
                        </svg>
                        Paste
                    </button>
                </div>
                {#if keywords.length > 0}
                    <div
                        class="keyword-chips"
                        class:keyword-chips--dragging={dragFromIndex !== null}
                        bind:this={chipsEl}
                    >
                        {#each dragDisplay as item (item ?? '__placeholder__')}
                            {#if item === null}
                                <span
                                    class="chip chip--placeholder"
                                    style="width:{ghostWidth}px"
                                ></span>
                            {:else}
                                <span
                                    class="chip"
                                    onpointerdown={(e) => startChipDrag(e, keywords.indexOf(item))}
                                    role="listitem"
                                >
                                    {item}
                                    <button
                                        class="chip-remove"
                                        onclick={() => removeKeyword(item)}
                                        aria-label="Remove keyword {item}"
                                    >×</button>
                                </span>
                            {/if}
                        {/each}
                    </div>
                {/if}
            </div>
        </section>

        <!-- ── Preset keywords ── -->
        <details
            class="optional-details"
            open={stockKeywordsOpen}
            ontoggle={(e) => stockKeywordsOpen = (e.currentTarget as HTMLDetailsElement).open}
        >
            <summary class="optional-summary">
                <span class="group-label" style="border: none; padding: 0;">Stock Keywords</span>
                <svg class="chevron" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M4 6l4 4 4-4"/>
                </svg>
            </summary>
            <div class="optional-body presets">
                {#each Object.entries(presets) as [group, tags]}
                    <div class="preset-group">
                        <span class="preset-group-label">{group}</span>
                        <div class="preset-tags">
                            {#each tags as tag}
                                <button
                                    class="preset-btn"
                                    class:active={keywords.includes(tag)}
                                    onclick={() => addKeyword(tag)}
                                >{tag}</button>
                            {/each}
                        </div>
                    </div>
                {/each}
            </div>
        </details>

        <!-- ── Optional fields ── -->
        <details
            class="optional-details"
            open={optionalOpen}
            ontoggle={(e) => optionalOpen = (e.currentTarget as HTMLDetailsElement).open}
        >
            <summary class="optional-summary">
                <span class="group-label" style="border: none; padding: 0;">Optional</span>
                <svg class="chevron" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M4 6l4 4 4-4"/>
                </svg>
            </summary>
            <div class="optional-body">
                <label class="field">
                    <span class="field-label">Categories</span>
                    <input class="input" type="text" placeholder="Travel, Landscape" bind:value={categories} />
                </label>
                <label class="field">
                    <span class="field-label">Release Filename</span>
                    <input class="input" type="text" placeholder="model_release.pdf" bind:value={releaseFilename} />
                </label>
            </div>
        </details>
    </div>

    <!-- ── Keyword suggestions dropdown ── -->
    <KeywordSuggestions
        bind:this={suggestionsComp}
        {inputEl}
        query={keywordInput}
        onSelect={(kw) => { addKeyword(kw); keywordInput = ''; inputEl?.focus(); }}
    />

    <!-- ── Footer ── -->
    <footer class="panel-footer">
        {#if saveAttempted && hasErrors}
            <div class="footer-errors">
                {validationErrors.join(' · ')}
            </div>
        {/if}
        <div class="footer-controls">
            <label class="autosave-toggle">
                <input type="checkbox" bind:checked={autoSave} />
                <span>Auto-save</span>
            </label>
            <button
                class="btn-primary save-btn"
                onclick={handleSave}
                disabled={!filepath}
            >Save Changes</button>
        </div>
    </footer>
</aside>

<style lang="scss">
    @use '../styles/mixins' as *;

    // ── File info ──
    .file-info {
        @include flex(column, flex-start, stretch);
        gap: 4px;
        padding: 10px 12px;
        background: $bg-surface;
        border: 1px solid $border;
        border-radius: $radius-md;
    }

    .file-status-row {
        @include flex(row, flex-start, center);
        gap: 7px;
        min-width: 0;
    }

    .status-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        flex-shrink: 0;

        &--none { background: $text-muted; }
        &--open { background: #4ade80; box-shadow: 0 0 5px rgba(74, 222, 128, 0.4); }
        &--edit { background: #fbbf24; box-shadow: 0 0 5px rgba(251, 191, 36, 0.4); }
    }

    .status-label {
        font-size: $fs-footnote1;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.06em;
        flex-shrink: 0;

        &--none { color: $text-muted; }
        &--open { color: #4ade80; }
        &--edit { color: #fbbf24; }
    }

    .file-basename {
        font-size: $fs-small;
        font-weight: 500;
        color: $text;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        min-width: 0;
    }

    .file-path {
        font-size: $fs-footnote2;
        color: $text-muted;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        padding-left: 15px;
    }

    // ── Keyword clipboard actions ──
    .keyword-actions {
        @include flex(row, flex-start, center);
        gap: 6px;
        margin-top: 4px;
    }

    .kw-action-btn {
        @include btn-reset;
        @include flex(row, flex-start, center);
        gap: 5px;
        padding: 3px 8px;
        border: 1px solid $border;
        border-radius: $radius-sm;
        background: $bg-surface;
        color: $text-secondary;
        font-size: $fs-footnote1;
        @include transition(background, color, border-color);

        svg {
            width: 11px;
            height: 11px;
            flex-shrink: 0;
        }

        &:hover:not(:disabled) {
            background: #2e2e2e;
            color: $text;
            border-color: $text-muted;
        }

        &:disabled {
            opacity: 0.35;
            cursor: not-allowed;
        }
    }

    // ── Input validation ──
    .input--invalid {
        border-color: $required-color !important;
    }

    // ── Preset keywords ──
    .presets { gap: 8px; }

    .preset-group {
        @include flex(column, flex-start, stretch);
        gap: 5px;
    }

    .preset-group-label {
        font-size: $fs-footnote1;
        color: $text-muted;
        font-weight: 500;
    }

    .preset-tags {
        @include flex(row, flex-start, flex-start);
        flex-wrap: wrap;
        gap: 4px;
    }

    .preset-btn {
        @include btn-reset;
        background: $bg-surface;
        border: 1px solid $border;
        border-radius: $radius-sm;
        color: $text-secondary;
        font-size: $fs-footnote1;
        padding: 3px 8px;
        @include transition(background, color, border-color);

        &:hover { background: #2e2e2e; color: $text; }
        &.active { background: $chip-bg; border-color: $chip-border; color: $chip-text; }
    }

    // ── Optional spoiler ──
    .optional-details {
        border: 1px solid $border;
        border-radius: $radius-md;
    }

    .optional-summary {
        @include flex(row, space-between, center);
        padding: 8px 12px;
        cursor: pointer;
        list-style: none;
        user-select: none;
        background: $bg-surface;
        border-radius: $radius-md;
        @include transition(background);

        &::-webkit-details-marker { display: none; }
        &:hover { background: #2a2a2a; }
    }

    .optional-details[open] .optional-summary { border-radius: $radius-md $radius-md 0 0; }

    .chevron {
        width: 14px;
        height: 14px;
        color: $text-muted;
        transition: transform 0.2s;
        flex-shrink: 0;
    }

    .optional-details[open] .chevron { transform: rotate(180deg); }

    .optional-body {
        @include flex(column, flex-start, stretch);
        gap: 12px;
        padding: 12px;
        background: $bg-panel;
        border-radius: 0 0 $radius-md $radius-md;
    }

    // ── Footer ──
    .panel-footer {
        height: auto;
        min-height: $footer-height;
        border-top: 1px solid $border;
        background: $bg-panel;
        @include flex(column, flex-start, stretch);
        padding: 0;
    }

    .footer-errors {
        padding: 6px 16px;
        font-size: $fs-footnote1;
        color: $required-color;
        border-bottom: 1px solid rgba($required-color, 0.2);
        background: rgba($required-color, 0.06);
        line-height: 1.5;
    }

    .footer-controls {
        @include flex(row, flex-start, center);
        gap: 12px;
        padding: 0 16px;
        flex: 1;
        min-height: $footer-height;
    }

    .autosave-toggle {
        @include flex(row, flex-start, center);
        gap: 6px;
        cursor: pointer;
        color: $text-secondary;
        font-size: $fs-small;
        white-space: nowrap;

        input[type="checkbox"] { accent-color: $accent; cursor: pointer; }
    }

    .save-btn {
        margin-left: auto;
        &:disabled { opacity: 0.4; cursor: not-allowed; }
    }
</style>
