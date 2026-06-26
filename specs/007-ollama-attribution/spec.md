# Feature Specification: Ollama Vision Auto-Attribution

**Feature Branch**: `007-ollama-attribution`

**Created**: 2026-06-26

**Status**: Draft

**Input**: User description: "Integrate Ollama vision models to automatically attribute photos (fill the
metadata fields). Ollama returns a strict JSON in a fixed format. The UI gets an attribute button for a
single photo and for batch mode; single mode fills the in-app fields, batch mode always saves. Add a
reusable, top-most progress bar with a cancel button and (partial) app blocking; optionally route batch
save through it (freeze until done). Settings gain an Ollama category (check-installed / install buttons,
a dropdown of offered models to download, a dropdown of installed models to use, and an editable
debug-only JSON response-format field) plus an 'Ollama Models' category with per-model profiles
(model id, run parameters, prompt) managed by a list with Create/Edit/Delete and a Save/Cancel popup. A
greyed-out attribute button with an explanatory tooltip when Ollama is unavailable. Default prompts and
the offered-model list will be provided later."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Auto-attribute a single photo (Priority: P1)

A user opens a photo, clicks the Ollama attribute button in the metadata panel, and the app analyses the
image with the configured vision model and fills the editor fields (title, description, keywords,
categories, and the editorial / mature-content / illustration flags) from the model's response. The user
reviews the suggestions and saves them manually.

**Why this priority**: This is the headline value — turning a blank metadata form into a filled one from
the image alone. Everything else exists to enable or scale this.

**Independent Test**: With Ollama configured and a model selected, open a photo, click attribute → the
fields populate from the image within a reasonable time and nothing is saved until the user saves.

**Acceptance Scenarios**:

1. **Given** Ollama is available and a model is selected, **When** the user clicks attribute on an open photo, **Then** the editor fields are filled from the model's response and remain unsaved (the file is unchanged until the user saves).
2. **Given** an attribution is running, **When** it completes, **Then** the title, description, keywords, categories, and the three flags reflect the returned values, with list values placed in the correct fields.
3. **Given** the model returns an invalid or non-conforming response, **When** attribution finishes, **Then** the user sees an error and no fields are corrupted or partially overwritten in a broken way.

---

### User Story 2 - Configure and detect Ollama in settings (Priority: P1)

A user opens settings, finds the Ollama category, checks whether Ollama is installed/running, installs it
if missing, downloads one of the offered models, selects which installed model the app uses, and (rarely)
inspects the enforced JSON response format. This makes attribution possible.

**Why this priority**: Attribution cannot run without a reachable Ollama and a selected model; this is the
enabling configuration and is independently testable on its own.

**Independent Test**: In settings → Ollama, the check action reports installed/not-installed correctly;
selecting an installed model persists it; with no model or no Ollama, attribution is disabled everywhere.

**Acceptance Scenarios**:

1. **Given** the Ollama settings category, **When** the user triggers the check action, **Then** the app reports whether Ollama is installed and reachable.
2. **Given** Ollama is not installed, **When** the user triggers the install action, **Then** the app guides the user through installing Ollama.
3. **Given** the offered-models dropdown, **When** the user picks a model and starts the download, **Then** the model is pulled into Ollama with progress shown, and it then appears in the installed-models list.
4. **Given** the installed-models dropdown, **When** the user selects a model, **Then** that model becomes the active model used for attribution and the choice persists across restarts.
5. **Given** the JSON response-format field, **When** the user views it, **Then** its description warns that it is for debugging only and should not normally be edited.

---

### User Story 3 - Batch attribute and always save (Priority: P2)

A user selects several photos and triggers batch attribution. The app attributes each photo and always
saves the result automatically, showing overall progress and offering cancel.

**Why this priority**: Scales the headline value to many files; depends on US1 working but adds the
auto-save and progress/cancel behaviour that make bulk processing usable.

**Independent Test**: Select N photos, run batch attribution → each photo is attributed and saved without
manual intervention; progress is visible and cancel stops further processing.

**Acceptance Scenarios**:

1. **Given** multiple photos are selected and Ollama is available, **When** the user runs batch attribution, **Then** every photo is attributed and its result is saved automatically.
2. **Given** a batch is running, **When** the user cancels, **Then** processing stops promptly and photos already saved stay saved while the rest are left unchanged.
3. **Given** a photo in the batch fails (e.g. invalid response), **When** the batch continues, **Then** the failure is recorded and surfaced without aborting the whole batch or corrupting other files.

---

### User Story 4 - Reusable global progress overlay with cancel (Priority: P2)

Any long-running operation shows a single shared progress overlay that sits above all other windows and
popups, displays what is happening and how far along it is, and offers a cancel action. Batch save can be
routed through it so the app is frozen until the save completes.

**Why this priority**: A consistent, top-most progress + cancel surface is what makes long AI and save
operations feel safe and controllable; it is reused by US3 and by existing batch save.

**Independent Test**: Start any long operation → the overlay appears above everything, shows progress and a
cancel button; cancelling ends the operation; while a frozen operation runs, the rest of the UI is blocked.

**Acceptance Scenarios**:

