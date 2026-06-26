# Contract: Frontend i18n module (`src/lib/i18n/`)

The public surface of the localization layer. Screens depend only on this contract, never on catalog
internals. All identifiers are TypeScript; comments/code are English (Constitution XI).

## Types

```ts
export type Locale = 'en' | 'ru';
export const LOCALES: Locale[];          // drives the settings selector; append to add a language
export const DEFAULT_LOCALE: Locale;     // 'en' — fallback for missing keys / unsupported OS langs

// Plural form sets — a catalog must supply the forms its language uses.
export type PluralForms =
  | {one: string; other: string}                 // English shape
  | {one: string; few: string; many: string};    // Russian shape

// The single typed contract every catalog implements (nested, grouped by area).
export interface Messages { /* fully typed keys; leaves are string | PluralForms */ }
```

## Accessors (reactive)

```ts
// Active locale (reads the $state rune; reactive in markup / $derived).
export function locale(): Locale;

// Translate a key. Reactive: re-runs when the active locale changes.
// - Interpolates {name} placeholders from params.
// - Fallback order: catalog[locale][key] -> catalog[DEFAULT_LOCALE][key] -> the key string itself.
export function t(key: MessageKey, params?: Record<string, string | number>): string;

// Plural-aware translate. Picks the form by CLDR category for the active locale, then interpolates
// (the count is available as {n} in the chosen form).
export function tn(key: PluralKey, count: number, params?: Record<string, string | number>): string;
```

- `MessageKey` / `PluralKey` are typed key unions derived from `Messages`, so callers get autocomplete and a
  compile error on an unknown key (FR-010).
- `t`/`tn` MUST never throw and MUST never return `undefined`/blank: a missing key resolves to the
  default-language text and, failing that, the readable key string (FR-007/SC-004).

## Mutators & lifecycle

```ts
// Set the active locale (normalizes unknown input to DEFAULT_LOCALE). Used by the root sync $effect.
export function setLocale(value: string): void;

// One-time startup init. MUST run after settings.load() and before the window is shown.
// - If general.language was NOT persisted (first run): invoke detect_os_locale, map the primary
//   subtag to a Locale, and settings.set('general.language', mapped).
// - Then set the active locale from general.language. Idempotent.
export function initLocale(): Promise<void>;
```

## Pluralization helper

```ts
// CLDR cardinal category for an integer count (rules: research.md Decision 3).
export function pluralCategory(loc: Locale, n: number): 'one' | 'few' | 'many' | 'other';
```

## Reactivity contract

- The active locale is a module-level `$state`. `t()`/`tn()`/`locale()` read it, so any markup or `$derived`
  that calls them re-evaluates on a language switch with no restart/reload (FR-004/SC-002).
- A single `$effect` in `+layout.svelte` mirrors `settings.subscribe('general.language')()` into `setLocale`,
  so a change made in the settings dialog propagates app-wide.

## Catalog completeness contract

- `en.ts` and `ru.ts` are declared `const <name>: Messages`. `npm run check` (svelte-check) MUST pass; a
  missing/extra/mistyped key is a build error (SC-005). Adding a locale = add one `Messages` file + extend
  `LOCALES` and the settings selector options — no screen edits (FR-009).
