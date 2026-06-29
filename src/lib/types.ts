export interface FileNode {
    name: string;
    path: string;
    is_dir: boolean;
    children: FileNode[];
    thumb_low?: string;     // cached low (360px longest side) thumbnail path
    thumb_high?: string;    // cached high (1920px longest side) thumbnail path
}

export interface Metadata {
    filepath: string;       // full path (for Rust to read/write the file)
    filename: string;       // stem only — no extension, no directory (for renaming)
    title: string;
    description: string;
    keywords: string[];
    categories: string;
    releaseFilename: string;
}

/** Configurable button for dialog/popup footers. Color fields accept any CSS value. */
export interface DialogButton {
    label: string;
    onClick: () => void;
    // Base state
    bg?: string;
    color?: string;
    border?: string;
    // Hover state (falls back to base if omitted)
    hoverBg?: string;
    hoverColor?: string;
    hoverBorder?: string;
    // Active / pressed state (falls back to hover if omitted)
    activeBg?: string;
    activeColor?: string;
    activeBorder?: string;
}

export interface ReadResult {
    title: string;
    description: string;
    keywords: string[];
    categories: string;
    releaseFilename: string;
}

// ── Intermediate metadata store (feature 008) ──

export type SyncState = 'synced' | 'appOnly';

/** The editable fields the store holds (mirrors what the editor edits). */
export interface StoredMetadata {
    title: string;
    description: string;
    keywords: string[];
    categories: string;
    releaseFilename: string;
}

/** Result of resolving a photo's metadata on open: load directly, or a conflict to prompt on. */
export type MetadataResolution =
    | {kind: 'resolved'; metadata: StoredMetadata; syncState: SyncState}
    | {kind: 'conflict'; store: StoredMetadata; file: StoredMetadata};
