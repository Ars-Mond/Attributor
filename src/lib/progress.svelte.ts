// Global progress overlay store (Svelte 5 runes). One operation at a time, shown above all other UI by
// ProgressOverlay. `run()` returns a handle to update/finish; a second concurrent run is rejected.

interface RunOpts {
    label: string;
    total?: number;        // determinate when set
    blocking?: boolean;    // freezes the rest of the UI
    cancelable?: boolean;
    onCancel?: () => void;
}

export interface ProgressHandle {
    update(p: {value?: number; total?: number; label?: string}): void;
    done(): void;
}

let active = $state(false);
let label = $state('');
let value = $state(0);
let total = $state<number | null>(null);
let blocking = $state(false);
let cancelable = $state(false);
let onCancel: (() => void) | null = null;

export const progress = {
    get active() {return active;},
    get label() {return label;},
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
            },
            done() {
                active = false;
                onCancel = null;
            }
        };
    }
};
