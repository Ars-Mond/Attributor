// Frontend glue for the Ollama backend commands: typed invoke wrappers + AttributionConfig assembly.
import {invoke, Channel} from '@tauri-apps/api/core';
import {settings} from '$lib/settings';
import type {BatchProgress, ItemStatus, PullProgress} from '$lib/events';

export interface OllamaStatus {
    installed: boolean;
    reachable: boolean;
    version: string | null;
}

export interface OllamaModel {
    name: string;
    size: number;
}

export interface AttributionResult {
    title: string;
    description: string;
    keywords: string[];
    categories: string[];
}

export interface AttributionConfig {
    baseUrl: string;
    model: string;
    prompt: string;
    think?: boolean | string | null;
    keepAlive?: string | null;
    options: Record<string, unknown>;
    format: unknown;
}

/** A per-model attribution profile, persisted in `ollama.modelProfiles`. */
export interface ModelProfile {
    id: string;
    name: string;
    modelId: string;
    prompt: string;
    think?: boolean | string | null;
    keepAlive?: string | null;
    options: Record<string, number | string | boolean>;
}

function baseUrl(): string {
    return settings.get<string>('ollama.baseUrl');
}

export function ollamaStatus(): Promise<OllamaStatus> {
    return invoke('ollama_status', {baseUrl: baseUrl()});
}

export function listModels(): Promise<OllamaModel[]> {
    return invoke('ollama_list_models', {baseUrl: baseUrl()});
}

export function pullModel(model: string, onProgress: (p: PullProgress) => void): Promise<void> {
    const channel = new Channel<PullProgress>();
    channel.onmessage = onProgress;
    return invoke('ollama_pull_model', {baseUrl: baseUrl(), model, onProgress: channel});
}

export function installOllama(): Promise<void> {
    return invoke('install_ollama');
}

export function cancelOllama(): Promise<void> {
    return invoke('ollama_cancel');
}

export function attributePhoto(path: string): Promise<AttributionResult> {
    return invoke('attribute_photo', {path, config: attributionConfig()});
}

export function attributeBatch(paths: string[], onProgress: (p: BatchProgress) => void): Promise<ItemStatus[]> {
    const channel = new Channel<BatchProgress>();
    channel.onmessage = onProgress;
    return invoke('attribute_batch', {paths, config: attributionConfig(), onProgress: channel});
}

// Minimal placeholder prompt used when the active model has no profile. The user supplies the real
// default prompt / per-model profiles (deferred content); this keeps attribution functional meanwhile.
const FALLBACK_PROMPT =
    'Analyze this stock photo and respond with JSON matching the schema: a concise title, a detailed ' +
    'description, relevant keywords, and broad categories.';

/** Assemble the per-request config from settings + the profile matching the active model. */
export function attributionConfig(): AttributionConfig {
    const model = settings.get<string>('ollama.activeModel');
    const profiles = settings.get<ModelProfile[]>('ollama.modelProfiles') ?? [];
    const profile = profiles.find(p => p.modelId === model);

    let format: unknown = {};
    try {
        format = JSON.parse(settings.get<string>('ollama.responseFormat'));
    } catch {
        format = {};
    }

    return {
        baseUrl: baseUrl(),
        model,
        prompt: profile?.prompt?.trim() || FALLBACK_PROMPT,
        think: profile?.think ?? null,
        keepAlive: profile?.keepAlive ?? null,
        options: profile?.options ?? {},
        format
    };
}
