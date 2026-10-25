// Frontend glue for the AI backend commands: typed invoke wrappers + AttributionConfig assembly.
import {invoke, Channel} from '@tauri-apps/api/core';
import {settings} from '$lib/settings';
import type {BatchProgress, ItemStatus, PullProgress} from '$lib/events';
import {DEFAULT_PROMPT} from './defaults';
import {activeModelKey, baseUrlKey, DEFAULT_PROVIDER, providerInfo, type ProfileProviderId, type ProviderId} from './providers';

export interface ProviderStatus {
    installed: boolean;
    reachable: boolean;
    version: string | null;
    detail: string | null;
}

export interface ModelInfo {
    name: string;
    size: number;
}

export interface ChatGptAccount {
    email: string | null;
}

export interface AttributionResult {
    title: string;
    description: string;
    keywords: string[];
    categories: string[];
    editorial: boolean;
    matureContent: boolean;
    illustration: boolean;
}

export interface AttributionConfig {
    provider: ProviderId;
    baseUrl: string;
    model: string;
    prompt: string;
    think?: boolean | string | null;
    keepAlive?: string | null;
    options: Record<string, unknown>;
    format: unknown;
}

/** A per-model attribution profile, persisted in `ai.modelProfiles`. Profiles saved before providers existed have no `provider` and belong to Ollama.
 *  A profile of the `base` pseudo-provider applies to every provider. */
export interface ModelProfile {
    id: string;
    name: string;
    provider?: ProfileProviderId;
    modelId: string;
    prompt: string;
    think?: boolean | string | null;
    keepAlive?: string | null;
    options: Record<string, number | string | boolean>;
}

/** The provider a profile belongs to (`base` for a profile that applies to every provider). */
export function profileProvider(p: ModelProfile): ProfileProviderId {
    return p.provider === 'base' ? 'base' : providerInfo(p.provider ?? DEFAULT_PROVIDER).id;
}

/** The provider selected for attribution. */
export function activeProvider(): ProviderId {
    return providerInfo(settings.get<string>('ai.provider')).id;
}

/** The configured base URL of `provider` ('' for providers without one). */
export function baseUrlOf(provider: ProviderId): string {
    const key = baseUrlKey(provider);
    return key ? settings.get<string>(key) ?? '' : '';
}

/** The active model of `provider`. */
export function activeModelOf(provider: ProviderId): string {
    return settings.get<string>(activeModelKey(provider)) ?? '';
}

export function providerStatus(provider: ProviderId): Promise<ProviderStatus> {
    return invoke('ai_status', {provider, baseUrl: baseUrlOf(provider)});
}

export function listModels(provider: ProviderId): Promise<ModelInfo[]> {
    return invoke('ai_list_models', {provider, baseUrl: baseUrlOf(provider)});
}

/** Stores the provider's API key in the OS keychain; an empty key removes it. */
export function setApiKey(provider: ProviderId, key: string): Promise<void> {
    return invoke('ai_set_api_key', {provider, key});
}

export function hasApiKey(provider: ProviderId): Promise<boolean> {
    return invoke('ai_has_api_key', {provider});
}

/** Opens the browser on OpenAI's sign-in page and resolves once the sign-in completes (cancelable via `cancelAi`).
 *  `newAccount` registers the app again, for signing in with a different ChatGPT account or workspace.
 *  `onLink` receives the sign-in page's address, for opening it in another browser. */
export function chatgptLogin(newAccount = false, onLink: (url: string) => void = () => {}): Promise<ChatGptAccount> {
    const channel = new Channel<string>();
    channel.onmessage = onLink;
    return invoke('chatgpt_login', {newAccount, onLink: channel});
}

export function chatgptLogout(): Promise<void> {
    return invoke('chatgpt_logout');
}

export function chatgptAccount(): Promise<ChatGptAccount | null> {
    return invoke('chatgpt_account');
}

export function pullModel(model: string, onProgress: (p: PullProgress) => void): Promise<void> {
    const channel = new Channel<PullProgress>();
    channel.onmessage = onProgress;
    return invoke('ollama_pull_model', {baseUrl: baseUrlOf('ollama'), model, onProgress: channel});
}

export function installOllama(): Promise<void> {
    return invoke('install_ollama');
}

/** Cancels the in-flight model pull, ChatGPT sign-in or attribution. */
export function cancelAi(): Promise<void> {
    return invoke('ai_cancel');
}

export function attributePhoto(path: string): Promise<AttributionResult> {
    return invoke('attribute_photo', {path, config: attributionConfig()});
}

export function attributeBatch(paths: string[], onProgress: (p: BatchProgress) => void): Promise<ItemStatus[]> {
    const channel = new Channel<BatchProgress>();
    channel.onmessage = onProgress;
    return invoke('attribute_batch', {paths, config: attributionConfig(), onProgress: channel});
}

/** Append the response schema (in a fenced code block, after a configurable heading) to the prompt, so
 *  models that don't strictly honor structured output still see the expected JSON shape. */
function appendAnswerFormat(prompt: string, schemaText: string): string {
    const schema = schemaText.trim();
    if (!schema) return prompt;
    const heading = (settings.get<string>('ai.answerFormatLabel') ?? '').trim();
    const lead = heading ? `${heading}\n` : '';
    return `${prompt}\n\n${lead}\`\`\`json\n${schema}\n\`\`\``;
}

/** The profile for `model` on `provider`, most specific first: the provider's profile for the model, a Base (all providers) profile for the model, the provider's "base" fallback, then the Base provider's "base" fallback. */
export function profileFor(provider: ProviderId, model: string): ModelProfile | undefined {
    const profiles = settings.get<ModelProfile[]>('ai.modelProfiles') ?? [];
    const find = (owner: ProfileProviderId, modelId: string) => profiles.find(p => profileProvider(p) === owner && p.modelId === modelId);
    return find(provider, model) ?? find('base', model) ?? find(provider, 'base') ?? find('base', 'base');
}

/** Assemble the per-request config from settings + the profile matching the active provider and model. */
export function attributionConfig(): AttributionConfig {
    const provider = activeProvider();
    const model = activeModelOf(provider);
    const profile = profileFor(provider, model);

    const schemaText = settings.get<string>('ai.responseFormat') ?? '';
    let format: unknown = {};
    try {
        format = JSON.parse(schemaText);
    } catch {
        format = {};
    }

    const basePrompt = profile?.prompt?.trim() || DEFAULT_PROMPT;

    return {
        provider,
        baseUrl: baseUrlOf(provider),
        model,
        // The schema is both enforced via the provider's structured-output field and appended to the prompt text so lenient models that ignore it still receive the expected JSON structure.
        prompt: appendAnswerFormat(basePrompt, schemaText),
        think: profile?.think ?? null,
        keepAlive: provider === 'ollama' ? profile?.keepAlive ?? null : null,
        options: profile?.options ?? {},
        format
    };
}
