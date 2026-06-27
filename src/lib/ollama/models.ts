// Curated vision models suggested in the model fields. `local` models can also be pulled (Download);
// `cloud` models run via Ollama's cloud (no local download). The model fields accept any free-text id.
export type ModelKind = 'local' | 'cloud';

export interface KnownModel {
    id: string;
    kind: ModelKind;
}

export const KNOWN_MODELS: KnownModel[] = [
    {id: 'qwen2.5vl:7b', kind: 'local'},
    {id: 'qwen2.5vl:3b', kind: 'local'},
    {id: 'qwen3-vl:8b', kind: 'local'},
    {id: 'llama3.2-vision:11b', kind: 'local'},
    {id: 'gemma3:12b', kind: 'local'},
    {id: 'gemma4:12b', kind: 'local'},
    {id: 'gemma4:cloud', kind: 'cloud'}
];

export interface OfferedModel {
    id: string;
    label: string;
}

// Models offered for local download — derived from the local KNOWN_MODELS.
export const OFFERED_MODELS: OfferedModel[] = KNOWN_MODELS
    .filter(m => m.kind === 'local')
    .map(m => ({id: m.id, label: m.id}));
