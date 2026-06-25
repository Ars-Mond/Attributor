# Contract: Batch Progress Channel Protocol

Per-operation progress flows over `tauri::ipc::Channel<BatchProgress>` passed to
`save_metadata_batch` (FR-005, FR-014). One message per file as it finishes.

## Message type

```rust
// src-tauri/src/events.rs
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub index: usize,        // index into the input items Vec
    pub status: ItemStatus,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ItemStatus {
    Ok { path: String },
    Failed { error: String },
    Cancelled,
}
```

TypeScript (generated into `src/lib/generated/events.d.ts`; consumed via `src/lib/events.ts`):

```ts
type ItemStatus =
  | {kind: "ok"; path: string}
  | {kind: "failed"; error: string}
  | {kind: "cancelled"};
interface BatchProgress {index: number; status: ItemStatus}
```

## Sequencing & rules

1. The backend sends exactly one `BatchProgress` per input item (count == `items.length`).
2. **No ordering guarantee** across items (concurrent `rayon` senders). The frontend MUST key UI
   state by `index`, not arrival order (R1).
3. Progress is derived: `total = items.length`; `done = number of messages received`. The batch is
   complete when `done == total` and the `invoke` promise resolves with the final `ItemStatus[]`.
4. `Channel::send` errors (window/webview gone) are ignored/logged backend-side and never panic
   (§VI/§IX); the command still returns its `Vec<ItemStatus>`.
5. All sends happen before the command returns (the JS Channel is only alive for the invoke's
   duration).

## Authoritative result vs stream

The streamed messages drive incremental UI; the command's returned `Vec<ItemStatus>` (input order)
is the source of truth the frontend reconciles against once the promise resolves (avoid
double-counting — treat the channel as progress only).
