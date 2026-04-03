export interface FileNode {
    name: string;
    path: string;
    is_dir: boolean;
    children: FileNode[];
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
