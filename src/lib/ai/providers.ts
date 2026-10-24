// Provider catalog: ids, defaults and capabilities that drive the AI settings page, the profile dialog and request assembly.
// Each provider persists its own `<id>.baseUrl` and `<id>.activeModel` settings (Ollama's keys predate the other providers).
import type {MessageKey} from '$lib/i18n/types';
import {CHATGPT_MODELS, CLAUDE_MODELS, OLLAMA_MODELS, OPENAI_MODELS, type KnownModel} from './models';

export type ProviderId = 'ollama' | 'llamacpp' | 'lmstudio' | 'jan' | 'vllm' | 'unsloth' | 'openai' | 'chatgpt' | 'anthropic';

/** `ollama`: native Ollama API; `local`: a local OpenAI-compatible server; `cloud`: a remote API. */
export type ProviderKind = 'ollama' | 'local' | 'cloud';

/** How requests authenticate: no credentials, an optional or required API key (kept in the OS keychain), or ChatGPT sign-in. */
export type AuthMode = 'none' | 'optionalKey' | 'key' | 'oauth';

export interface ProviderInfo {
    id: ProviderId;
    label: string;                  // brand name, never translated
    kind: ProviderKind;
    auth: AuthMode;
    defaultBaseUrl: string | null;  // null: the provider has no configurable address
    baseUrlHint?: MessageKey;       // extra note under the base URL field
    keyUrl?: string;                // where the user creates an API key
    keyHint?: MessageKey;           // where the key comes from, shown under the API key field
    thinkOptions: string[];         // values offered in the profile's thinking field besides "—"
    optionsHint: MessageKey;        // description of the profile's run-options JSON
    known: KnownModel[];
}

// Levels understood by the local servers' chat templates / `reasoning_effort`.
const LOCAL_THINK = ['false', 'true', 'low', 'medium', 'high'];

export const PROVIDERS: ProviderInfo[] = [
    {
        id: 'ollama', label: 'Ollama', kind: 'ollama', auth: 'none', defaultBaseUrl: 'http://localhost:11434',
        thinkOptions: ['false', 'true', 'low', 'medium', 'high', 'max'], optionsHint: 'settings.modelProfile.options.description.ollama', known: OLLAMA_MODELS
    },
    {
        id: 'llamacpp', label: 'llama.cpp', kind: 'local', auth: 'optionalKey', defaultBaseUrl: 'http://127.0.0.1:9931/v1',
        baseUrlHint: 'settings.ai.baseUrl.hint.llamacpp', keyHint: 'settings.ai.apiKey.hint.llamacpp', thinkOptions: LOCAL_THINK, optionsHint: 'settings.modelProfile.options.description.local', known: []
    },
    {
        id: 'lmstudio', label: 'LM Studio', kind: 'local', auth: 'optionalKey', defaultBaseUrl: 'http://127.0.0.1:1234/v1',
        keyHint: 'settings.ai.apiKey.hint.lmstudio', thinkOptions: LOCAL_THINK, optionsHint: 'settings.modelProfile.options.description.local', known: []
    },
    {
        id: 'jan', label: 'Jan', kind: 'local', auth: 'optionalKey', defaultBaseUrl: 'http://127.0.0.1:1337/v1',
        keyHint: 'settings.ai.apiKey.hint.jan', thinkOptions: LOCAL_THINK, optionsHint: 'settings.modelProfile.options.description.local', known: []
    },
    {
        id: 'vllm', label: 'vLLM', kind: 'local', auth: 'optionalKey', defaultBaseUrl: 'http://127.0.0.1:8000/v1',
        baseUrlHint: 'settings.ai.baseUrl.hint.vllm', keyHint: 'settings.ai.apiKey.hint.vllm', thinkOptions: LOCAL_THINK, optionsHint: 'settings.modelProfile.options.description.local', known: []
    },
    {
        id: 'unsloth', label: 'Unsloth', kind: 'local', auth: 'optionalKey', defaultBaseUrl: 'http://127.0.0.1:8888/v1',
        baseUrlHint: 'settings.ai.baseUrl.hint.unsloth', keyHint: 'settings.ai.apiKey.hint.unsloth', thinkOptions: LOCAL_THINK, optionsHint: 'settings.modelProfile.options.description.local', known: []
    },
    {
        id: 'openai', label: 'OpenAI API', kind: 'cloud', auth: 'key', defaultBaseUrl: 'https://api.openai.com/v1',
        keyUrl: 'https://platform.openai.com/api-keys',
        thinkOptions: ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'], optionsHint: 'settings.modelProfile.options.description.openai', known: OPENAI_MODELS
    },
    {
        id: 'chatgpt', label: 'ChatGPT', kind: 'cloud', auth: 'oauth', defaultBaseUrl: null,
        thinkOptions: ['low', 'medium', 'high', 'xhigh', 'max'], optionsHint: 'settings.modelProfile.options.description.chatgpt', known: CHATGPT_MODELS
    },
    {
        id: 'anthropic', label: 'Claude API', kind: 'cloud', auth: 'key', defaultBaseUrl: 'https://api.anthropic.com',
        keyUrl: 'https://platform.claude.com/settings/keys',
        thinkOptions: ['low', 'medium', 'high', 'xhigh', 'max'], optionsHint: 'settings.modelProfile.options.description.anthropic', known: CLAUDE_MODELS
    }
];

export const DEFAULT_PROVIDER: ProviderId = 'ollama';

/** The provider entry for `id`; unknown ids (e.g. a profile saved by a newer version) fall back to Ollama. */
export function providerInfo(id: string | null | undefined): ProviderInfo {
    return PROVIDERS.find(p => p.id === id) ?? PROVIDERS[0];
}

/** Settings key of the provider's base URL, or null when it has none. */
export function baseUrlKey(id: ProviderId): string | null {
    return providerInfo(id).defaultBaseUrl === null ? null : `${id}.baseUrl`;
}

/** Settings key of the provider's active model. */
export function activeModelKey(id: ProviderId): string {
    return `${id}.activeModel`;
}
