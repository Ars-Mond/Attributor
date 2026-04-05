<script lang="ts">
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import {writeText, readText} from "@tauri-apps/plugin-clipboard-manager";
    import KeywordSuggestions from "$reusable/KeywordSuggestions.svelte";
    import ConfirmDialog from "$lib/dialog/ConfirmDialog.svelte";
    import {loadAppState, saveAppState} from "$lib/store";
    import {settings} from "$lib/settings";
    import type {Metadata, ReadResult} from "$lib/types";

    // ── Bindable props ─────────────────────────────────────────────────────

    let {
        isDirty = $bindable(false),
        onPathChange,
        batchPaths = [],
    }: {
        isDirty?: boolean;
        onPathChange?: (newPath: string) => void;
        batchPaths?: string[];
    } = $props();

    const isBatch = $derived(batchPaths.length > 1);

    // ── Form state ─────────────────────────────────────────────────────────

    let filepath = $state('');          // full OS path (internal, not editable)
    let filename = $state('');          // stem only — shown and editable
    let title = $state('');
    let description = $state('');
    let keywordInput = $state('');
    let keywords = $state<string[]>([]);
    let categories = $state('');
    let releaseFilename = $state('');
    const autoSave = $derived(settings.subscribe('editor.autosave')());
    let saveAttempted = $state(false);
    let saveError = $state<string | null>(null);
    let showClearConfirm = $state(false);

    // ── UI preferences (persisted) ─────────────────────────────────────────

    let descriptionEl = $state<HTMLTextAreaElement | undefined>(undefined);
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
        return {filename, title, description, keywords: [...keywords], categories, releaseFilename};
    }

    const isDirtyComputed = $derived.by(() => {
        const snap = snapshot;
        if (snap === null) return false;
        return (
            filename !== snap.filename ||
            title !== snap.title ||
            description !== snap.description ||
            keywords.length !== snap.keywords.length ||
            keywords.some((k, i) => k !== snap.keywords[i]) ||
            categories !== snap.categories ||
            releaseFilename !== snap.releaseFilename
        );
    });

    $effect(() => {
        isDirty = isDirtyComputed;
    });

    // ── Auto-save ──────────────────────────────────────────────────────────

    $effect(() => {
        filename; title; description; keywords; categories; releaseFilename;
        if (!autoSave || !isDirtyComputed || hasErrors) return;
        const delay = settings.get<number>('editor.autosave_delay');
        const timer = setTimeout(() => { doSave().catch(() => {}); }, delay);
        return () => clearTimeout(timer);
    });

    // ── File status ────────────────────────────────────────────────────────

    const fileStatus = $derived(
        snapshot === null ? 'none' : isDirtyComputed ? 'edit' : 'open'
    );

    const displayPath = $derived(filepath);

    // ── Validation ─────────────────────────────────────────────────────────

    const validationErrors = $derived(
        isBatch ? [] :
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

    // ── Batch mode state ───────────────────────────────────────────────────

    let batchLoading = $state(false);
    let batchTitle = $state('');
    let batchTitleMixed = $state(false);
    let batchDescription = $state('');
    let batchDescMixed = $state(false);
    let batchCategories = $state('');
    let batchCatMixed = $state(false);
    let batchKeywordStates = $state<{word: string; state: 'all' | 'some'}[]>([]);
    let batchOriginalUnion: Set<string> = new Set();
    let kwFileMap = $state<Map<string, string[]>>(new Map());

    let applyTitle = $state(false);
    let applyDescription = $state(false);
    let applyCategories = $state(false);

    let batchKeywordInput = $state('');
    let batchKwInputEl = $state<HTMLInputElement | undefined>(undefined);

    let savingCount = $state(0);
    let savingTotal = $state(0);
    const isSaving = $derived(savingTotal > 0);

    // ── Field stats (depends on batch state) ──────────────────────────────

    const titleWords = $derived.by(() => {
        const t = isBatch ? batchTitle : title;
        return t.trim() ? t.split(' ').length : 0;
    });
    const titleChars = $derived(isBatch ? batchTitle.length : title.length);
    const descWords = $derived.by(() => {
        const d = isBatch ? batchDescription : description;
        return d.trim() ? d.split(' ').length : 0;
    });
    const descChars = $derived(isBatch ? batchDescription.length : description.length);

    let batchLoadId = 0;

    async function loadBatchData(paths: string[]) {
        const myId = ++batchLoadId;
        batchLoading = true;

        const results = await Promise.all(
            paths.map(p => invoke<ReadResult>('read_metadata', {path: p}).catch(() => null))
        );

        if (myId !== batchLoadId) return; // superseded

        const valid = results.filter((r): r is ReadResult => r !== null);

        if (valid.length === 0) {
            batchLoading = false;
            return;
        }

        // Title
        const titleSet = new Set(valid.map(r => r.title));
        batchTitleMixed = titleSet.size > 1;
        batchTitle = batchTitleMixed ? '' : valid[0].title;
        applyTitle = !batchTitleMixed && batchTitle !== '';

        // Description
        const descSet = new Set(valid.map(r => r.description));
        batchDescMixed = descSet.size > 1;
        batchDescription = batchDescMixed ? '' : valid[0].description;
        applyDescription = !batchDescMixed && batchDescription !== '';

        // Categories
        const catSet = new Set(valid.map(r => r.categories));
        batchCatMixed = catSet.size > 1;
        batchCategories = batchCatMixed ? '' : valid[0].categories;
        applyCategories = !batchCatMixed && batchCategories !== '';

        // Keywords: union with per-word state and per-word file list
        const kwSets = valid.map(r => new Set(r.keywords));
        const union = new Set(valid.flatMap(r => r.keywords));
        batchOriginalUnion = union;
        batchKeywordStates = [...union].map(word => ({
            word,
            state: kwSets.every(s => s.has(word)) ? 'all' : 'some',
        }));

        // Map keyword → basenames of files that contain it
        const fileMap = new Map<string, string[]>();
        for (let i = 0; i < paths.length; i++) {
            const r = results[i];
            if (!r) continue;
            const base = paths[i].replace(/\\/g, '/').split('/').pop() ?? paths[i];
            for (const kw of r.keywords) {
                const arr = fileMap.get(kw);
                if (arr) arr.push(base);
                else fileMap.set(kw, [base]);
            }
        }
        kwFileMap = fileMap;

        batchLoading = false;
    }

    $effect(() => {
        const paths = batchPaths;
        if (paths.length <= 1) {
            batchLoadId++; // cancel any in-flight load
            batchLoading = false;
            return;
        }
        loadBatchData(paths);
    });

    // ── Batch keyword helpers ──────────────────────────────────────────────

    function addBatchKeyword(word: string) {
        const trimmed = word.trim().toLowerCase();
        if (!trimmed || batchKeywordStates.some(s => s.word === trimmed)) return;
        batchKeywordStates = [...batchKeywordStates, {word: trimmed, state: 'all'}];
    }

    function promoteBatchKeyword(word: string) {
        batchKeywordStates = batchKeywordStates.map(s =>
            s.word === word ? {word, state: 'all'} : s
        );
    }

    function removeBatchKeyword(word: string) {
        batchKeywordStates = batchKeywordStates.filter(s => s.word !== word);
    }

    function handleBatchKeywordKeydown(e: KeyboardEvent) {
        if (suggestionsComp?.handleKeydown(e)) return;
        if (e.key === 'Enter') {
            e.preventDefault();
            addBatchKeyword(batchKeywordInput);
            batchKeywordInput = '';
        }
    }

    function handleBatchKeywordInput() {
        if (batchKeywordInput.includes(', ')) {
            const parts = batchKeywordInput.split(', ');
            for (let i = 0; i < parts.length - 1; i++) addBatchKeyword(parts[i]);
            batchKeywordInput = parts[parts.length - 1];
        }
    }

    async function copyBatchKeywords() {
        const common = batchKeywordStates.filter(s => s.state === 'all').map(s => s.word);
        if (!common.length) return;
        await writeText(common.join(', ') + ', ');
    }

    async function pasteBatchKeywords() {
        const text = await readText();
        if (!text) return;
        const parts = text.split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
        for (const word of parts) addBatchKeyword(word);
    }

    // Unified keyword adder for preset buttons
    function handleAddKeyword(word: string) {
        if (isBatch) {
            addBatchKeyword(word);
        } else {
            addKeyword(word);
        }
    }

    // Compute the new keyword list for a file during batch save
    function computeNewKeywords(fileKeywords: string[]): string[] {
        const currentWords = new Set(batchKeywordStates.map(s => s.word));
        const allWords = batchKeywordStates.filter(s => s.state === 'all').map(s => s.word);
        // Keywords that were in the original union but removed by user
        const removedWords = new Set([...batchOriginalUnion].filter(w => !currentWords.has(w)));

        const kept = fileKeywords.filter(k => !removedWords.has(k));
        const toAdd = allWords.filter(k => !kept.includes(k));
        return [...kept, ...toAdd];
    }

    async function handleBatchSave() {
        savingTotal = batchPaths.length;
        savingCount = 0;

        for (const path of batchPaths) {
            try {
                let currentMeta: ReadResult | null = null;
                try {
                    currentMeta = await invoke<ReadResult>('read_metadata', {path});
                } catch { /* use empty defaults */ }

                const metadata: Metadata = {
                    filepath: path,
                    filename: extractStem(path),
                    title: applyTitle ? batchTitle : (currentMeta?.title ?? ''),
                    description: applyDescription ? batchDescription : (currentMeta?.description ?? ''),
                    keywords: computeNewKeywords(currentMeta?.keywords ?? []),
                    categories: applyCategories ? batchCategories : (currentMeta?.categories ?? ''),
                    releaseFilename: currentMeta?.releaseFilename ?? '',
                };

                await invoke('save_metadata', {metadata});
            } catch (e) {
                console.error('Batch save failed for', path, e);
            }
            savingCount++;
        }

        savingTotal = 0;
        // Reload to reflect saved state
        loadBatchData(batchPaths);
    }

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
        snapshot = null;

        try {
            const meta = await invoke<ReadResult>('read_metadata', {path});
            title = meta.title;
            description = meta.description;
            keywords = meta.keywords;
            categories = meta.categories;
            releaseFilename = meta.releaseFilename;
        } catch (e) {
            console.warn('read_metadata failed:', e);
        }

        snapshot = captureSnapshot();
    }

    /** Clear everything — called when the open file is deleted externally. */
    export function clear(): void {
        filepath = '';
        filename = '';
        title = '';
        description = '';
        keywordInput = '';
        keywords = [];
        categories = '';
        releaseFilename = '';
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
        filename = extractStem(newPath);
        snapshot = captureSnapshot();

        if (newPath !== prevFilepath) {
            onPathChange?.(newPath);
        }

        return newPath;
    }

    async function handleSave() {
        saveAttempted = true;
        saveError = null;
        if (hasErrors) return;
        try {
            await doSave();
        } catch (e) {
            saveError = e instanceof Error ? e.message : String(e);
        }
    }

    // ── Preset keywords ────────────────────────────────────────────────────

    const presets: Record<string, string[]> = {
        Nature: [
            'nature', 'landscape', 'sky', 'water', 'forest', 'mountain',
            'sunset', 'ocean', 'river', 'flower', 'tree', 'grass', 'cloud',
        ],
        People: [
            'portrait', 'woman', 'man', 'child', 'family', 'people',
            'lifestyle', 'crowd', 'face', 'smile',
        ],
        Urban: [
            'city', 'architecture', 'building', 'street', 'urban',
            'skyline', 'road', 'bridge',
        ],
        Concepts: [
            'business', 'technology', 'abstract', 'vintage', 'minimal',
            'creative', 'design', 'background', 'texture', 'pattern',
        ],
        Animals: [
            'dog', 'cat', 'bird', 'wildlife', 'animal', 'pet', 'horse',
        ],
        Seasons: [
            'winter', 'summer', 'spring', 'autumn', 'snow', 'rain', 'fog',
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

    let inputEl = $state<HTMLInputElement | undefined>(undefined);
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

    function clearKeywords() {
        if (isBatch) {
            batchKeywordStates = [];
        } else {
            keywords = [];
        }
        showClearConfirm = false;
    }

    // ── Batch keyword drag & drop ──────────────────────────────────────────

    let batchDragFromIndex = $state<number | null>(null);
    let batchDropInsert = $state<number | null>(null);
    let batchGhostWidth = $state(0);
    let batchChipsEl = $state<HTMLElement | undefined>(undefined);
    let batchFrozenRects: DOMRect[] = [];

    const batchDragDisplay = $derived.by((): ({word: string; state: 'all' | 'some'} | null)[] => {
        if (batchDragFromIndex === null || batchDropInsert === null) return batchKeywordStates;
        const arr: ({word: string; state: 'all' | 'some'} | null)[] = batchKeywordStates.filter((_, i) => i !== batchDragFromIndex);
        arr.splice(Math.min(batchDropInsert, arr.length), 0, null);
        return arr;
    });

    function startBatchChipDrag(e: PointerEvent, kwIndex: number) {
        if ((e.target as HTMLElement).closest('.chip-remove') || (e.target as HTMLElement).closest('.chip-promote')) return;
        if (batchDragFromIndex !== null || batchKeywordStates.length < 2) return;
        e.preventDefault();

        const chipEl = e.currentTarget as HTMLElement;
        const rect = chipEl.getBoundingClientRect();
        const ox = e.clientX - rect.left;
        const oy = e.clientY - rect.top;
        batchGhostWidth = rect.width;

        if (batchChipsEl) {
            batchFrozenRects = [...batchChipsEl.querySelectorAll<HTMLElement>('.chip')]
                .filter((_, i) => i !== kwIndex)
                .map(c => c.getBoundingClientRect());
        }

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

        batchDragFromIndex = kwIndex;
        batchDropInsert = kwIndex;

        const onMove = (ev: PointerEvent) => {
            ghost.style.left = `${ev.clientX - ox}px`;
            ghost.style.top = `${ev.clientY - oy}px`;
            const next = calcDropInsert(ev.clientX, ev.clientY, batchFrozenRects);
            if (next !== batchDropInsert) batchDropInsert = next;
        };

        const onUp = () => {
            ghost.remove();
            if (batchDropInsert !== null && batchDragFromIndex !== null && batchDropInsert !== batchDragFromIndex) {
                const arr = [...batchKeywordStates];
                const [moved] = arr.splice(batchDragFromIndex, 1);
                arr.splice(batchDropInsert, 0, moved);
                batchKeywordStates = arr;
            }
            batchDragFromIndex = null;
            batchDropInsert = null;
            batchGhostWidth = 0;
            batchFrozenRects = [];
            document.removeEventListener('pointermove', onMove);
            document.removeEventListener('pointerup', onUp);
        };

        document.addEventListener('pointermove', onMove);
        document.addEventListener('pointerup', onUp);
    }

    // ── Keyword drag & drop (pointer-based, ghost + placeholder) ──────────

    let dragFromIndex = $state<number | null>(null);
    let dropInsert = $state<number | null>(null);
    let ghostWidth = $state(0);
    let chipsEl = $state<HTMLElement | undefined>(undefined);

    // Chip rects captured once at drag start to prevent oscillation
    let frozenRects: DOMRect[] = [];

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

        if (chipsEl) {
            frozenRects = [...chipsEl.querySelectorAll<HTMLElement>('.chip')]
                .filter((_, i) => i !== kwIndex)
                .map(c => c.getBoundingClientRect());
        }

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
            const next = calcDropInsert(ev.clientX, ev.clientY, frozenRects);
            if (next !== dropInsert) dropInsert = next;
        };

        const onUp = () => {
            ghost.remove();
            if (dropInsert !== null && dragFromIndex !== null && dropInsert !== dragFromIndex) {
                const arr = [...keywords];
                const [moved] = arr.splice(dragFromIndex, 1);
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

    function calcDropInsert(x: number, y: number, rects: DOMRect[]): number {
        if (!rects.length) return 0;
        const chipH = rects[0].height;

        const rows: {top: number; bottom: number; indices: number[]}[] = [];
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

        let targetRow = rows[rows.length - 1];
        for (let r = 0; r < rows.length - 1; r++) {
            const midY = (rows[r].bottom + rows[r + 1].top) / 2;
            if (y <= midY) { targetRow = rows[r]; break; }
        }

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
        {#if isBatch}
            <div class="file-info">
                <div class="file-status-row">
                    <span class="status-dot status-dot--batch"></span>
                    <span class="status-label status-label--batch">Batch</span>
                    <span class="file-basename">{batchPaths.length} files</span>
                </div>
                {#if batchLoading}
                    <span class="file-path">Loading...</span>
                {/if}
            </div>
        {:else}
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
        {/if}

        <!-- ── Required / batch fields ── -->
        {#if isBatch}
            <section class="field-group">
                <p class="group-label">Fields</p>

                <!-- Title -->
                <div class="field">
                    <div class="field-header">
                        <label class="batch-apply-label">
                            <input type="checkbox" bind:checked={applyTitle} />
                            <span class="field-label">Title</span>
                        </label>
                        <span class="field-stats">{titleWords}w : {titleChars}l</span>
                    </div>
                    <input
                        class="input"
                        type="text"
                        placeholder={batchTitleMixed ? '(mixed values)' : 'A stunning mountain sunset'}
                        bind:value={batchTitle}
                        oninput={() => { if (batchTitle) applyTitle = true; }}
                    />
                </div>

                <!-- Description -->
                <div class="field">
                    <div class="field-header">
                        <label class="batch-apply-label">
                            <input type="checkbox" bind:checked={applyDescription} />
                            <span class="field-label">Description</span>
                        </label>
                        <span class="field-stats">{descWords}w : {descChars}l</span>
                    </div>
                    <textarea
                        bind:this={descriptionEl}
                        class="input textarea"
                        placeholder={batchDescMixed ? '(mixed values)' : 'Describe the image in detail...'}
                        rows={4}
                        bind:value={batchDescription}
                        oninput={() => { if (batchDescription) applyDescription = true; }}
                    ></textarea>
                </div>

                <!-- Keywords (batch) -->
                <div class="field">
                    <span class="field-label">Keywords <span class="hint">— Enter or ", " to add</span></span>
                    <input
                        bind:this={batchKwInputEl}
                        class="input"
                        type="text"
                        placeholder="Add keyword to all..."
                        bind:value={batchKeywordInput}
                        onkeydown={handleBatchKeywordKeydown}
                        oninput={handleBatchKeywordInput}
                        onblur={() => suggestionsComp?.handleBlur()}
                    />
                    <div class="keyword-actions">
                        <button
                            class="kw-action-btn"
                            onclick={copyBatchKeywords}
                            disabled={batchKeywordStates.filter(s => s.state === 'all').length === 0}
                            title="Copy common keywords to clipboard"
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M4 2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V2zm2-1a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V2a1 1 0 0 0-1-1H6z"/>
                                <path d="M2 5a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-1h-1v1H2V6h1V5H2z"/>
                            </svg>
                            Copy
                        </button>
                        <button
                            class="kw-action-btn"
                            onclick={pasteBatchKeywords}
                            title="Paste keywords from clipboard"
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z"/>
                                <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z"/>
                            </svg>
                            Paste
                        </button>
                        <button
                            class="kw-action-btn kw-action-btn--danger"
                            onclick={() => { showClearConfirm = true; }}
                            disabled={batchKeywordStates.length === 0}
                            title="Clear all keywords"
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z"/>
                                <path fill-rule="evenodd" d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z"/>
                            </svg>
                            Clear
                        </button>
                        <span class="kw-count">{batchKeywordStates.length}</span>
                    </div>
                    {#if batchKeywordStates.length > 0}
                        <div
                            class="keyword-chips"
                            class:keyword-chips--dragging={batchDragFromIndex !== null}
                            bind:this={batchChipsEl}
                        >
                            {#each batchDragDisplay as item, i (item === null ? `__placeholder__${i}` : `${i}:${item.word}`)}
                                {#if item === null}
                                    <span
                                        class="chip chip--placeholder"
                                        style="width:{batchGhostWidth}px"
                                    ></span>
                                {:else}
                                    <span
                                        class="chip"
                                        class:chip--some={item.state === 'some'}
                                        onpointerdown={(e) => startBatchChipDrag(e, batchKeywordStates.findIndex(s => s.word === item.word))}
                                        role="listitem"
                                        title={(kwFileMap.get(item.word) ?? []).join('\n')}
                                    >
                                        {#if item.state === 'some'}
                                            <button
                                                class="chip-promote"
                                                onclick={() => promoteBatchKeyword(item.word)}
                                                title="Add to all files"
                                            ></button>
                                        {/if}
                                        {item.word}
                                        <button
                                            class="chip-remove"
                                            onclick={() => removeBatchKeyword(item.word)}
                                            aria-label="Remove keyword {item.word}"
                                        >×</button>
                                    </span>
                                {/if}
                            {/each}
                        </div>
                    {/if}
                </div>
            </section>
        {:else}
            <!-- ── Required fields (single mode) ── -->
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
                    <div class="field-header">
                        <span class="field-label">Title <span class="required">*</span></span>
                        <span class="field-stats">{titleWords}w : {titleChars}l</span>
                    </div>
                    <input
                        class="input"
                        class:input--invalid={saveAttempted && !title.trim()}
                        type="text"
                        placeholder="A stunning mountain sunset"
                        bind:value={title}
                    />
                </label>

                <label class="field">
                    <div class="field-header">
                        <span class="field-label">Description <span class="required">*</span></span>
                        <span class="field-stats">{descWords}w : {descChars}l</span>
                    </div>
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
                        <button
                            class="kw-action-btn kw-action-btn--danger"
                            onclick={() => { showClearConfirm = true; }}
                            disabled={keywords.length === 0}
                            title="Clear all keywords"
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z"/>
                                <path fill-rule="evenodd" d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z"/>
                            </svg>
                            Clear
                        </button>
                        <span class="kw-count">{keywords.length}</span>
                    </div>
                    {#if keywords.length > 0}
                        <div
                            class="keyword-chips"
                            class:keyword-chips--dragging={dragFromIndex !== null}
                            bind:this={chipsEl}
                        >
                            {#each dragDisplay as item, i (item === null ? `__placeholder__${i}` : `${i}:${item}`)}
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
        {/if}

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
                                    class:active={isBatch
                                        ? batchKeywordStates.some(s => s.word === tag && s.state === 'all')
                                        : keywords.includes(tag)}
                                    onclick={() => handleAddKeyword(tag)}
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
                {#if isBatch}
                    <div class="field">
                        <div class="field-header">
                            <label class="batch-apply-label">
                                <input type="checkbox" bind:checked={applyCategories} />
                                <span class="field-label">Categories</span>
                            </label>
                        </div>
                        <input
                            class="input"
                            type="text"
                            placeholder={batchCatMixed ? '(mixed values)' : 'Travel, Landscape'}
                            bind:value={batchCategories}
                            oninput={() => { if (batchCategories) applyCategories = true; }}
                        />
                    </div>
                {:else}
                    <label class="field">
                        <span class="field-label">Categories</span>
                        <input class="input" type="text" placeholder="Travel, Landscape" bind:value={categories} />
                    </label>
                    <label class="field">
                        <span class="field-label">Release Filename</span>
                        <input class="input" type="text" placeholder="model_release.pdf" bind:value={releaseFilename} />
                    </label>
                {/if}
            </div>
        </details>
    </div>

    {#if showClearConfirm}
        <ConfirmDialog
            title="Clear Keywords"
            body={isBatch
                ? `Remove all keywords from all ${batchPaths.length} selected files?`
                : 'Remove all keywords from this image?'}
            icon="warning"
            buttons={[
                {label: 'Cancel', onClick: () => { showClearConfirm = false; }},
                {
                    label: 'Clear All',
                    onClick: clearKeywords,
                    color: 'var(--required-color)',
                    border: 'var(--required-color)',
                    hoverBg: 'var(--required-alpha-08)',
                    hoverBorder: 'var(--required-color)',
                    hoverColor: 'var(--required-color)',
                },
            ]}
            onClose={() => { showClearConfirm = false; }}
        />
    {/if}

    <!-- ── Keyword suggestions dropdown ── -->
    <KeywordSuggestions
        bind:this={suggestionsComp}
        inputEl={isBatch ? batchKwInputEl : inputEl}
        query={isBatch ? batchKeywordInput : keywordInput}
        onSelect={isBatch
            ? (kw) => { addBatchKeyword(kw); batchKeywordInput = ''; batchKwInputEl?.focus(); }
            : (kw) => { addKeyword(kw); keywordInput = ''; inputEl?.focus(); }
        }
    />

    <!-- ── Footer ── -->
    <footer class="panel-footer">
        {#if !isBatch && saveAttempted && hasErrors}
            <div class="footer-errors">
                {validationErrors.join(' · ')}
            </div>
        {:else if !isBatch && saveError}
            <div class="footer-errors">
                Save failed: {saveError}
            </div>
        {/if}
        <div class="footer-controls">
            {#if isBatch}
                <button
                    class="btn-primary save-btn"
                    onclick={handleBatchSave}
                    disabled={isSaving}
                >
                    {isSaving ? `Saving ${savingCount}/${savingTotal}...` : `Save ${batchPaths.length} Files`}
                </button>
            {:else}
                <label class="autosave-toggle">
                    <input
                        type="checkbox"
                        checked={autoSave as boolean}
                        onchange={(e) => settings.set('editor.autosave', e.currentTarget.checked)}
                    />
                    <span>Auto-save</span>
                </label>
                <button
                    class="btn-primary save-btn"
                    onclick={handleSave}
                    disabled={!filepath}
                >Save Changes</button>
            {/if}
        </div>
    </footer>
</aside>

<style lang="scss">
    @use 'styles/mixins' as *;

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
        &--batch { background: #60a5fa; box-shadow: 0 0 5px rgba(96, 165, 250, 0.4); }
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
        &--batch { color: #60a5fa; }
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
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }

        &:disabled {
            opacity: 0.35;
            cursor: not-allowed;
        }

        &--danger:hover:not(:disabled) {
            background: var(--required-alpha-08);
            color: var(--required-color);
            border-color: var(--required-color);
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

        &:hover { background: var(--hover-bg-strong); color: $text; }
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
        &:hover { background: var(--hover-bg); }
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
        border-bottom: 1px solid var(--required-alpha-20);
        background: var(--required-alpha-06);
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

    // ── Field header (label + stats row) ──
    .field-header {
        @include flex(row, space-between, baseline);
    }

    .field-stats {
        font-size: $fs-footnote2;
        color: $text-muted;
        font-weight: 400;
        flex-shrink: 0;
    }

    // ── Keyword count ──
    .kw-count {
        margin-left: auto;
        font-size: $fs-footnote1;
        color: $text-muted;
    }

    // ── Batch apply checkbox row ──
    .batch-apply-label {
        @include flex(row, flex-start, center);
        gap: 6px;
        cursor: pointer;

        input[type="checkbox"] { accent-color: $accent; cursor: pointer; }
    }

    // ── Batch 'some' chip ──
    .chip--some {
        border: 1px dashed $text-muted !important;
        background: $bg-surface !important;
        color: $text-muted !important;
        // chip-promote takes over the left padding hit area
        padding-left: 0 !important;
    }

    .chip-promote {
        @include btn-reset;
        // Extend hit area to chip left/top/bottom edges
        align-self: stretch;
        display: flex;
        align-items: center;
        margin: -3px 0 -3px 0;
        padding: 0 4px 0 8px;
        flex-shrink: 0;
        cursor: pointer;

        // Visual circle via pseudo-element
        &::before {
            content: '';
            display: block;
            width: 10px;
            height: 10px;
            border-radius: 50%;
            background: $accent;
            flex-shrink: 0;
        }

        &:hover::before { opacity: 0.75; }
    }
</style>
