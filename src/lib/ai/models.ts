// Curated vision models suggested in the model fields, per provider. Ollama `local` models can also be pulled (Download);
// `cloud` models run remotely (Ollama's cloud or a cloud provider). The model fields accept any free-text id.
export type ModelKind = 'local' | 'cloud';

export interface KnownModel {
    id: string;
    kind: ModelKind;
}

export const OLLAMA_MODELS: KnownModel[] = [
    {id: 'qwen2.5vl:7b', kind: 'local'},
    {id: 'qwen2.5vl:3b', kind: 'local'},
    {id: 'qwen3-vl:8b', kind: 'local'},
    {id: 'llama3.2-vision:11b', kind: 'local'},
    {id: 'gemma3:12b', kind: 'local'},
    {id: 'gemma4:12b', kind: 'local'},
    {id: 'gemma4:cloud', kind: 'cloud'}
];

// OpenAI Platform vision models (October 2026); the account's live list is merged in from GET /v1/models.
export const OPENAI_MODELS: KnownModel[] = [
    {id: 'gpt-6-luna', kind: 'cloud'},
    {id: 'gpt-6.1-sol', kind: 'cloud'},
    {id: 'gpt-6-astra', kind: 'cloud'},
    {id: 'gpt-5.6-luna', kind: 'cloud'},
    {id: 'gpt-5.5', kind: 'cloud'},
    {id: 'gpt-5.4-mini', kind: 'cloud'}
];

// Models of the ChatGPT plan (Sign in with ChatGPT, October 2026); the plan's live catalog is merged in.
export const CHATGPT_MODELS: KnownModel[] = [
    {id: 'gpt-6.1-sol', kind: 'cloud'},
    {id: 'gpt-6-luna', kind: 'cloud'},
    {id: 'gpt-6-astra', kind: 'cloud'},
    {id: 'gpt-6-sol', kind: 'cloud'},
    {id: 'gpt-5.6-sol', kind: 'cloud'},
    {id: 'gpt-5.6-terra', kind: 'cloud'},
    {id: 'gpt-5.6-luna', kind: 'cloud'}
];

// Claude models with vision input; the key's live list is merged in from GET /v1/models.
export const CLAUDE_MODELS: KnownModel[] = [
    {id: 'claude-opus-5-5', kind: 'cloud'},
    {id: 'claude-sonnet-5-5', kind: 'cloud'},
    {id: 'claude-haiku-5-5', kind: 'cloud'},
    {id: 'claude-fable-5-1', kind: 'cloud'}
];

export interface OfferedModel {
    id: string;
    label: string;
}

// Models offered for local download — derived from the local Ollama models.
export const OFFERED_MODELS: OfferedModel[] = OLLAMA_MODELS
    .filter(m => m.kind === 'local')
    .map(m => ({id: m.id, label: m.id}));
