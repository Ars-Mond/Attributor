// The app-curated list of vision models offered for download in the Ollama settings (FR-004).
export interface OfferedModel {
    id: string;
    label: string;
}

export const OFFERED_MODELS: OfferedModel[] = [
    {id: 'qwen2.5vl:7b', label: 'qwen2.5vl:7b'},
    {id: 'qwen2.5vl:3b', label: 'qwen2.5vl:3b'},
    {id: 'qwen3-vl:8b', label: 'qwen3-vl:8b'},
    {id: 'llama3.2-vision:11b', label: 'llama3.2-vision:11b'},
    {id: 'gemma4:12b', label: 'gemma4:12b'},
    {id: 'gemma3:12b', label: 'gemma3:12b'}
];
