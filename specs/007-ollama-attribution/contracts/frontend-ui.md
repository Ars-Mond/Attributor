# Contract: Frontend UI surfaces (feature 007)

Svelte 5 runes only (Constitution II). Reuse existing primitives (Constitution V): settings custom-section
registration, the dialog/popup overlay pattern, the i18n layer, the Channel+cancel pattern. All new strings
go through `t()`/`tn()` and are added to `en.ts`/`ru.ts`.

## Reusable progress overlay — `src/lib/reusable/ProgressOverlay.svelte` + `progress.svelte.ts`

A single global overlay, mounted once in `src/routes/+page.svelte`, at **z-index 600** (above SettingsDialog
500 and dialogs/loading 200; below DockLayout drag ghost 9999). Driven by a runes store:

```ts
// progress.svelte.ts — singleton, $state-backed
interface ProgressHandle { update(p: {value?: number; total?: number; label?: string}): void; done(): void; }
export const progress: {
  active(): boolean;                       // reactive getter
  run(opts: {label: string; total?: number; blocking?: boolean; cancelable?: boolean;
             onCancel?: () => void}): ProgressHandle;   // rejects if one is already active
};
```

- `blocking: true` covers the screen and swallows pointer events (freeze) until `done()`.
- `cancelable: true` shows a Cancel button that calls `onCancel` (which invokes `ollama_cancel` /
  `cancel_batch`) and then `done()`.
- Determinate when `total` is set (batch items, pull bytes), else indeterminate (single inference).
- One operation at a time (FR-016/017/018/019).

## Settings: Ollama category — `src/lib/settings/OllamaSettingsPage.svelte`

Registered `settings.registerSection({id:'ollama', label:'settings.section.ollama', order:N, component:OllamaSettingsPage})`.
Renders (all labels via `t()`):

- **Status row** + "Check" button → `ollama_status` (shows reachable + version; heartbeat only).
- **Install** button (shown when not reachable) → `install_ollama` (runs the official per-OS script, then re-checks status).
- **Offered-models** dropdown + **Download** button → `ollama_pull_model` with a Channel; progress via the
  overlay; on success refresh installed list. (Offered list contents deferred.)
- **Installed-models** dropdown → `ollama_list_models`; selection writes `ollama.activeModel`.
- **Base URL** field → `ollama.baseUrl`.
- **JSON response-format** textarea → `ollama.responseFormat`; description (`field-desc`) warns it is
  debug-only and should not normally be edited.

## Settings: Ollama Models category — `src/lib/settings/OllamaModelsPage.svelte` + `OllamaModelDialog.svelte`

Registered as a second custom section (`id:'ollama-models'`). A list of `ModelProfile` with **Create / Edit /
Delete** buttons (modeled on the user's reference screenshot). Create/Edit open `OllamaModelDialog` (a popup
using the existing dialog overlay) with: model id (select from installed or free text), run parameters
(context length, thinking mode, …), and the prompt, plus **Save** / **Cancel**. Persisted to
`ollama.modelProfiles`.

## Metadata panel: attribute action — `src/lib/panel/MetadataPanel.svelte`

- An **"Attribute via Ollama"** button. Disabled/greyed with a tooltip (`t('ollama.unavailable.tooltip')`)
  when not `available` (status not reachable, or no active model) (FR-008/009).
- **Single mode**: calls `attribute_photo(path, config)`; on success applies the result — `title` and
  `description` overwrite, `categories` ← `result.categories.join(', ')`, `keywords` ← existing + result
  (de-duped via the existing `addKeyword` logic); marks dirty; the user saves (FR-010).
- **Batch mode**: calls `attribute_batch(paths, config, channel)` through the progress overlay (cancelable);
  the backend saves each file; on completion the panel reloads batch data (FR-013).
- `config` is assembled from settings (`baseUrl`, `activeModel`, the matching profile's prompt/think/
  keepAlive/options, and `responseFormat` parsed to an object).

## Batch save routed through the overlay (clarification)

`handleBatchSave` (existing) wraps its `save_metadata_batch` call in `progress.run({blocking:true,
cancelable:true, onCancel:()=>invoke('cancel_batch')})`, freezing the UI until done (FR-020).

## i18n keys (added to `en.ts` / `ru.ts`)

`settings.section.ollama`, `settings.section.ollamaModels`, the Ollama settings labels/descriptions
(`settings.ollama.*`), the model-dialog labels, `ollama.attribution.*` (in-progress/complete/failed),
`ollama.unavailable.tooltip`, `ollama.pull.*`, and the progress/cancel labels.