1. **Given** a long operation starts, **When** it is running, **Then** the progress overlay is shown above every other UI element (panels, dialogs, menus) with a label, progress indication, and a cancel control.
2. **Given** the overlay is shown for a blocking operation, **When** the user tries to interact with the rest of the app, **Then** interaction is prevented until the operation finishes or is cancelled.
3. **Given** a cancellable operation, **When** the user clicks cancel, **Then** the operation stops as soon as practical and the overlay is dismissed.

---

### User Story 5 - Manage per-model profiles (Priority: P3)

In the "Ollama Models" settings category, a user manages a list of model profiles. Each profile holds a
model identifier, its run parameters, and its prompt. The user can create, edit, and delete profiles; create
and edit open a popup with Save and Cancel.

**Why this priority**: Lets advanced users tune prompts and run parameters per model; valuable but not
required for the basic attribute-and-fill flow.

**Independent Test**: Create a profile via the popup, edit it, delete it → changes persist across restart
and the active model's profile (prompt + parameters) is the one used during attribution.

**Acceptance Scenarios**:

1. **Given** the Ollama Models category, **When** the user clicks Create, **Then** a popup opens to enter/select a model identifier, set run parameters, and write a prompt, with Save and Cancel.
2. **Given** an existing profile is selected, **When** the user clicks Edit and saves changes, **Then** the profile is updated and persisted; **When** the user clicks Delete, **Then** the profile is removed.
3. **Given** a saved profile for the active model, **When** an attribution runs, **Then** that profile's prompt and parameters drive the request.

---

### Edge Cases

- Ollama is installed but the service is not running, or is unreachable → attribution is disabled and the reason is shown; the check action reports "not reachable".
- The selected model is not actually pulled / was removed in Ollama → attribution reports a clear error instead of silently failing.
- The model returns malformed JSON, missing fields, or extra fields → handled gracefully (validation, error surfaced), never corrupting existing metadata.
- The user switches photos or closes the app while a single attribution is in flight → the in-flight result is discarded for the no-longer-open photo.
- A batch is cancelled midway → already-saved files remain saved; the remaining files are untouched.
- A very large batch → progress remains responsive; the overlay keeps the cancel control usable.
- The image cannot be read / is unsupported → that item fails with a clear message and does not abort the batch.
- No active model is selected, or no offered model has been downloaded → the attribute button is disabled with an explanatory tooltip.
- A model download is interrupted or fails → the user is informed and can retry; no partial model is treated as installed.
- Two long operations are requested at once → the shared overlay represents one operation at a time (a second request waits or is rejected, not silently overlapped).

## Requirements *(mandatory)*

### Functional Requirements

#### Ollama configuration & availability

- **FR-001**: Settings MUST include a dedicated "Ollama" category.
- **FR-002**: The Ollama settings MUST provide an action to check whether Ollama is installed and reachable, and MUST display the result.
- **FR-003**: The Ollama settings MUST provide an action to install Ollama when it is not present.
- **FR-004**: The Ollama settings MUST provide a selection of models the app offers for download and an action to download (pull) the chosen model into Ollama, with progress shown during the download.
- **FR-005**: The Ollama settings MUST provide a selection of the models currently installed in Ollama (sourced from Ollama); the selected one is the active model used for attribution, and the selection MUST persist across restarts.
- **FR-006**: The Ollama settings MUST include an editable field holding the enforced JSON response format; its description MUST state that it is for debugging only and should not normally be edited.
- **FR-007**: The app MUST treat attribution as available only when Ollama is reachable AND an active model is selected; otherwise attribution MUST be disabled.

#### Attribution

- **FR-008**: The metadata panel MUST present an "attribute via Ollama" action.
- **FR-009**: When attribution is unavailable, the attribute action MUST be visibly disabled (greyed out) and MUST show a tooltip explaining that Ollama is not available/working.
- **FR-010**: Single-photo attribution MUST analyse the open image with the active model and populate the editor fields from the model's response; it MUST NOT auto-save — the user reviews and saves.
- **FR-011**: Attribution MUST request and enforce a strict JSON response in the fixed format and MUST validate the response before applying it.
- **FR-012**: The attribution result MUST map to the editor's fields: title, description, keywords, categories, and the editorial, mature-content, and illustration flags.
- **FR-013**: Batch attribution MUST attribute every selected photo and MUST always save each result automatically.
- **FR-014**: A failed item in a batch MUST be recorded and surfaced without aborting the rest of the batch and without corrupting other files.
- **FR-015**: A malformed or non-conforming response MUST never corrupt or partially-overwrite metadata in a broken state; the user MUST be informed of the failure.

#### Progress & cancellation

- **FR-016**: The app MUST provide a single reusable progress overlay component, displayed above all other UI elements (panels, dialogs, menus, popups).
- **FR-017**: The progress overlay MUST show a description of the current operation, its progress, and a cancel control.
- **FR-018**: While a blocking operation is shown, the overlay MUST prevent interaction with the rest of the app until the operation completes or is cancelled.
- **FR-019**: Cancelling MUST stop the operation as soon as practical and dismiss the overlay; work already committed (e.g. files already saved) MUST remain.
- **FR-020**: Batch attribution and batch save MUST report their progress through this overlay; batch save MUST freeze interaction until it completes.

