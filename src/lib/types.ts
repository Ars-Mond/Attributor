export interface FileNode {
    name: string;
    path: string;
    is_dir: boolean;
    children: FileNode[];
}

export interface Metadata {
    filename: string;
    title: string;
    description: string;
    keywords: string[];
    categories: string;
    releaseFilename: string;
}
