# i18n — UI localization

A small, typed, runes-based localization layer. No external i18n library. The active locale is derived
from the `general.language` setting, so calling `t()`/`tn()` in markup re-renders instantly on a language
switch (no restart).

## Usage

```svelte
<script lang="ts">
  import {t, tn} from '$lib/i18n';
</script>

<h2>{t('metadata.title')}</h2>
<span>{t('dialog.unsavedChanges.body', {filename})}</span>   <!-- {name} interpolation -->
<span>{tn('metadata.batch.fileCount', count)}</span>          <!-- plural, count exposed as {n} -->
```

- `t(key, params?)` — plain string keys. `tn(key, count, params?)` — plural keys (`{n}` is the count).
- Missing key → falls back to the default language, then to the key string (never blank). In dev a one-time
  `warn` is logged for a missing key.
- Registry-driven labels (settings/shortcuts) store a message key as their `label`/`description`; the render
  site resolves it with `t(label as MessageKey)`.

## Files

| File | Purpose |
|------|---------|
| `types.ts` | `Locale`, `LOCALES`, `DEFAULT_LOCALE`, `ENDONYMS`, `PluralForms`, the `Messages` contract, key types |
| `en.ts`, `ru.ts` | per-language catalogs implementing `Messages` |
| `catalog.ts` | `Record<Locale, Messages>` |
| `plural.ts` | CLDR cardinal category (ru one/few/many, en one/other) |
| `store.svelte.ts` | `$derived` active locale, `t`, `tn`, `setLocale`, interpolation, fallback |
| `index.ts` | public re-exports + `initLocale()` (first-run OS detection) |

## Adding a language

No screen edits are required — only data:

1. Add the code to the `Locale` union and to `LOCALES` in `types.ts`, plus its endonym in `ENDONYMS`
   (e.g. `de: 'Deutsch'`).
2. Create `de.ts` exporting `const de: Messages = {…}` — implement **every** key. `npm run check`
   (svelte-check) fails until it is complete; that is the completeness guarantee.
3. Add `de` to the `catalog` map in `catalog.ts`.
4. (Optional) add `Help.de.md` under `static/` — the Help dialog loads `Help.<locale>.md` with an English
   fallback.

The settings language selector is built from `LOCALES`/`ENDONYMS`, so the new language appears automatically.
For Russian-style plural rules of a new language, extend `pluralCategory` in `plural.ts`.

## Scope

Only the application's own chrome is localized. Inserted keyword **values** (preset buttons), user-entered
metadata, and file names stay as-is.
