// Window side of the MCP tools.
// Through `mcp-request` events the backend asks for state only the window holds (the open folder, the selection, settings, the AI configuration), has the editor keep its pending edits before metadata changes, and has the window show the result; each request is answered with `mcp_ui_reply`.
import {invoke} from '@tauri-apps/api/core';
import type {UnlistenFn} from '@tauri-apps/api/event';
import {listenEvent, EVENT} from '$lib/events';
import type {FileNode} from '$lib/types';
import {panelState, cacheGenConfig} from '$lib/panel/filesPanelStore.svelte';
import {settings} from '$lib/settings';
import {t, type MessageKey} from '$lib/i18n';
import {ai} from '$lib/ai/availability.svelte';
import {activeModelOf, attributionConfig, listModels} from '$lib/ai/ai';
import {progress} from '$lib/progress.svelte';
import {PROVIDERS, providerInfo, type ProviderId} from '$lib/ai/providers';
import {checkSetting} from './validate';

/** The editor operations the bridge needs (implemented by MetadataPanel). */
export interface McpEditor {
    openPath(): string | null;
    flush(): Promise<void>;
    hasPendingRename(): boolean;
    refresh(path: string): Promise<void>;
    followRename(from: string, to: string): void;
    reloadBatch(paths: string[]): void;
}

/** What the page lends the bridge: its panels and the handlers its own controls use. */
export interface McpWindow {
    editor(): McpEditor | null;
    currentPath(): string | null;
    batchPaths(): string[];
    isDirty(): boolean;
    openFile(path: string): Promise<void>;
    folderOpened(path: string): void;
    selectionChanged(paths: string[]): void;
    pathChanged(path: string): void;
}

type Params = Record<string, unknown>;
type Handler = (params: Params) => unknown;

// Settings sections whose keys MCP may change only with the user's permission (Settings > MCP).
const AI_SECTIONS = new Set(['ai', 'model-profiles']);
// Longest rescan pause without a new write, should a job never release it.
const GUARD_LIMIT_MS = 5 * 60 * 1000;
// Longest wait for the AI settings page's follow-up (status and models) after a provider change.
const AI_INIT_WAIT_MS = 10_000;
const SELECTED_LIMIT = 200;

function isImageName(name: string): boolean {
    return /\.(jpg|jpeg|png|webp)$/i.test(name);
}

/** The open folder's images in the order the file list shows them. */
function imagePaths(tree: FileNode | null): string[] {
    const out: string[] = [];
    const walk = (node: FileNode) => {
        for (const child of node.children) {
            if (child.is_dir) walk(child);
            else if (isImageName(child.name)) out.push(child.path);
        }
    };
    if (tree) walk(tree);
    return out;
}

function unavailableReason(id: ProviderId): string {
    const info = providerInfo(id);
    if (!activeModelOf(id)) return `no model is selected for ${info.label}`;
    if (info.auth === 'key') return `the ${info.label} API key is not set`;
    if (info.auth === 'oauth') return 'Attributor is not signed in to ChatGPT';
    if (id === 'ollama') return 'Ollama is not installed';
    return `${info.label} is not ready`;
}

