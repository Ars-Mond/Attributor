export interface ActionDescriptor {
    id: string;
    label: string;
    section: string;
    defaultBinding: string | null;
    handler: () => void;
    // When set, the action only fires while this specific layer is the one being evaluated (e.g. file
    // navigation is scoped to the 'files' layer so it never hijacks arrow keys while typing in a field).
    layer?: string;
}

export interface LayerConfig {
    id: string;
    priority: number;
    suppressBelow?: boolean;
    autoActivate?: () => boolean;
}

export interface ParsedBinding {
    key: string;
    ctrl: boolean;
    shift: boolean;
    alt: boolean;
    meta: boolean;
}