#### Per-model profiles

- **FR-021**: Settings MUST include an "Ollama Models" category presenting a list of model profiles with Create, Edit, and Delete actions.
- **FR-022**: Creating or editing a profile MUST open a popup where the user sets the model identifier (Ollama id), the run parameters (such as context length, thinking mode, and other model options), and the prompt, with Save and Cancel.
- **FR-023**: Model profiles MUST persist across application restarts.
- **FR-024**: During attribution, the active model's profile (its prompt and run parameters) MUST drive the request.

### Key Entities *(include if feature involves data)*

- **Ollama availability**: Whether Ollama is installed and reachable, used to enable/disable attribution and to inform the user.
- **Offered model**: An entry in the app-curated list of models it offers to download (identifier + display name). The specific list is provided separately and is out of scope to author here.
- **Installed model**: A model currently present in Ollama, discovered from Ollama; one is marked active.
- **Active model selection**: The installed model the app uses for attribution; persisted.
- **Response format / schema**: The enforced JSON structure for model output; editable for debugging; default is the fixed format below.
- **Attribution result**: A single image's model output — `title`, `description`, `keywords[]`, `categories[]`, `editorial` (bool), `mature_content` (bool), `illustration` (bool).
- **Model profile**: A reusable configuration for one model — model identifier, run parameters (context, thinking mode, and others), and prompt; persisted; the active model's profile is used.
- **Long-running operation**: An operation surfaced by the progress overlay — a label, progress, blocking/non-blocking, and whether it is cancellable.

**Fixed response format** (default for the response-format field):

```json
{
    "title": "Some title",
    "description": "A brief descriptive text for the image.",
    "keywords": ["keyword1", "keyword2", "..."],
    "categories": ["category1", "category2"],
    "editorial": true,
    "mature_content": true,
    "illustration": true
}
```

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: From a configured setup, a user attributes a single photo and sees all target fields populated from the image in one action, then saves — with zero manual field entry required to get a first draft.
- **SC-002**: Batch attribution processes a selection and saves 100% of successfully-attributed photos automatically, with per-item progress visible throughout.
- **SC-003**: The progress overlay appears above every other UI element and offers a working cancel control for 100% of long-running operations.
- **SC-004**: When Ollama is unreachable or no model is selected, the attribute action is disabled with an explanatory tooltip 100% of the time, and no attribution request is sent.
- **SC-005**: A malformed model response never results in corrupted or partially-saved metadata; the user is informed in 100% of such cases.
- **SC-006**: Cancelling a batch stops further processing within a short, bounded time, and every file saved before cancellation remains intact.
- **SC-007**: A user can create, edit, and delete a model profile and have the changes persist across an application restart, with the active model's profile used by attribution.
- **SC-008**: A user can select an installed model and download an offered model entirely from settings, with the installed-models list reflecting the result.

## Assumptions

- **Local Ollama**: Ollama runs locally on the user's machine and the app communicates with the local service; remote/cloud LLM providers are out of scope.
- **Install action**: "Install Ollama" guides the user to the official installation (e.g. opening the official download/installation path) rather than performing a silent, unattended system install; the check action then confirms success. (Open for `/speckit-clarify` if a deeper automated install is desired.)
- **The three flags**: `editorial`, `mature_content`, and `illustration` are surfaced as editable boolean attributes in the editor so the auto-filled values can be reviewed and saved alongside the existing metadata. Their exact persistence target in the image is to be confirmed in planning/clarify; this feature introduces them as part of what the editor manages. (Open for `/speckit-clarify`.)
- **Single-mode overwrite**: Single-photo attribution fills/overwrites the current field values with the suggestions; the user reviews before saving (nothing is written to disk until save).
- **Batch concurrency**: Batch attribution may process images sequentially or with bounded concurrency (a planning decision), but always saves each result; ordering of progress is not guaranteed.
- **Cancellation is cooperative**: Cancelling stops before starting the next item and/or after the current inference returns; an in-flight inference may need to finish before the operation ends.
- **Profile persistence**: Model profiles and Ollama settings reuse the application's existing settings/store mechanism; a separate storage system is not introduced unless unavoidable.
- **One overlay at a time**: The shared progress overlay represents a single operation at a time; concurrent long operations are serialized or rejected rather than stacked.
- **Deferred content**: Default prompts, the curated list of offered models, and default run-parameter values are NOT authored in this feature; they will be supplied later. The structures that hold them are in scope; their contents are not.
- **Model parameters are open-ended**: Run parameters include at least context length and thinking mode, plus other model options; the exact, complete parameter set is finalized during planning.

### Out of scope

- Authoring the default prompts, the offered-model list, and default parameter values (provided later).
- Cloud or non-Ollama model providers.
- Training, fine-tuning, or evaluating models.
- Translating or post-editing the model's textual output beyond placing it into fields.
- Guaranteeing the factual accuracy of model suggestions (the user reviews single-photo results; batch trusts the model by design).
