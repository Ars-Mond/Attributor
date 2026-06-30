# Quickstart & Validation: CSV Export

A run/validation guide proving the feature end to end. Details live in [spec.md](./spec.md), [data-model.md](./data-model.md), and [contracts/ipc-commands.md](./contracts/ipc-commands.md).

## Prerequisites

- `pnpm install` done; app runs via `pnpm tauri dev`.
- A folder of photos (JPEG/PNG/WebP) where several photos have **saved** metadata (open/edit/save or run attribution so the SQLite store has records).
- Build/check gates: `cargo test` (in `src-tauri`) and `npx svelte-check --tsconfig ./tsconfig.json` both pass.

## Validate US1 — Configure stock CSV presets (P1)

1. Open **Settings → CSV** (new category, ordered after Ollama models).
2. Create a preset: set **name** = "Test Stock", **identifier** = "teststock", **delimiter** = comma.
3. Add fields: `Filename`→file name, `Title`→title, `Keywords`→keywords, `Editorial`→editorial (set bool format = yes/no), `Lic`→none (default value = "RF").
4. Confirm: the **default value** input appears only for the `none` field; the **bool format** control appears only for the editorial field.
5. Reorder a field up/down with the arrows; confirm order changes.
6. Try identifier "teststock" on a second preset → rejected as duplicate. Try `bad/name` → rejected as invalid file name.
7. Restart the app → the preset and its exact field order persist.

**Expected**: preset persists with correct conditional inputs, reordering, and validation (FR-016…FR-027, FR-034).

## Validate US2 — Export to CSV (P2)

1. With ≥1 preset configured, open a folder of photos (some with store records).
2. **File → Export to CSV** → choose a destination folder.
3. Confirm a `<identifier>.csv` is written for each preset.
4. Open a file: row 1 = the field headers in order; one data row per in-scope photo **that has a store record**; cells match the database (title/keywords/etc.), the `none` column shows "RF", the editorial column shows `yes`/`no`.
5. Edit a photo so a value contains a comma, a `"`, and a newline; re-export; open the file in a spreadsheet → it parses correctly (quoted).
6. Remove all presets → Export to CSV shows a "nothing to export" warning and writes no files.

**Expected**: correct files, headers, DB-sourced cells, bool/none formatting, RFC 4180 escaping, and the no-presets guard (FR-001…FR-015, FR-029, FR-031, FR-033, SC-002…SC-007).

## Validate US3 — Export scope (P3)

1. Select 3 of 10 photos → Export → each CSV has exactly 3 data rows (the selected ones).
2. Clear selection; ensure the folder has a sub-folder with photos → Export → each CSV has exactly the root folder's photos, none from the sub-folder.
3. In a folder where 2 of 10 in-scope photos have **no** store record → Export → each CSV has 8 rows and the result dialog reports "2 skipped".

**Expected**: selection vs. folder scope, non-recursive, and skipped-count reporting (FR-003…FR-005, FR-035, SC-002).

## Validate delimiter (clarified behavior)

1. Set a preset's delimiter to **semicolon**; export → its CSV uses `;` between columns; a keywords cell still joins keywords with `,` inside the cell.
2. Set delimiter to **tab**; export → tab-separated columns, same UTF-8 + quoting rules.

**Expected**: per-preset delimiter with global UTF-8 + RFC 4180 quoting (FR-034, Clarifications).

## Automated checks

- `cargo test` covers: `writer.rs` (quoting for delimiter/quote/newline; comma/semicolon/tab), `cell.rs` (each value type incl. bool formats and keyword join), identifier sanitization, and `DbState::fetch` returning `Some`/`None` without mutating the DB.
- `npx svelte-check` passes with the new i18n keys present in both `en.ts` and `ru.ts`.
