---

description: "Task list for Russian UI localization"
---

# Tasks: Russian UI Language (Localization)

**Input**: Design documents from `/specs/006-ui-localization/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: No automated test tasks (vitest/playwright are out of scope per prior project decision). The
completeness gate is `npm run check` (svelte-check enforces that every catalog implements `Messages`);
`cargo check`/`cargo test` guards the backend; quickstart.md covers manual validation.

**Organization**: Tasks are grouped by user story. US1 delivers the visible Russian UI (MVP); US2 adds
first-run OS default + remembered choice; US3 secures extensibility/fallback.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1 / US2 / US3 (setup, foundational, polish carry no story label)

## Path Conventions

Single desktop app: frontend in `src/`, backend in `src-tauri/`. The new localization folder is
`src/lib/i18n/`. Paths below are repo-relative.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add the one new dependency and establish a clean baseline.

- [ ] T001 [P] Add `sys-locale = "0.3"` to the `[dependencies]` table in `src-tauri/Cargo.toml` (pure-Rust OS-locale detection; justified in research.md Decision 2).
- [ ] T002 [P] Confirm a clean baseline before changes: `npm run check` and `cargo check` (in `src-tauri/`) both pass.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The localization runtime that every screen and story depends on. After this phase the i18n
engine works (English) but no screen consumes it yet.

**⚠️ CRITICAL**: No user story can begin until this phase is complete.

- [ ] T003 Define localization types in `src/lib/i18n/types.ts`: the `Locale` union (`'en' | 'ru'`), `LOCALES`, `DEFAULT_LOCALE` (`'en'`), an `endonyms` map (`en: 'English'`, `ru: 'Русский'`), `PluralForms` (`{one, other}` for EN / `{one, few, many}` for RU), the `Messages` interface grouped by area (`common`, `menu`, `files`, `metadata`, `viewer`, `dialog`, `shortcuts`, `settings`) covering the inventory in research.md Decision 5, and the derived `MessageKey` / `PluralKey` key types.
- [ ] T004 [P] Implement `pluralCategory(loc, n)` in `src/lib/i18n/plural.ts` per CLDR (RU one/few/many, EN one/other) exactly as research.md Decision 3.
- [ ] T005 Implement the reactive store in `src/lib/i18n/store.svelte.ts`: a module-level `$derived` active locale normalized from `settings.subscribe<string>('general.language')()`; `locale()`; `setLocale(value)` → `settings.set('general.language', normalize(value))`; `t(key, params?)` with `{name}` interpolation and fallback chain `catalog[locale][key] → catalog[DEFAULT_LOCALE][key] → key`; `tn(key, count, params?)` selecting the form via `pluralCategory` with `{n}` exposed (depends on T003, T004, T006).
- [ ] T006 Create `src/lib/i18n/catalog.ts` (`export const catalog: Record<Locale, Messages>` from `en`/`ru`) and `src/lib/i18n/index.ts` (re-export `t`, `tn`, `locale`, `setLocale`, `LOCALES`, `DEFAULT_LOCALE`, types; `initLocale()` is added in US2) (depends on T003).
- [ ] T007 Author the full English catalog `src/lib/i18n/en.ts` as `const en: Messages = {…}`, supplying every key declared by `Messages` (this is also the fallback source) (depends on T003).

**Checkpoint**: `npm run check` passes; `en.ts` satisfies `Messages`; the runtime is importable.

---

## Phase 3: User Story 1 - Read the whole app in Russian (Priority: P1) 🎯 MVP

**Goal**: Every screen renders through `t()`/`tn()`, a complete Russian catalog exists, and switching
`general.language` flips the entire UI live (no restart). Preset button labels localize while inserted
keyword values stay English.

**Independent Test**: Set Settings → Language to **Русский** → every menu, panel, dialog, the settings
screen, tooltips, and toast/validation messages read in Russian with no leftover English; set back to
**English** → reverts. (OS auto-detect is US2; here English default is acceptable.)

- [ ] T008 [US1] Author the full Russian catalog `src/lib/i18n/ru.ts` as `const ru: Messages = {…}`, providing Russian for every key in `Messages`, with correct one/few/many plural forms (FR-003/FR-013) (depends on T003, T007).
- [ ] T009 [P] [US1] Localize menu chrome: replace literal labels with `t()` in `src/routes/+page.svelte` (File, Open directory…, Theme + theme display names, Settings, Windows, Show Control, Show Hierarchy, Help, About) and any literals in `src/lib/menu/MenuBar.svelte`, `MenuItem.svelte`, `MenuTab.svelte`.
- [ ] T010 [P] [US1] Localize `src/lib/panel/FilesPanel.svelte` (Files, Table/Content/Icons, Vertical/Horizontal tooltips, "No folder open").
- [ ] T011 [P] [US1] Localize `src/lib/reusable/FileTree.svelte` (any user-facing literals/tooltips).
- [ ] T012 [US1] Localize `src/lib/panel/MetadataPanel.svelte` (largest): field labels (Title/Description/Keywords/Filename/rename on save/Categories/Release Filename/Stock Keywords/Optional/Required), buttons (Copy/Paste/Clear/Clear All/Save Changes/Cancel/Cancelling…), placeholders (Enter or ", " to add), validation messages (No file selected, Filename/Title/Description is required, At least one keyword is required, Save failed:), file-status labels (none/open/edit/batch) via `t()`, and count-bearing strings via `tn()` ("x of y", "{n} files", "Save {n} Files", word/char counts). Localize preset **category labels** (Nature/People/Urban/Concepts/Animals/Seasons) via `t()` while keeping the inserted keyword **values** as English constants (FR-014).
- [ ] T013 [P] [US1] Localize `src/lib/panel/ImageViewerPanel.svelte` (No image open, Loading preview, "was moved or deleted externally", Dismiss).
- [ ] T014 [P] [US1] Localize `src/lib/dialog/ConfirmDialog.svelte` (generic title/body/buttons it renders).
- [ ] T015 [P] [US1] Localize `src/lib/dialog/UnsavedChangesDialog.svelte` (Unsaved Changes, "{filename} has unsaved changes", Cancel/Discard/Save).
- [ ] T016 [P] [US1] Localize `src/lib/dialog/AboutDialog.svelte` (Version/Identifier/License labels + description + Close; keep dynamic version/identifier values as-is).
- [ ] T017 [P] [US1] Localize `src/lib/dialog/HelpDialog.svelte` chrome (title, Close) and change the doc fetch to a locale-aware path `/Help.${locale}.md` with fallback to `/Help.en.md`; rename `static/Help.md` → `static/Help.en.md`.
- [ ] T018 [P] [US1] Localize `src/lib/reusable/InputContextMenu.svelte` (Copy/Paste/Cut) — reuse `common.*` keys.
- [ ] T019 [P] [US1] Localize `src/lib/reusable/KeywordSuggestions.svelte` (any user-facing literals).
- [ ] T020 [P] [US1] Author the Russian Help document `static/Help.ru.md` mirroring `static/Help.en.md` (Constitution Comms & Docs: English is source of truth) (depends on T017 for the file convention).
- [ ] T021 [US1] Convert settings descriptors to message keys in `src/lib/settings/index.ts`: section labels (General/Editor/Appearance/Caching/Shortcuts) and each setting's `label`/`description` become stable `settings.*` keys (keep `options[].label` endonyms literal). Add the corresponding keys to `Messages`/`en.ts`/`ru.ts`.
- [ ] T022 [US1] Resolve registry labels at render time in `src/lib/settings/SettingsDialog.svelte`: wrap section labels and descriptor `label`/`description` in `t()`, and localize dialog chrome (Settings, Decrease, Increase, Reset to defaults) (depends on T021).
- [ ] T023 [US1] Convert shortcut action labels to message keys in `src/lib/shortcuts/registry.svelte.ts` and resolve them (plus page chrome: Press keys…, Already used by:, Reassign, Reset to defaults, Reset all shortcuts, Close) via `t()` in `src/lib/shortcuts/ShortcutsPage.svelte`. Add the keys to `Messages`/`en.ts`/`ru.ts`.

**Checkpoint**: With Russian selected the whole UI reads in Russian and switches live; `npm run check` passes.

---

## Phase 4: User Story 2 - Language choice is remembered (Priority: P2)

**Goal**: First-ever launch starts in the OS language (Russian OS → Russian, else English) with the first
painted frame already correct; an explicit choice persists and wins over detection on later launches.

**Independent Test**: Delete `general.language` from `settings.json` → launch on a Russian OS starts in
Russian (non-Russian → English); choose a language, restart → it persists and overrides OS detection.

- [ ] T024 [P] [US2] Add the `detect_os_locale() -> Result<String, String>` command in `src-tauri/src/lib.rs` using `sys_locale::get_locale()` — return `Ok(tag)`, or `Ok("en")` + a `log::warn!` when `None` (never `Err` for the absent case, never panic) — and register it in `tauri::generate_handler![…]` (contract: contracts/locale-command.md).
- [ ] T025 [US2] Add a first-run helper to `src/lib/settings/registry.svelte.ts` that reports whether a key was loaded from disk vs. defaulted (e.g. record loaded keys during `load()` and expose `wasPersisted(key): boolean`).
- [ ] T026 [US2] Implement `initLocale()` in `src/lib/i18n/index.ts`: if `settings.wasPersisted('general.language')` is false, `invoke<string>('detect_os_locale')` (catch → `'en'`), map the primary subtag to a `Locale` (unknown → `DEFAULT_LOCALE`), and `settings.set('general.language', mapped)`; log invoke failures via `@tauri-apps/plugin-log`. Idempotent (depends on T024, T025).
- [ ] T027 [US2] Call `await initLocale()` in `src/routes/+page.svelte` `onMount` immediately after `await settings.load()` and before `await win.show()`, so the first frame is already localized (depends on T026).

**Checkpoint**: First-run OS detection works, no language flash, and saved choices persist and win.

---

## Phase 5: User Story 3 - Ready for more languages without rework (Priority: P3)

**Goal**: Adding a language = one typed catalog file + registering it; missing keys fall back gracefully;
no screen edits required.

**Independent Test**: Add a throwaway `de.ts`, extend `LOCALES`; it appears in the selector and switches the
UI with zero screen edits; remove one `ru` key → that text shows the English fallback (build still flags it).

- [ ] T028 [US3] Drive the `general.language` selector options from `LOCALES` + `endonyms` (edit `src/lib/settings/index.ts` to build `options` from the i18n source of truth instead of hardcoding en/ru), so a newly added language appears automatically (FR-009/SC-005).
- [ ] T029 [US3] Verify and harden the missing-key fallback in `src/lib/i18n/store.svelte.ts` (`catalog[loc] → catalog[DEFAULT_LOCALE] → key`), and emit a guarded dev-only `log` warning when a fallback occurs (no `console.*`) (FR-007/SC-004).
- [ ] T030 [P] [US3] Add `src/lib/i18n/README.md` documenting how to add a language (create one `Messages` file, append to `LOCALES`, add the endonym) — English, as code docs.
- [ ] T031 [US3] Validate extensibility per quickstart Scenario 7: add a temporary `de.ts`, confirm it appears/switches with no screen edits and that `npm run check` enforces completeness, then remove it.

**Checkpoint**: Extensibility and fallback proven; all three stories independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final validation across all stories.

- [ ] T032 [P] Run `npm run check` (svelte-check) — the completeness gate passes with no missing/extra/mistyped keys (SC-005); fix any reported gaps in `en.ts`/`ru.ts`/`Messages`.
- [ ] T033 [P] Run `cargo check` and `cargo test` in `src-tauri/` — backend builds with `sys-locale` and the new command.
- [ ] T034 Run quickstart.md Scenarios 1–6 manually and confirm SC-001..SC-007 (whole-UI Russian, live switch, persistence, first-run detection, fallback, plurals, preset label/value split).
- [ ] T035 [P] Audit Constitution VI logging (warn path in `detect_os_locale`; frontend `initLocale` invoke-failure log) and confirm no `console.*` / `println!` / `dbg!` were introduced.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (P1)** → no deps; start immediately.
- **Foundational (P2)** → after Setup; **blocks all stories**. Internal order: T003 → (T004 [P]) → T006/T007 → T005.
- **US1 (P3)** → after Foundational. T008 (ru.ts) first; migrations T009–T023 then consume keys.
- **US2 (P4)** → after Foundational (independent of US1). T024 → T025 → T026 → T027.
- **US3 (P5)** → after Foundational; T028/T029 lightly touch foundational files; T031 validates.
- **Polish (P6)** → after the stories you intend to ship.

### Story independence

- **US1** needs only Foundational — switching works via the store's `$derived` over `general.language`.
- **US2** needs only Foundational — first-run detection + init ordering; does not depend on US1 migrations.
- **US3** needs only Foundational — extensibility/fallback; `T028` also assumes the selector exists (it does).

### Shared-file notes (avoid parallel conflicts)

- `src/lib/i18n/en.ts` & `ru.ts`: edited by T007/T008 and appended by T021/T023 (settings/shortcut keys) — serialize edits to these two files.
- `src/routes/+page.svelte`: T009 (menu) and T027 (init) touch it — sequential (different phases).
- `src/lib/settings/index.ts`: T021 (label keys) then T028 (selector from LOCALES) — sequential.
- `src/lib/i18n/store.svelte.ts`: T005 (create) then T029 (harden) — sequential.

## Parallel Opportunities

- Setup: T001 ∥ T002.
- US1 component migrations are mostly independent files: **T009, T010, T011, T013, T014, T015, T016, T017, T018, T019** can run in parallel after T008. (T012, T021→T022, T023 touch large/shared files — run on their own.)
- US2 T024 (Rust) can proceed in parallel with US1 work.
- Polish: T032 ∥ T033 ∥ T035.

### Parallel example — US1 migrations

```text
After T008 (ru.ts) is in place, launch together:
Task: "Localize FilesPanel.svelte"            (T010)
Task: "Localize ImageViewerPanel.svelte"      (T013)
Task: "Localize UnsavedChangesDialog.svelte"  (T015)
Task: "Localize AboutDialog.svelte"           (T016)
Task: "Localize HelpDialog.svelte"            (T017)
```

## Implementation Strategy

### MVP first (US1)

1. Phase 1 Setup → Phase 2 Foundational (runtime + English catalog).
2. Phase 3 US1: author `ru.ts`, migrate all screens, registries, presets, Help.
3. **STOP & VALIDATE**: switch to Russian → whole UI Russian; `npm run check` green. This is the shippable MVP.

### Incremental delivery

1. Foundational ready (engine works in English).
2. + US1 → live Russian UI (MVP).
3. + US2 → correct first-run OS default + remembered choice.
4. + US3 → proven extensibility + graceful fallback.
5. Polish → full quickstart validation.

## Notes

- `[P]` = different files, no incomplete-task dependency.
- Keep keyword **values** English (FR-014); localize only their category **labels**.
- Every new user-facing string MUST exist in both `en.ts` and `ru.ts` or `npm run check` fails (this is the
  intended completeness guard, SC-005).
- Commit after each logical group; the implement phase ends with the mandatory `plan`→`implement` phase commit
  (Constitution VII) and a passing `npm run check` / `cargo check` (Dev Workflow).
