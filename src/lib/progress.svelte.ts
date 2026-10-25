// Global progress overlay store (Svelte 5 runes). One operation at a time, shown above all other UI by
// ProgressOverlay. `run()` returns a handle to update/finish; a second concurrent run is rejected.

/** An extra button on the overlay, next to Cancel. */
export interface ProgressAction {
    label: string;
    run: () => void;
}

interface RunOpts {
    label: string;
    detail?: string;       // explanatory text under the label
    action?: ProgressAction;
    total?: number;        // determinate when set
    blocking?: boolean;    // freezes the rest of the UI
    cancelable?: boolean;
    onCancel?: () => void;
}

export interface ProgressHandle {
    update(p: {value?: number; total?: number; label?: string; detail?: string | null; action?: ProgressAction | null}): void;
    done(): void;
}

let active = $state(false);
let label = $state('');
let detail = $state<string | null>(null);
let action = $state<ProgressAction | null>(null);
let value = $state(0);
let total = $state<number | null>(null);
let blocking = $state(false);
let cancelable = $state(false);
let onCancel: (() => void) | null = null;

export const progress = {
    get active() {return active;},
    get label() {return label;},
    get detail() {return detail;},
    get action() {return action;},
    get value() {return value;},
    get total() {return total;},
    get blocking() {return blocking;},
    get cancelable() {return cancelable;},
    get determinate() {return total !== null;},
    get percent() {return total && total > 0 ? Math.min(100, Math.round((value / total) * 100)) : 0;},

    cancel() {onCancel?.();},

    run(opts: RunOpts): ProgressHandle {
        if (active) throw new Error('progress: an operation is already active');
        active = true;
        label = opts.label;
        detail = opts.detail ?? null;
        action = opts.action ?? null;
        value = 0;
        total = opts.total ?? null;
        blocking = opts.blocking ?? false;
        cancelable = opts.cancelable ?? false;
        onCancel = opts.onCancel ?? null;

        return {
            update(p) {
                if (p.value !== undefined) value = p.value;
                if (p.total !== undefined) total = p.total;
                if (p.label !== undefined) label = p.label;
                if (p.detail !== undefined) detail = p.detail;
                if (p.action !== undefined) action = p.action;
            },
            done() {
                active = false;
                detail = null;
                action = null;
                blocking = false;
                cancelable = false;
                onCancel = null;
            }
        };
    }
};
