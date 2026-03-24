export type SplitDirection = 'horizontal' | 'vertical';
export type DropZone = 'top' | 'right' | 'bottom' | 'left';

export interface SplitChild {
    node: LayoutNode;
    size: number; // fraction, all sizes in a split sum to 1.0
}

export interface SplitNode {
    type: 'split';
    direction: SplitDirection;
    children: SplitChild[];
}

export interface PanelNode {
    type: 'panel';
    windowId: string;
}

export type LayoutNode = SplitNode | PanelNode;

export interface WindowConfig {
    id: string;
    title: string;
    closable: boolean;
}

export interface DragState {
    windowId: string;
    targetWindowId: string | null;
    zone: DropZone | null;
    startX: number;
    startY: number;
    active: boolean;
}