function createHandlers(w: McpWindow): Record<string, Handler> {
    let guardTimer: ReturnType<typeof setTimeout> | null = null;

    // Pauses the watcher-driven rescans while many files are written, as the batch Save does; every write extends the pause.
    function holdRescans() {
        if (guardTimer) clearTimeout(guardTimer);
        panelState.mcpWriting = true;
        guardTimer = setTimeout(() => {panelState.mcpWriting = false; guardTimer = null;}, GUARD_LIMIT_MS);
    }

    // The folder-changed events of the last writes arrive after the job ends, so the pause outlives it briefly.
    function releaseRescans() {
        if (guardTimer) clearTimeout(guardTimer);
        guardTimer = setTimeout(() => {panelState.mcpWriting = false; guardTimer = null;}, 1000);
    }

    async function flushEditor(paths?: string[]) {
        const editor = w.editor();
        const open = editor?.openPath();
        if (editor && open && (!paths || paths.includes(open))) await editor.flush();
    }

    // Switching the file or folder under the user's blocking operation (single-image attribution, batch save) would hand its result to another image.
    function refuseWhileBusy() {
        if (progress.blocking) throw new Error('Attributor is busy with an operation the user started; try again when it finishes');
    }

    function refuseOnPendingRename() {
        if (w.editor()?.hasPendingRename()) {
            throw new Error('the editor has an unsaved file name change; ask the user to save or revert it first');
        }
    }

    return {
        status() {
            const tree = panelState.fileTree;
            const batch = w.batchPaths();
            const selected = [...panelState.selectedPaths];
            const provider = ai.provider;
            return {
                folder: tree?.path ?? null,
                imageCount: imagePaths(tree).length,
                activeImage: batch.length > 1 ? null : w.currentPath(),
                selectedCount: selected.length,
                selectedImages: selected.slice(0, SELECTED_LIMIT),
                batchMode: batch.length > 1,
                editor: {open: w.editor() !== null, unsavedChanges: w.isDirty()},
                ai: {provider, providerLabel: ai.label, model: activeModelOf(provider) || null, available: ai.available}
            };
        },

        list_images() {
            const tree = panelState.fileTree;
            return {folder: tree?.path ?? null, images: imagePaths(tree)};
        },

        async prepare(p) {
            if (p.guard === true) holdRescans();
            await flushEditor(p.paths as string[]);
            return null;
        },

        async refresh(p) {
            const paths = p.paths as string[];
            const renamed = (p.renamed as {from: string; to: string}[]) ?? [];
            const editor = w.editor();
            for (const {from, to} of renamed) {
                if (w.batchPaths().length > 1) {
                    // Batch mode: keep the selection, with the renamed file under its new path.
                    if (!panelState.selectedPaths.has(from)) continue;
                    const selected = [...panelState.selectedPaths].map(s => (s === from ? to : s));
                    panelState.selectedPaths = new Set(selected);
                    if (panelState.activePath === from) panelState.activePath = to;
                    if (panelState.anchorPath === from) panelState.anchorPath = to;
                    w.selectionChanged(selected);
                } else if (editor?.openPath() === from) {
                    editor.followRename(from, to);
                    w.pathChanged(to);
                }
            }
            const open = editor?.openPath();
            if (editor && open && paths.includes(open)) await editor.refresh(open);
            if (p.batch !== false) editor?.reloadBatch([...paths, ...renamed.map(r => r.from)]);
            if (p.release === true) releaseRescans();
            return null;
        },

        async open_folder(p) {
            refuseWhileBusy();
            if (w.batchPaths().length <= 1) refuseOnPendingRename();
            await flushEditor();
            const tree = await invoke<FileNode>('open_folder_path', {path: p.path, gen: cacheGenConfig()});
            panelState.fileTree = tree;
            panelState.selectedPaths = new Set();
            panelState.activePath = '';
            panelState.anchorPath = '';
            w.folderOpened(tree.path);
            return {folder: tree.path, imageCount: imagePaths(tree).length};
        },

        async select(p) {
            const paths = p.paths as string[];
            refuseWhileBusy();
            if (!panelState.fileTree) throw new Error('no folder is open in Attributor; call open_folder first');
            const known = new Set(imagePaths(panelState.fileTree));
            const unknown = paths.filter(path => !known.has(path));
            if (unknown.length > 0) {
                throw new Error(`not in the open folder (use paths from list_images): ${unknown.slice(0, 5).join(', ')}${unknown.length > 5 ? ', …' : ''}`);
            }
            if (w.batchPaths().length <= 1) {
                refuseOnPendingRename();
                await flushEditor();
            }
            panelState.selectedPaths = new Set(paths);
            panelState.anchorPath = paths[0];
            panelState.activePath = paths[paths.length - 1];
            if (paths.length === 1) await w.openFile(paths[0]);
            w.selectionChanged(paths);
            return {selected: paths.length, mode: paths.length === 1 ? 'single' : 'batch'};
        },

        get_settings() {
            return {
                settings: settings.entries().map(({section, descriptor: d}) => {
                    const providers = d.key === 'ai.provider' ? PROVIDERS.map(p => ({value: p.id, label: p.label})) : undefined;
                    const options = d.options?.map(o => ({value: o.value, label: d.localizeOptions ? t(o.label as MessageKey) : o.label}));
                    return {
                        key: d.key,
                        section,
                        type: d.type,
                        label: t(d.label as MessageKey),
                        description: d.description ? t(d.description as MessageKey) : undefined,
                        value: settings.get(d.key),
                        default: d.default,
                        min: d.min,
                        max: d.max,
                        options: providers ?? options,
                        aiSetting: AI_SECTIONS.has(section)
                    };
                })
            };
        },

        async update_settings(p) {
            const values = p.values as Record<string, unknown>;
            const allowAi = p.allowAi === true;
            const known = new Map(settings.entries().map(e => [e.descriptor.key, e]));
            const errors: string[] = [];
            const changes: [string, unknown][] = [];
            for (const [key, value] of Object.entries(values)) {
                const entry = known.get(key);
                if (!entry) {
                    errors.push(`${key}: unknown setting`);
                    continue;
                }
                if (AI_SECTIONS.has(entry.section) && !allowAi) {
                    errors.push(`${key}: changing AI settings through MCP is turned off`);
                    continue;
                }
                const checked = checkSetting(entry.descriptor, value);
                if (checked.ok) changes.push([key, checked.value]);
                else errors.push(`${key} ${checked.error}`);
            }
            if (errors.length > 0) throw new Error(`nothing was changed: ${errors.join('; ')}`);
            for (const [key, value] of changes) settings.set(key, value);
            // As on the AI settings page: another provider, address or model needs a fresh status and model list.
            // The answer does not wait long for it: a slow cloud provider must not turn applied settings into a failed call.
            const active = ai.provider;
            if (changes.some(([key]) => key === 'ai.provider' || key.startsWith(`${active}.`))) {
                await Promise.race([ai.init(active), new Promise(resolve => setTimeout(resolve, AI_INIT_WAIT_MS))]);
            }
            return {applied: Object.fromEntries(changes)};
        },

        attribution_config() {
            const provider = ai.provider;
            const available = ai.available;
            return {
                config: attributionConfig(),
                available,
                reason: available ? null : unavailableReason(provider),
                provider,
                providerLabel: ai.label,
                model: activeModelOf(provider) || null
            };
        },

        async list_models(p) {
            const id = typeof p.provider === 'string' ? providerInfo(p.provider).id : ai.provider;
            const models = await listModels(id);
            return {provider: id, providerLabel: providerInfo(id).label, activeModel: activeModelOf(id) || null, models};
        }
    };
}

/** Starts answering MCP requests and tells the backend the window is ready. */
export async function startMcpBridge(w: McpWindow): Promise<UnlistenFn> {
    const handlers = createHandlers(w);
    const unlisten = await listenEvent(EVENT.mcpRequest, async (request) => {
        try {
            const handler = handlers[request.op];
            if (!handler) throw new Error(`unknown window operation ${request.op}`);
            const value = await handler((request.params ?? {}) as Params);
            await invoke('mcp_ui_reply', {id: request.id, ok: true, value: value ?? null});
        } catch (e) {
            const error = e instanceof Error ? e.message : String(e);
            await invoke('mcp_ui_reply', {id: request.id, ok: false, error}).catch(() => {});
        }
    });
    await invoke('mcp_ui_ready');
    return unlisten;
}
