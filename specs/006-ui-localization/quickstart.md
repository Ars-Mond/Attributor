# Quickstart & Validation: Russian UI Localization

**Feature**: 006-ui-localization | **Date**: 2026-06-26

Runnable scenarios that prove the feature works end to end. Details of the API live in
[contracts/](./contracts/) and [data-model.md](./data-model.md).

## Prerequisites

- Toolchain already set up for Attributor (Node + pnpm, Rust + Tauri 2).
- `sys-locale` added to `src-tauri/Cargo.toml`; `detect_os_locale` registered in `src-tauri/src/lib.rs`.
- `src/lib/i18n/` implemented and all screens migrated to `t()`/`tn()`.

## Build / run / check

```sh
# Type + completeness gate — the primary automated guard for this feature.
npm run check          # svelte-check: missing/extra/mistyped message key => build error

# Backend compiles with the new crate + command.
cargo test --manifest-path src-tauri/Cargo.toml

# Run the app for manual scenarios below.
npm run tauri dev
```

## Scenario 1 — Whole UI switches to Russian, live (US1 / FR-001..FR-004, SC-001, SC-002)

1. Launch the app (English).
2. Open **Settings → General → Language**, choose **Русский**.
3. **Expected**: without restarting, menus, panels, the settings dialog, every dialog, buttons, tooltips,
   placeholders, and toast/validation messages read in Russian. Switch back to **English** → all reverts.
4. Open a dialog (e.g. Unsaved Changes), then switch language while it is open → its text updates in place.

## Scenario 2 — Choice persists across restart (US2 / FR-005, SC-003)

1. Set language to **Русский**, close the app, reopen it.
2. **Expected**: it starts in Russian. Repeat with **English** → starts in English.

## Scenario 3 — First-run OS detection (FR-006, SC-007)

1. Close the app. Delete the `general.language` entry from `settings.json` (AppData) — or delete the file to
   simulate a first-ever launch.
2. On a Russian-language OS, launch → **Expected**: UI starts in Russian. On a non-Russian OS → starts in
   English. The first painted frame is already in the detected language (no English flash).
3. Change the language in Settings, restart → the saved choice wins over OS detection.

## Scenario 4 — Missing-key fallback (FR-007, SC-004)

1. Temporarily remove one key from `ru.ts` value (or set it absent) — note this normally fails `npm run check`;
   for a manual demo use a key only present in `en.ts` via a throwaway language.
2. **Expected**: that one text renders the English (default-language) string, never blank or a raw key.
   No layout breakage.

## Scenario 5 — Russian plurals (FR-013, SC-006)

1. With Russian active, exercise count-bearing texts (batch select 1, then 2, then 5 files; keyword counts).
2. **Expected**: «1 файл», «2 файла», «5 файлов», «11 файлов», «21 файл» — correct one/few/many forms; the
   count value is correctly placed in each form.

## Scenario 6 — Preset labels localized, keyword values stay English (FR-014)

1. With Russian active, open the metadata panel keyword presets.
2. **Expected**: category **labels** (Nature/People/…) appear translated, but clicking a preset inserts the
   original **English** keyword values into the metadata (e.g. `nature`, `landscape`), unchanged.

## Scenario 7 — Add a language with no screen edits (US3 / FR-009, SC-005)

1. Add a throwaway `de.ts` implementing `Messages`, extend `LOCALES` and the selector options.
2. **Expected**: `npm run check` passes only when `de.ts` is complete; the new language appears in the
   selector and switches the whole UI — with zero edits to any screen/component. (Only en+ru ship; remove
   the throwaway afterward.)

## Done / acceptance

- `npm run check` and `cargo test` pass.
- Scenarios 1–7 behave as described; no leftover English chrome with Russian selected (SC-001).
