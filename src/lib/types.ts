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

export interface ReadResult {
    title: string;
    description: string;
    keywords: string[];
    categories: string;
    releaseFilename: string;
}
