import type {LayoutNode, SplitNode, SplitChild, PanelNode, DropZone, SplitDirection} from './dockTypes';

export function getDefaultLayout(): LayoutNode {
    return {
        type: 'split',
        direction: 'horizontal',
        children: [
            {node: {type: 'panel', windowId: 'control'}, size: 0.28},
            {node: {type: 'panel', windowId: 'view'}, size: 0.54},
            {node: {type: 'panel', windowId: 'hierarchy'}, size: 0.18},
        ],
    };
}

/** Check if a panel exists in the tree. */
export function findPanel(tree: LayoutNode, windowId: string): boolean {
    if (tree.type === 'panel') return tree.windowId === windowId;
    return tree.children.some(c => findPanel(c.node, windowId));
}

/** Remove a panel from the tree. Returns simplified tree or null if root was the panel. */
export function removePanel(tree: LayoutNode, windowId: string): LayoutNode | null {
    if (tree.type === 'panel') {
        return tree.windowId === windowId ? null : tree;
    }

    const idx = tree.children.findIndex(c => c.node.type === 'panel' && c.node.windowId === windowId);
    if (idx !== -1) {
        // Direct child — remove it and redistribute its size to neighbor
        const removed = tree.children[idx];
        const remaining = tree.children.filter((_, i) => i !== idx);
        if (remaining.length === 0) return null;
        if (remaining.length === 1) return remaining[0].node;

        // Give removed size to adjacent sibling
        const neighborIdx = idx > 0 ? idx - 1 : 0;
        const newChildren = remaining.map((c, i) =>
            i === neighborIdx ? {node: c.node, size: c.size + removed.size} : c
        );
        return {type: 'split', direction: tree.direction, children: newChildren};
    }

    // Recurse into children
    const newChildren: SplitChild[] = [];
    let changed = false;
    for (const child of tree.children) {
        const result = removePanel(child.node, windowId);
        if (result !== child.node) {
            changed = true;
            if (result === null) {
                // Child was the panel — shouldn't happen here since we checked above
                continue;
            }
            newChildren.push({node: result, size: child.size});
        } else {
            newChildren.push(child);
        }
    }

    if (!changed) return tree;
    if (newChildren.length === 0) return null;
    if (newChildren.length === 1) return newChildren[0].node;
    return {type: 'split', direction: tree.direction, children: newChildren};
}

/** Get the direction a drop zone implies. */
function zoneDirection(zone: DropZone): SplitDirection {
    return (zone === 'left' || zone === 'right') ? 'horizontal' : 'vertical';
}

/** Whether the dragged panel should come before the target for this zone. */
function zoneBefore(zone: DropZone): boolean {
    return zone === 'left' || zone === 'top';
}

/**
 * Insert a panel next to a target panel.
 * If the target is inside a split with matching direction, inserts flat into the same array.
 * Otherwise creates a new nested split.
 */
export function insertPanel(
    tree: LayoutNode,
    targetId: string,
    draggedId: string,
    zone: DropZone,
): LayoutNode {
    return insertPanelInner(tree, targetId, draggedId, zone, null);
}

function insertPanelInner(
    tree: LayoutNode,
    targetId: string,
    draggedId: string,
    zone: DropZone,
    parentDirection: SplitDirection | null,
): LayoutNode {
    if (tree.type === 'panel') {
        if (tree.windowId !== targetId) return tree;

        // Target found — create a new split wrapping target + dragged
        const dir = zoneDirection(zone);
        const dragged: PanelNode = {type: 'panel', windowId: draggedId};
        const target: PanelNode = {type: 'panel', windowId: targetId};
        const first = zoneBefore(zone) ? dragged : target;
        const second = zoneBefore(zone) ? target : dragged;
        return {
            type: 'split',
            direction: dir,
            children: [
                {node: first, size: 0.5},
                {node: second, size: 0.5},
            ],
        };
    }

    // Split node — check if any direct child is the target panel AND the zone direction matches
    const dir = zoneDirection(zone);
    if (dir === tree.direction) {
        const targetIdx = tree.children.findIndex(
            c => c.node.type === 'panel' && c.node.windowId === targetId
        );
        if (targetIdx !== -1) {
            // Flatten: insert into this array, splitting the target's size
            const targetChild = tree.children[targetIdx];
            const half = targetChild.size / 2;
            const dragged: SplitChild = {node: {type: 'panel', windowId: draggedId}, size: half};
            const resized: SplitChild = {node: targetChild.node, size: half};

            const newChildren = [...tree.children];
            if (zoneBefore(zone)) {
                newChildren.splice(targetIdx, 1, dragged, resized);
            } else {
                newChildren.splice(targetIdx, 1, resized, dragged);
            }
            return {type: 'split', direction: tree.direction, children: newChildren};
        }
    }

    // Recurse into children
    const newChildren = tree.children.map(child => {
        const result = insertPanelInner(child.node, targetId, draggedId, zone, tree.direction);
        if (result !== child.node) {
            // If the result is a split with same direction as us, flatten it
            if (result.type === 'split' && result.direction === tree.direction) {
                // This case is when a panel was replaced by a split of same direction —
                // but this only happens when the panel was NOT a direct child (handled above).
                // For nested panels that created a new split with matching parent direction,
                // we keep it nested to preserve the tree structure correctly.
            }
            return {node: result, size: child.size};
        }
        return child;
    });

    return {type: 'split', direction: tree.direction, children: newChildren};
}

/** Add a panel back to the layout. If root is horizontal split, append; otherwise wrap. */
export function addPanelToRoot(tree: LayoutNode, windowId: string): LayoutNode {
    if (tree.type === 'split' && tree.direction === 'horizontal') {
        const total = tree.children.reduce((s, c) => s + c.size, 0);
        const newSize = 0.2;
        const scale = (total - newSize) / total;
        const scaled = tree.children.map(c => ({node: c.node, size: c.size * scale}));
        return {
            type: 'split',
            direction: 'horizontal',
            children: [...scaled, {node: {type: 'panel', windowId}, size: newSize}],
        };
    }
    return {
        type: 'split',
        direction: 'horizontal',
        children: [
            {node: tree, size: 0.8},
            {node: {type: 'panel', windowId}, size: 0.2},
        ],
    };
}

/** Serialize layout to JSON string. */
export function serializeLayout(tree: LayoutNode): string {
    return JSON.stringify(tree);
}

/** Deserialize layout from JSON string. Returns null if invalid. */
export function deserializeLayout(json: string): LayoutNode | null {
    try {
        const parsed = JSON.parse(json);
        if (isValidNode(parsed)) return parsed;
        return null;
    } catch {
        return null;
    }
}

function isValidNode(node: unknown): node is LayoutNode {
    if (!node || typeof node !== 'object') return false;
    const n = node as Record<string, unknown>;
    if (n.type === 'panel') return typeof n.windowId === 'string';
    if (n.type === 'split') {
        if (n.direction !== 'horizontal' && n.direction !== 'vertical') return false;
        if (!Array.isArray(n.children) || n.children.length < 2) return false;
        return (n.children as unknown[]).every(
            c => c && typeof c === 'object' &&
                typeof (c as Record<string, unknown>).size === 'number' &&
                isValidNode((c as Record<string, unknown>).node)
        );
    }
    return false;
}
