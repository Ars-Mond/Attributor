<script lang="ts">
    import {onMount} from "svelte";
    import {invoke, Channel} from "@tauri-apps/api/core";
    import {writeText, readText} from "@tauri-apps/plugin-clipboard-manager";
    import KeywordSuggestions from "$reusable/KeywordSuggestions.svelte";
    import ConfirmDialog from "$lib/dialog/ConfirmDialog.svelte";
    import {loadAppState, saveAppState} from "$lib/store";
    import {settings} from "$lib/settings";
    import {t, tn, type MessageKey} from "$lib/i18n";
    import {attributePhoto, attributeBatch, cancelOllama} from "$lib/ollama/ollama";
    import {ollama} from "$lib/ollama/availability.svelte";
    import {progress} from "$lib/progress.svelte";
    import {openMetadata, storeMetadata, revertToFile, applyMetadataSource} from "$lib/store/metadata";
    import {warn, error} from "@tauri-apps/plugin-log";
    import type {Metadata, SyncState, StoredMetadata} from "$lib/types";
    import type {BatchProgress, ItemStatus} from "$lib/events";
    import {SvelteMap} from "svelte/reactivity";
    import {panelState} from "./filesPanelStore.svelte";

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
    // Attribution-only flags (filled by Ollama, shown as checkboxes; not written to file metadata).
    let editorial = $state(false);
    let matureContent = $state(false);
    let illustration = $state(false);
    // Stock-keyword presets popup (moved out of the inline spoiler into an on-demand dialog).
    let showStockKeywords = $state(false);
    // Sync state of the open record vs the file (feature 008): 'appOnly' → metadata is in the app
    // store but not yet written to the file; 'synced' → file and store agree; null → no file open.
    let syncState = $state<SyncState | null>(null);
    // Conflict resolution (feature 008, US3): a single-photo path awaiting a store-vs-file choice, and
    // the list of batch paths whose files changed externally (resolved together, apply-to-all).
    let conflictPath = $state<string | null>(null);
    let batchConflicts = $state<string[]>([]);
    const autoSave = $derived(settings.subscribe('editor.autosave')());
    let saveAttempted = $state(false);
    let saveError = $state<string | null>(null);
    let attributeError = $state<string | null>(null);
    let showClearConfirm = $state(false);
    let showRevertConfirm = $state(false);

    // ── UI preferences (persisted) ─────────────────────────────────────────

    let descriptionEl = $state<HTMLTextAreaElement | undefined>(undefined);
    let optionalOpen = $state(false);
    let uiLoaded = $state(false);

    onMount(async () => {
        const s = await loadAppState();
        // Apply height imperatively — avoids reactive style conflicts with browser resize handle
        if (s.descriptionHeight && descriptionEl) {
            descriptionEl.style.height = `${s.descriptionHeight}px`;
        }
        if (s.optionalOpen !== undefined) optionalOpen = s.optionalOpen;
        uiLoaded = true;
        void ollama.refresh();
    });

    // ── Ollama attribution ─────────────────────────────────────────────────

    /** Single-photo attribution: fill the form from the model (overwrite text, append+dedupe keywords). */
    async function handleAttribute() {
        if (!ollama.available || !filepath) return;
        attributeError = null;
        // Blocking overlay with a cancel button — freezes the whole app until the inference finishes
        // or the user cancels (which aborts the backend request).
        const handle = progress.run({
            label: t('ollama.attribute.progress'),
            blocking: true,
            cancelable: true,
            onCancel: () => {cancelOllama();}
        });
        try {
            const r = await attributePhoto(filepath);
            title = r.title;
            description = r.description;
            categories = r.categories.join(', ');
            editorial = r.editorial;
            matureContent = r.matureContent;
            illustration = r.illustration;
            for (const kw of r.keywords) addKeyword(kw);
            // Persist the attribution result to the store immediately (don't wait for the debounce),
            // so expensive attribution survives an app close within the debounce window (SC-002).
            await flushStore();
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            // A user-initiated cancel surfaces as "cancelled" from the backend — not a real error.
            if (msg !== 'cancelled') {
                attributeError = t('ollama.attribute.failed', {error: msg});
            }
        } finally {
            handle.done();
        }
    }

    /** Batch attribution: attribute + always save every selected photo, via the blocking overlay. */
    async function handleBatchAttribute() {
        if (!ollama.available || isSaving || batchLoading) return;
        const paths = [...batchPaths];
        const handle = progress.run({
            label: t('ollama.attribute.batch.progress'),
            total: paths.length,
            blocking: true,
            cancelable: true,
            onCancel: () => {cancelOllama();}
        });
        if (batchGuardTimer) {clearTimeout(batchGuardTimer); batchGuardTimer = null;}
        panelState.batchInProgress = true;
        let done = 0;
        try {
            await attributeBatch(paths, () => {done++; handle.update({value: done});});
            loadBatchData(batchPaths);
        } catch (e) {
            error(`batch attribution failed: ${e}`);
        } finally {
            handle.done();
            // Keep the watcher-rescan guard armed briefly past our own writes (see handleBatchSave).
            batchGuardTimer = setTimeout(() => {panelState.batchInProgress = false; batchGuardTimer = null;}, 1000);
        }
    }

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

    // Persist spoiler state
    $effect(() => {
        if (!uiLoaded) return;
        saveAppState({optionalOpen});
    });

    // ── Snapshot (dirty tracking) ──────────────────────────────────────────

    interface Snapshot {
        filename: string;
        title: string;
        description: string;
        keywords: string[];
        categories: string;
        releaseFilename: string;
        editorial: boolean;
        matureContent: boolean;
        illustration: boolean;
    }

    let snapshot = $state<Snapshot | null>(null);

    function captureSnapshot(): Snapshot {
        return {filename, title, description, keywords: [...keywords], categories, releaseFilename, editorial, matureContent, illustration};
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
            releaseFilename !== snap.releaseFilename ||
            editorial !== snap.editorial ||
            matureContent !== snap.matureContent ||
            illustration !== snap.illustration
        );
    });

    $effect(() => {
        isDirty = isDirtyComputed;
    });

    // Store-backed dirtiness: the fields the store actually persists (everything except `filename`,
    // which is a file-rename realized only on save). Drives the app-only persistence so that editing
    // only the filename never flips a synced record to app-only.
    const metadataDirty = $derived.by(() => {
        const snap = snapshot;
        if (snap === null) return false;
        return (
            title !== snap.title ||
            description !== snap.description ||
            keywords.length !== snap.keywords.length ||
            keywords.some((k, i) => k !== snap.keywords[i]) ||
            categories !== snap.categories ||
            releaseFilename !== snap.releaseFilename ||
            editorial !== snap.editorial ||
            matureContent !== snap.matureContent ||
            illustration !== snap.illustration
        );
    });

    // Persist the working store-backed fields app-only and rebaseline. Used by the debounced effect
    // and for an immediate flush after single attribution. No-op when nothing store-backed changed.
    async function flushStore() {
        if (isBatch || !filepath || snapshot === null || !metadataDirty) return;
        const path = filepath;
        const fields: StoredMetadata = {title, description, keywords: [...keywords], categories, releaseFilename, editorial, matureContent, illustration};
        try {
            const s = await storeMetadata(path, fields);
            if (filepath !== path || snapshot === null) return;
            syncState = s;
            // Rebaseline the store-backed fields so the status flips edit → app (keep filename dirty).
            snapshot = {...snapshot, title: fields.title, description: fields.description, keywords: [...fields.keywords], categories: fields.categories, releaseFilename: fields.releaseFilename, editorial: fields.editorial, matureContent: fields.matureContent, illustration: fields.illustration};
        } catch (e) {
            warn(`storeMetadata failed: ${e}`);
        }
    }

    // ── Auto-save ──────────────────────────────────────────────────────────

    $effect(() => {
        filename; title; description; keywords; categories; releaseFilename; editorial; matureContent; illustration;
        if (!autoSave || !isDirtyComputed || hasErrors) return;
        const delay = settings.get<number>('editor.autosave_delay');
        const timer = setTimeout(() => { doSave().catch(() => {}); }, delay);
        return () => clearTimeout(timer);
    });

    // ── Store persistence (feature 008) ────────────────────────────────────
    // The store is the working layer — persist store-backed edits app-only (debounced). Skipped only
    // when file-autosave will actually write the file (i.e. autosave on AND the form is valid); when
    // autosave is on but invalid, doSave bails, so the store must still capture the working edits.
    $effect(() => {
        title; description; keywords; categories; releaseFilename; editorial; matureContent; illustration;
        if (isBatch || !filepath || snapshot === null || (autoSave && !hasErrors)) return;
        if (!metadataDirty) return;
        const delay = settings.get<number>('editor.autosave_delay');
        const timer = setTimeout(() => { flushStore(); }, delay);
        return () => clearTimeout(timer);
    });

    // ── File status ────────────────────────────────────────────────────────

    const fileStatus = $derived(
        snapshot === null ? 'none'
        : isDirtyComputed ? 'edit'
        : syncState === 'appOnly' ? 'app'
        : 'open'
    );

    const displayPath = $derived(filepath);

    // ── Validation ─────────────────────────────────────────────────────────

    const validationErrors = $derived(
        isBatch ? [] :
        !filepath
            ? [t('metadata.validation.noFileSelected')]
            : (
                [
                    !filename.trim() && t('metadata.validation.filenameRequired'),
                    !title.trim() && t('metadata.validation.titleRequired'),
                    !description.trim() && t('metadata.validation.descriptionRequired'),
                    keywords.length === 0 && t('metadata.validation.keywordRequired'),
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

    // Per-file metadata captured when the batch loaded — lets save resolve items without re-reading.
    let batchFileMeta = $state<Map<string, StoredMetadata>>(new Map());
    // Per-file outcomes streamed from the backend during a batch save (keyed by item index).
    let batchResults = new SvelteMap<number, ItemStatus>();
    let batchCancelling = $state(false);
    let batchGuardTimer: ReturnType<typeof setTimeout> | null = null;
    const batchFailed = $derived([...batchResults.values()].filter(s => s.kind === 'failed').length);
    const batchCancelled = $derived([...batchResults.values()].filter(s => s.kind === 'cancelled').length);

    // ── Field stats (depends on batch state) ──────────────────────────────

    const titleWords = $derived.by(() => {
        const tv = isBatch ? batchTitle : title;
        return tv.trim() ? tv.split(' ').length : 0;
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

        // Store-first resolution per file (US3). A conflict provisionally shows the store version and
        // is collected for a single apply-to-all prompt.
        const resolutions = await Promise.all(
            paths.map(p => openMetadata(p).catch(() => null))
        );

        if (myId !== batchLoadId) return; // superseded

        const conflicts: string[] = [];
        const results: (StoredMetadata | null)[] = resolutions.map((res, i) => {
            if (!res) return null;
            if (res.kind === 'resolved') return res.metadata;
            conflicts.push(paths[i]);
            return res.store;
        });
        batchConflicts = conflicts;

        const valid = results.filter((r): r is StoredMetadata => r !== null);

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

        // Map keyword → basenames of files that contain it, and cache each file's metadata.
        const fileMap = new Map<string, string[]>();
        const metaMap = new Map<string, StoredMetadata>();
        for (let i = 0; i < paths.length; i++) {
            const r = results[i];
            if (!r) continue;
            metaMap.set(paths[i], r);
            const base = paths[i].replace(/\\/g, '/').split('/').pop() ?? paths[i];
            for (const kw of r.keywords) {
                const arr = fileMap.get(kw);
                if (arr) arr.push(base);
                else fileMap.set(kw, [base]);
            }
        }
        kwFileMap = fileMap;
        batchFileMeta = metaMap;

        batchLoading = false;
    }

    $effect(() => {
        const paths = batchPaths;
        if (paths.length <= 1) {
            batchLoadId++; // cancel any in-flight load
            batchLoading = false;
            batchConflicts = [];
            return;
        }
        conflictPath = null;
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
        // The metadata cache (batchFileMeta) is only ready after loadBatchData resolves;
        // saving before then would resolve every item to empty and overwrite files.
        if (batchLoading) return;
        const paths = [...batchPaths];
        savingTotal = paths.length;
        savingCount = 0;
        batchCancelling = false;
        batchResults.clear();
        if (batchGuardTimer) {
            clearTimeout(batchGuardTimer);
            batchGuardTimer = null;
        }
        panelState.batchInProgress = true;

        // Route through the top-most overlay and freeze the UI until the save completes (FR-020).
        const handle = progress.run({
            label: t('metadata.button.saveBatch.progress', {n: 0, total: paths.length}),
            blocking: true,
            cancelable: true,
            onCancel: () => {cancelBatchSave();}
        });

        // Resolve each file's final metadata from data already loaded for the batch (no re-read).
        const items: Metadata[] = paths.map(path => {
            const cur = batchFileMeta.get(path);
            return {
                filepath: path,
                filename: extractStem(path),
                title: applyTitle ? batchTitle : (cur?.title ?? ''),
                description: applyDescription ? batchDescription : (cur?.description ?? ''),
                keywords: computeNewKeywords(cur?.keywords ?? []),
                categories: applyCategories ? batchCategories : (cur?.categories ?? ''),
                releaseFilename: cur?.releaseFilename ?? '',
                // Attribution flags are single-mode only; batch save does not edit them (US3 will carry
                // them through the store). Backend SaveRequest defaults these, so false is a no-op here.
                editorial: false,
                matureContent: false,
                illustration: false,
            };
        });

        // One backend call writes the whole batch concurrently; progress streams over the channel.
        const channel = new Channel<BatchProgress>();
        channel.onmessage = (m) => {
            batchResults.set(m.index, m.status);
            savingCount = batchResults.size;
            handle.update({label: t('metadata.button.saveBatch.progress', {n: savingCount, total: savingTotal})});
        };

        try {
            await invoke<ItemStatus[]>('save_metadata_batch', {items, onProgress: channel});
        } catch (e) {
            error(`batch save failed: ${e}`);
        }

        handle.done();
        savingTotal = 0;
        batchCancelling = false;
        // Reload to reflect saved state
        loadBatchData(batchPaths);
        // Keep the watcher-rescan guard armed briefly: the folder-changed events for our own
        // writes are delivered AFTER this call resolves, so clearing synchronously would let
        // them trigger a redundant full rescan / thumbnail-pipeline restart (FR-008).
        batchGuardTimer = setTimeout(() => {
            panelState.batchInProgress = false;
            batchGuardTimer = null;
        }, 1000);
    }

    async function cancelBatchSave() {
        batchCancelling = true;
        try {
            await invoke('cancel_batch');
        } catch (e) {
            error(`cancel_batch failed: ${e}`);
        }
    }

    // ── Exported interface ─────────────────────────────────────────────────

    /** Load a file: reset fields, resolve metadata store-first (feature 008), then take snapshot. */
    export async function loadFile(path: string): Promise<void> {
        filepath = path;
        filename = extractStem(path);
        title = '';
        description = '';
        keywordInput = '';
        keywords = [];
        categories = '';
        releaseFilename = '';
        editorial = false;
        matureContent = false;
        illustration = false;
        saveAttempted = false;
        saveError = null;
        attributeError = null;
        snapshot = null;
        syncState = null;
        conflictPath = null;

        try {
            const res = await openMetadata(path);
            let meta: StoredMetadata;
            if (res.kind === 'resolved') {
                meta = res.metadata;
                syncState = res.syncState;
            } else {
                // Conflict (file changed outside the app): show the store version provisionally and
                // prompt the user to choose store vs file (resolved by handleConflict).
                meta = res.store;
                syncState = 'synced';
                conflictPath = path;
            }
            title = meta.title;
            description = meta.description;
            keywords = meta.keywords;
            categories = meta.categories;
            releaseFilename = meta.releaseFilename;
            editorial = meta.editorial;
            matureContent = meta.matureContent;
            illustration = meta.illustration;
        } catch (e) {
            warn(`openMetadata failed: ${e}`);
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
        editorial = false;
        matureContent = false;
        illustration = false;
        snapshot = null;
        syncState = null;
        conflictPath = null;
        batchConflicts = [];
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
        editorial = snapshot.editorial;
        matureContent = snapshot.matureContent;
        illustration = snapshot.illustration;
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
            editorial,
            matureContent,
            illustration,
        };

        const newPath = await invoke<string>('save_metadata', {metadata});

        const prevFilepath = filepath;
        filepath = newPath;
        filename = extractStem(newPath);
        // The backend wrote the file and synced the store — reflect that in the status (feature 008).
        syncState = 'synced';
        snapshot = captureSnapshot();

        if (newPath !== prevFilepath) {
            onPathChange?.(newPath);
        }

        return newPath;
    }

    async function handleSave() {
        saveAttempted = true;
        saveError = null;
        attributeError = null;
        if (hasErrors) return;
        try {
            await doSave();
        } catch (e) {
            saveError = e instanceof Error ? e.message : String(e);
        }
    }

    // Cancel / revert-to-file (feature 008): discard app-only changes and restore the form + store
    // from the photo file (the store keeps its releaseFilename). Whether the record can be reverted.
    const canRevert = $derived(!isBatch && !!filepath && (syncState === 'appOnly' || isDirtyComputed));

    async function handleRevert() {
        if (!canRevert) return;
        const path = filepath;
        try {
            const meta = await revertToFile(path);
            if (filepath !== path) return;
            title = meta.title;
            description = meta.description;
            keywords = meta.keywords;
            categories = meta.categories;
            releaseFilename = meta.releaseFilename;
            editorial = meta.editorial;
            matureContent = meta.matureContent;
            illustration = meta.illustration;
            syncState = 'synced';
            saveError = null;
            attributeError = null;
            saveAttempted = false;
            snapshot = captureSnapshot();
        } catch (e) {
            warn(`revertToFile failed: ${e}`);
        }
    }

    // Conflict resolution (feature 008, US3). Idempotent: clears the pending state first so a
    // backdrop-dismiss (which defaults to "keep store") cannot double-resolve.
    async function handleConflict(source: 'store' | 'file') {
        const path = conflictPath;
        conflictPath = null;
        if (!path) return;
        try {
            const res = await applyMetadataSource(path, source);
            if (filepath !== path || res.kind !== 'resolved') return;
            const m = res.metadata;
            title = m.title;
            description = m.description;
            keywords = m.keywords;
            categories = m.categories;
            releaseFilename = m.releaseFilename;
            editorial = m.editorial;
            matureContent = m.matureContent;
            illustration = m.illustration;
            syncState = res.syncState;
            snapshot = captureSnapshot();
        } catch (e) {
            warn(`applyMetadataSource failed: ${e}`);
        }
    }

    // Batch conflict resolution: apply one choice to every conflicted file, then reload the batch.
    async function handleBatchConflict(source: 'store' | 'file') {
        const paths = batchConflicts;
        batchConflicts = [];
        if (paths.length === 0) return;
        await Promise.all(
            paths.map(p => applyMetadataSource(p, source).catch((e: unknown) => warn(`applyMetadataSource failed: ${e}`)))
        );
        loadBatchData(batchPaths);
    }

    // ── Footer error (resolved text + dismiss action, shared close/copy controls) ──

    interface FooterError {
        text: string;
        dismiss: () => void;
    }

    const footerError = $derived.by((): FooterError | null => {
        if (!isBatch && saveAttempted && hasErrors) {
            return {text: validationErrors.join(' · '), dismiss: () => { saveAttempted = false; }};
        }
        if (!isBatch && saveError) {
            return {text: `${t('metadata.error.saveFailed')}: ${saveError}`, dismiss: () => { saveError = null; }};
        }
        if (!isBatch && attributeError) {
            return {text: attributeError, dismiss: () => { attributeError = null; }};
        }
        if (isBatch && !isSaving && (batchFailed > 0 || batchCancelled > 0)) {
            const tail = batchCancelled > 0 ? ` · ${t('metadata.batch.error.cancelled', {n: batchCancelled})}` : '';
            const text = `${t('metadata.batch.error.failed', {n: batchFailed})}${tail} ${t('metadata.batch.error.of', {n: batchResults.size})}`;
            return {text, dismiss: () => { batchResults.clear(); }};
        }
        return null;
    });

    async function copyFooterError() {
        if (footerError) await writeText(footerError.text);
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

<svelte:window onkeydown={(e) => { if (showStockKeywords && e.key === 'Escape') { e.stopPropagation(); showStockKeywords = false; } }} />

<aside class="panel">
    <div class="panel-content">
        <h2 class="panel-title">{t('metadata.title')}</h2>

        <!-- ── File info ── -->
        {#if isBatch}
            <div class="file-info">
                <div class="file-status-row">
                    <span class="status-dot status-dot--batch"></span>
                    <span class="status-label status-label--batch">{t('metadata.fileStatus.batch')}</span>
                    <span class="file-basename">{tn('metadata.batch.fileCount', batchPaths.length)}</span>
                </div>
                {#if batchLoading}
                    <span class="file-path">{t('metadata.batch.loading')}</span>
                {/if}
            </div>
        {:else}
            <div class="file-info">
                <div class="file-status-row">
                    <span class="status-dot status-dot--{fileStatus}"></span>
                    <span class="status-label status-label--{fileStatus}">{t(`metadata.fileStatus.${fileStatus}` as MessageKey)}</span>
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
                <p class="group-label">{t('metadata.fieldGroup.fields')}</p>

                <!-- Title -->
                <div class="field">
                    <div class="field-header">
                        <label class="batch-apply-label">
                            <input type="checkbox" bind:checked={applyTitle} />
                            <span class="field-label">{t('metadata.field.title')}</span>
                        </label>
                        <span class="field-stats">{tn('metadata.stats.words', titleWords)} : {tn('metadata.stats.chars', titleChars)}</span>
                    </div>
                    <input
                        class="input"
                        type="text"
                        placeholder={batchTitleMixed ? t('metadata.batch.mixedValues') : t('metadata.field.title.placeholder')}
                        bind:value={batchTitle}
                        oninput={() => { if (batchTitle) applyTitle = true; }}
                    />
                </div>

                <!-- Description -->
                <div class="field">
                    <div class="field-header">
                        <label class="batch-apply-label">
                            <input type="checkbox" bind:checked={applyDescription} />
                            <span class="field-label">{t('metadata.field.description')}</span>
                        </label>
                        <span class="field-stats">{tn('metadata.stats.words', descWords)} : {tn('metadata.stats.chars', descChars)}</span>
                    </div>
                    <textarea
                        bind:this={descriptionEl}
                        class="input textarea"
                        placeholder={batchDescMixed ? t('metadata.batch.mixedValues') : t('metadata.field.description.placeholder')}
                        rows={4}
                        bind:value={batchDescription}
                        oninput={() => { if (batchDescription) applyDescription = true; }}
                    ></textarea>
                </div>

                <!-- Keywords (batch) -->
                <div class="field">
                    <span class="field-label">{t('metadata.field.keywords')} <span class="hint">— {t('metadata.field.keywords.hint')}</span></span>
                    <div class="kw-input-row">
                        <input
                            bind:this={batchKwInputEl}
                            class="input"
                            type="text"
                            placeholder={t('metadata.field.keywords.batch.placeholder')}
                            bind:value={batchKeywordInput}
                            onkeydown={handleBatchKeywordKeydown}
                            oninput={handleBatchKeywordInput}
                            onblur={() => suggestionsComp?.handleBlur()}
                        />
                        <button
                            class="kw-stock-btn"
                            onclick={() => { showStockKeywords = true; }}
                            title={t('metadata.keywords.optionalSection')}
                            aria-label={t('metadata.keywords.optionalSection')}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M2 2a1 1 0 0 1 1-1h4.586a1 1 0 0 1 .707.293l7 7a1 1 0 0 1 0 1.414l-4.586 4.586a1 1 0 0 1-1.414 0l-7-7A1 1 0 0 1 2 6.586V2zm3.5 4a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3z"/>
                            </svg>
                        </button>
                    </div>
                    <div class="keyword-actions">
                        <button
                            class="kw-action-btn"
                            onclick={copyBatchKeywords}
                            disabled={batchKeywordStates.filter(s => s.state === 'all').length === 0}
                            title={t('metadata.button.copy.batch.title')}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M4 2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V2zm2-1a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V2a1 1 0 0 0-1-1H6z"/>
                                <path d="M2 5a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-1h-1v1H2V6h1V5H2z"/>
                            </svg>
                            {t('metadata.button.copy')}
                        </button>
                        <button
                            class="kw-action-btn"
                            onclick={pasteBatchKeywords}
                            title={t('metadata.button.paste.title')}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z"/>
                                <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z"/>
                            </svg>
                            {t('metadata.button.paste')}
                        </button>
                        <button
                            class="kw-action-btn kw-action-btn--danger"
                            onclick={() => { showClearConfirm = true; }}
                            disabled={batchKeywordStates.length === 0}
                            title={t('metadata.button.clear.title')}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z"/>
                                <path fill-rule="evenodd" d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z"/>
                            </svg>
                            {t('metadata.button.clear')}
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
                                                title={t('metadata.chip.promoteToAll')}
                                            ></button>
                                        {/if}
                                        {item.word}
                                        <button
                                            class="chip-remove"
                                            onclick={() => removeBatchKeyword(item.word)}
                                            aria-label="{t('metadata.chip.removeKeyword')} {item.word}"
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
                <p class="group-label">{t('metadata.fieldGroup.required')}</p>

                <label class="field">
                    <span class="field-label">
                        {t('metadata.field.filename')} <span class="required">*</span>
                        <span class="hint">— {t('metadata.field.filename.hint')}</span>
                    </span>
                    <input
                        class="input"
                        class:input--invalid={saveAttempted && !filename.trim()}
                        type="text"
                        placeholder={t('metadata.field.filename.placeholder')}
                        bind:value={filename}
                    />
                </label>

                <label class="field">
                    <div class="field-header">
                        <span class="field-label">{t('metadata.field.title')} <span class="required">*</span></span>
                        <span class="field-stats">{tn('metadata.stats.words', titleWords)} : {tn('metadata.stats.chars', titleChars)}</span>
                    </div>
                    <input
                        class="input"
                        class:input--invalid={saveAttempted && !title.trim()}
                        type="text"
                        placeholder={t('metadata.field.title.placeholder')}
                        bind:value={title}
                    />
                </label>

                <label class="field">
                    <div class="field-header">
                        <span class="field-label">{t('metadata.field.description')} <span class="required">*</span></span>
                        <span class="field-stats">{tn('metadata.stats.words', descWords)} : {tn('metadata.stats.chars', descChars)}</span>
                    </div>
                    <textarea
                        bind:this={descriptionEl}
                        class="input textarea"
                        class:input--invalid={saveAttempted && !description.trim()}
                        placeholder={t('metadata.field.description.placeholder')}
                        rows={4}
                        bind:value={description}
                    ></textarea>
                </label>

                <div class="field">
                    <span class="field-label">
                        {t('metadata.field.keywords')} <span class="required">*</span>
                        <span class="hint">— {t('metadata.field.keywords.hint')}</span>
                    </span>
                    <div class="kw-input-row">
                        <input
                            bind:this={inputEl}
                            class="input"
                            class:input--invalid={saveAttempted && keywords.length === 0}
                            type="text"
                            placeholder={t('metadata.field.keywords.placeholder')}
                            bind:value={keywordInput}
                            onkeydown={handleKeywordKeydown}
                            oninput={handleKeywordInput}
                            onblur={() => suggestionsComp?.handleBlur()}
                        />
                        <button
                            class="kw-stock-btn"
                            onclick={() => { showStockKeywords = true; }}
                            title={t('metadata.keywords.optionalSection')}
                            aria-label={t('metadata.keywords.optionalSection')}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M2 2a1 1 0 0 1 1-1h4.586a1 1 0 0 1 .707.293l7 7a1 1 0 0 1 0 1.414l-4.586 4.586a1 1 0 0 1-1.414 0l-7-7A1 1 0 0 1 2 6.586V2zm3.5 4a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3z"/>
                            </svg>
                        </button>
                    </div>
                    <div class="keyword-actions">
                        <button
                            class="kw-action-btn"
                            onclick={copyKeywords}
                            disabled={keywords.length === 0}
                            title={t('metadata.button.copy.title')}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M4 2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V2zm2-1a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V2a1 1 0 0 0-1-1H6z"/>
                                <path d="M2 5a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-1h-1v1H2V6h1V5H2z"/>
                            </svg>
                            {t('metadata.button.copy')}
                        </button>
                        <button
                            class="kw-action-btn"
                            onclick={pasteKeywords}
                            title={t('metadata.button.paste.title')}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z"/>
                                <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z"/>
                            </svg>
                            {t('metadata.button.paste')}
                        </button>
                        <button
                            class="kw-action-btn kw-action-btn--danger"
                            onclick={() => { showClearConfirm = true; }}
                            disabled={keywords.length === 0}
                            title={t('metadata.button.clear.title')}
                        >
                            <svg viewBox="0 0 16 16" fill="currentColor">
                                <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z"/>
                                <path fill-rule="evenodd" d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z"/>
                            </svg>
                            {t('metadata.button.clear')}
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
                                            aria-label="{t('metadata.chip.removeKeyword')} {item}"
                                        >×</button>
                                    </span>
                                {/if}
                            {/each}
                        </div>
                    {/if}
                </div>
            </section>
        {/if}

        <!-- ── Optional fields ── -->
        <details
            class="optional-details"
            open={optionalOpen}
            ontoggle={(e) => optionalOpen = (e.currentTarget as HTMLDetailsElement).open}
        >
            <summary class="optional-summary">
                <span class="group-label" style="border: none; padding: 0;">{t('metadata.optional.section')}</span>
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
                                <span class="field-label">{t('metadata.field.categories')}</span>
                            </label>
                        </div>
                        <input
                            class="input"
                            type="text"
                            placeholder={batchCatMixed ? t('metadata.batch.mixedValues') : t('metadata.field.categories.batch.placeholder')}
                            bind:value={batchCategories}
                            oninput={() => { if (batchCategories) applyCategories = true; }}
                        />
                    </div>
                {:else}
                    <label class="field">
                        <span class="field-label">{t('metadata.field.categories')}</span>
                        <input class="input" type="text" placeholder={t('metadata.field.categories.placeholder')} bind:value={categories} />
                    </label>
                    <label class="field">
                        <span class="field-label">{t('metadata.field.releaseFilename')}</span>
                        <input class="input" type="text" placeholder={t('metadata.field.releaseFilename.placeholder')} bind:value={releaseFilename} />
                    </label>
                    <div class="field flags-row">
                        <label class="flag-check">
                            <input type="checkbox" bind:checked={editorial} />
                            <span class="field-label">{t('metadata.field.editorial')}</span>
                        </label>
                        <label class="flag-check">
                            <input type="checkbox" bind:checked={matureContent} />
                            <span class="field-label">{t('metadata.field.matureContent')}</span>
                        </label>
                        <label class="flag-check">
                            <input type="checkbox" bind:checked={illustration} />
                            <span class="field-label">{t('metadata.field.illustration')}</span>
                        </label>
                    </div>
                {/if}
            </div>
        </details>
    </div>

    {#if showClearConfirm}
        <ConfirmDialog
            title={t('metadata.dialog.clearKeywords.title')}
            body={isBatch
                ? tn('metadata.dialog.clearKeywords.batch.body', batchPaths.length)
                : t('metadata.dialog.clearKeywords.body')}
            icon="warning"
            buttons={[
                {label: t('common.cancel'), onClick: () => { showClearConfirm = false; }},
                {
                    label: t('metadata.button.clearAll'),
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

    {#if showRevertConfirm}
        <ConfirmDialog
            title={t('metadata.dialog.revert.title')}
            body={t('metadata.dialog.revert.body')}
            icon="warning"
            buttons={[
                {label: t('common.cancel'), onClick: () => { showRevertConfirm = false; }},
                {
                    label: t('metadata.button.revert'),
                    onClick: () => { showRevertConfirm = false; handleRevert(); },
                    color: 'var(--required-color)',
                    border: 'var(--required-color)',
                    hoverBg: 'var(--required-alpha-08)',
                    hoverBorder: 'var(--required-color)',
                    hoverColor: 'var(--required-color)',
                },
            ]}
            onClose={() => { showRevertConfirm = false; }}
        />
    {/if}

    {#if conflictPath}
        <ConfirmDialog
            title={t('metadata.dialog.conflict.title')}
            body={t('metadata.dialog.conflict.body')}
            icon="warning"
            buttons={[
                {label: t('metadata.button.useFile'), onClick: () => handleConflict('file')},
                {
                    label: t('metadata.button.keepApp'),
                    onClick: () => handleConflict('store'),
                    bg: 'var(--accent)', color: '#fff', border: 'var(--accent)',
                    hoverBg: 'var(--accent-hover)', hoverBorder: 'var(--accent-hover)',
                },
            ]}
            onClose={() => handleConflict('store')}
        />
    {/if}

    {#if batchConflicts.length > 0}
        <ConfirmDialog
            title={t('metadata.dialog.conflict.title')}
            body={tn('metadata.dialog.conflict.batch.body', batchConflicts.length)}
            icon="warning"
            buttons={[
                {label: t('metadata.button.useFile'), onClick: () => handleBatchConflict('file')},
                {
                    label: t('metadata.button.keepApp'),
                    onClick: () => handleBatchConflict('store'),
                    bg: 'var(--accent)', color: '#fff', border: 'var(--accent)',
                    hoverBg: 'var(--accent-hover)', hoverBorder: 'var(--accent-hover)',
                },
            ]}
            onClose={() => handleBatchConflict('store')}
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
        {#if footerError}
            <div class="footer-errors">
                <span class="footer-error-text">{footerError.text}</span>
                <div class="footer-error-actions">
                    <button
                        class="footer-error-btn"
                        onclick={copyFooterError}
                        title={t('metadata.error.copy')}
                        aria-label={t('metadata.error.copy')}
                    >
                        <svg viewBox="0 0 16 16" fill="currentColor">
                            <path d="M4 2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V2zm2-1a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V2a1 1 0 0 0-1-1H6z"/>
                            <path d="M2 5a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-1h-1v1H2V6h1V5H2z"/>
                        </svg>
                    </button>
                    <button
                        class="footer-error-btn"
                        onclick={footerError.dismiss}
                        title={t('metadata.error.dismiss')}
                        aria-label={t('metadata.error.dismiss')}
                    >
                        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                            <path d="M4 4l8 8M12 4l-8 8"/>
                        </svg>
                    </button>
                </div>
            </div>
        {/if}
        <div class="footer-controls">
            {#if isBatch}
                {#if isSaving}
                    <button
                        class="btn-ghost"
                        onclick={cancelBatchSave}
                        disabled={batchCancelling}
                    >
                        {batchCancelling ? t('metadata.button.cancel.batch.progress') : t('metadata.button.cancel.batch')}
                    </button>
                {/if}
                <div class="footer-actions">
                    <button
                        class="btn-ghost"
                        onclick={handleBatchAttribute}
                        disabled={!ollama.available || isSaving || batchLoading}
                        title={ollama.available ? t('ollama.attribute.tooltip') : t('ollama.unavailable.tooltip')}
                    >{t('ollama.attribute')}</button>
                    <button
                        class="btn-primary save-btn"
                        onclick={handleBatchSave}
                        disabled={isSaving || batchLoading}
                    >
                        {isSaving ? t('metadata.button.saveBatch.progress', {n: savingCount, total: savingTotal}) : tn('metadata.button.saveBatch', batchPaths.length)}
                    </button>
                </div>
            {:else}
                <label class="autosave-toggle">
                    <input
                        type="checkbox"
                        checked={autoSave as boolean}
                        onchange={(e) => settings.set('editor.autosave', e.currentTarget.checked)}
                    />
                    <span>{t('metadata.button.autosave')}</span>
                </label>
                <div class="footer-actions">
                    <button
                        class="btn-ghost"
                        onclick={handleAttribute}
                        disabled={!ollama.available || !filepath}
                        title={ollama.available ? t('ollama.attribute.tooltip') : t('ollama.unavailable.tooltip')}
                    >{t('ollama.attribute')}</button>
                    <button
                        class="btn-ghost"
                        onclick={() => { showRevertConfirm = true; }}
                        disabled={!canRevert}
                        title={t('metadata.button.revert.title')}
                    >{t('metadata.button.revert')}</button>
                    <button
                        class="btn-primary save-btn"
                        onclick={handleSave}
                        disabled={!filepath}
                    >{t('metadata.button.saveChanges')}</button>
                </div>
            {/if}
        </div>
    </footer>
</aside>

<!-- ── Stock-keyword presets popup (triggered from the keyword actions) ── -->
{#if showStockKeywords}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="sk-overlay" role="presentation" onclick={() => { showStockKeywords = false; }} onkeydown={() => {}}>
        <div
            class="sk-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={t('metadata.keywords.optionalSection')}
            tabindex="-1"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.key !== 'Escape' && e.stopPropagation()}
        >
            <div class="sk-header">
                <span class="sk-title">{t('metadata.keywords.optionalSection')}</span>
                <button class="sk-close" onclick={() => { showStockKeywords = false; }} aria-label={t('common.close')}>✕</button>
            </div>
            <div class="sk-body presets">
                {#each Object.entries(presets) as [group, tags]}
                    <div class="preset-group">
                        <!-- Category labels are localized; the keyword VALUES below stay English (FR-014). -->
                        <span class="preset-group-label">{t(`metadata.keywords.stockKeywords.${group.toLowerCase()}` as MessageKey)}</span>
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
        </div>
    </div>
{/if}

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
        &--app { background: #a78bfa; box-shadow: 0 0 5px rgba(167, 139, 250, 0.4); }
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
        &--app { color: #a78bfa; }
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

    // Keyword input + stock-presets icon button on one row
    .kw-input-row {
        @include flex(row, flex-start, stretch);
        gap: 6px;

        .input { flex: 1; min-width: 0; }
    }

    .kw-stock-btn {
        @include btn-reset;
        @include flex(row, center, center);
        flex-shrink: 0;
        aspect-ratio: 1;
        border: 1px solid $border;
        border-radius: $radius-sm;
        background: $bg-surface;
        color: $text-secondary;
        @include transition(background, color, border-color);

        svg { width: 14px; height: 14px; }

        &:hover {
            background: var(--hover-bg-strong);
            color: $text;
            border-color: $text-muted;
        }
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

    // ── Stock-keyword presets popup ──
    .sk-overlay {
        position: fixed;
        inset: 0;
        background: var(--overlay-bg);
        backdrop-filter: blur(3px);
        @include flex(row, center, center);
        z-index: 500;
    }

    .sk-dialog {
        background: $bg-panel;
        border: 1px solid $border;
        border-radius: $radius-md;
        width: 460px;
        max-width: calc(100vw - 48px);
        max-height: 80vh;
        @include flex(column, flex-start, stretch);
        box-shadow: 0 12px 40px var(--shadow-heavy);
        overflow: hidden;
    }

    .sk-header {
        @include flex(row, space-between, center);
        padding: 12px 16px;
        border-bottom: 1px solid $border;
        flex-shrink: 0;
    }

    .sk-title {
        font-size: $fs-regular;
        font-weight: 600;
        color: $text;
    }

    .sk-close {
        @include btn-reset;
        @include transition(color);
        color: $text-muted;
        font-size: $fs-small;
        width: 24px;
        height: 24px;
        @include flex(row, center, center);
        border-radius: $radius-sm;

        &:hover { color: $text; }
    }

    .sk-body {
        padding: 14px 16px;
        overflow-y: auto;
        @include flex(column, flex-start, stretch);
        @include scrollbar;
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

    // ── Attribution flag checkboxes (inline row) ──
    .flags-row {
        @include flex(row, flex-start, center);
        flex-wrap: wrap;
        gap: 16px;
    }

    .flag-check {
        @include flex(row, flex-start, center);
        gap: 6px;
        cursor: pointer;

        input {
            width: 14px;
            height: 14px;
            flex-shrink: 0;
            cursor: pointer;
            accent-color: $accent;
        }

        .field-label { font-weight: 400; cursor: pointer; }
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
        @include flex(row, flex-start, flex-start);
        gap: 8px;
        padding: 6px 16px;
        font-size: $fs-footnote1;
        color: $required-color;
        border-bottom: 1px solid var(--required-alpha-20);
        background: var(--required-alpha-06);
        line-height: 1.5;
    }

    .footer-error-text {
        flex: 1;
        min-width: 0;
        word-break: break-word;
    }

    .footer-error-actions {
        @include flex(row, flex-start, center);
        gap: 2px;
        flex-shrink: 0;
    }

    .footer-error-btn {
        @include btn-reset;
        @include flex(row, center, center);
        padding: 3px;
        border-radius: $radius-sm;
        color: $required-color;
        cursor: pointer;
        opacity: 0.7;
        @include transition(opacity, background);

        svg {
            width: 13px;
            height: 13px;
        }

        &:hover {
            opacity: 1;
            background: var(--required-alpha-08);
        }
    }

    .footer-controls {
        @include flex(row, flex-start, center);
        gap: 12px;
        padding: 0 16px;
        flex: 1;
        min-height: $footer-height;

        button { white-space: nowrap; }
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

    // Ollama attribute + Save grouped together on the right of the footer.
    .footer-actions {
        @include flex(row, flex-end, center);
        gap: 8px;
        margin-left: auto;
    }

    .save-btn {
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
