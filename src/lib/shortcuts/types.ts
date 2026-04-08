export interface ActionDescriptor {
    id: string;
    label: string;
    section: string;
    defaultBinding: string | null;
    handler: () => void;
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
