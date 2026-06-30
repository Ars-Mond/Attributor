# Phase 1 Data Model: Russian UI Localization

**Feature**: 006-ui-localization | **Date**: 2026-06-26

This feature adds a frontend localization layer plus one backend detection command. "Data" here is mostly
in-memory typed structures (translation catalogs) and one persisted setting; there is no new on-disk schema
beyond the existing `general.language` key in `settings.json`.

## Entities

### Locale

A selectable interface language.

| Field | Type | Notes |
|-------|------|-------|
| code | `'en' \| 'ru'` (`Locale` union) | Stable BCP-47 primary subtag; the persisted value of `general.language`. |
| endonym | string | Human-readable name shown in the selector ("English", "Русский"); NOT translated. |

- `DEFAULT_LOCALE = 'en'` — the fallback language for missing keys and for unsupported OS languages.
- `LOCALES: Locale[]` — the list driving the settings selector; adding a language appends here (FR-009/FR-011).
- Validation: any string read from settings or the OS that is not in `LOCALES` normalizes to `DEFAULT_LOCALE`.

### Messages (translation contract)

The shared TypeScript type that every catalog must implement. It is the single source of truth for the set of
text keys; it makes a missing/extra key a compile error (FR-010/SC-005).

- Shape: a nested, fully-typed object of message keys, e.g. grouped by area
  (`menu`, `files`, `metadata`, `viewer`, `dialog`, `settings`, `shortcuts`, `common`, …).
- Leaf value is either:
  - a **plain string** (optionally with `{name}` placeholders for interpolation), or
  - a **plural set** `{one, few, many}` (RU) / `{one, other}` (EN) — typed as a `PluralForms` shape so both
    catalogs must supply the forms their language requires.
- Validation rule: `const en: Messages` and `const ru: Messages` — `svelte-check` enforces completeness.

### Translation catalog

The concrete per-language data implementing `Messages`.

| Field | Type | Notes |
|-------|------|-------|
| en | `Messages` | English catalog (`en.ts`) — also the fallback source. |
| ru | `Messages` | Russian catalog (`ru.ts`). |
| catalog | `Record<Locale, Messages>` | Lookup by active locale (`catalog.ts`). |

### Active locale (runtime state)

The currently displayed language.

| Field | Type | Notes |
|-------|------|-------|
| value | `Locale` (`$state`) | Drives every `t()` call; changing it re-renders all dependent markup (FR-004). |
| initialized | boolean | Guards `initLocale()` against running twice. |

- State transitions:
  - **First launch** (no saved `general.language`): `initLocale()` → `detect_os_locale` → map subtag →
    `settings.set('general.language', mapped)` → `value = mapped`.
  - **Subsequent launch**: `value = normalize(settings.get('general.language'))`.
  - **User switch** (settings dialog): `settings.set('general.language', x)` → root `$effect` → `value = x`.
- Source of truth is the `general.language` setting; `value` is kept in sync with it.

### Plural category

A derived classification of a count, used to pick a plural form.

| Field | Type | Notes |
|-------|------|-------|
| category | `'one' \| 'few' \| 'many' \| 'other'` | Computed per locale from an integer count (rules in research.md, Decision 3). |

### OS locale (transient)

The BCP-47 tag returned by the backend on first run only.

| Field | Type | Notes |
|-------|------|-------|
| tag | string | e.g. `"ru-RU"`; the frontend keeps only the primary subtag and discards the rest. |

## Persisted state

- **`general.language`** (existing `settings.json` key, `tauri-plugin-store`): `'en' | 'ru'`, default `'en'`.
  - Absence of the key ⇒ first run ⇒ OS detection. Presence ⇒ explicit choice ⇒ used as-is (FR-005/FR-006).

## Relationships

```text
general.language (settings) ──sync──> Active locale ($state)
                                            │
                                            ├─ selects ─> catalog[locale] : Messages
                                            │                   │
                              t(key, params) ────────────────────┘  (fallback: catalog[DEFAULT][key] -> key)
                                            │
                              plural(locale, count) ─> PluralForms[category]
detect_os_locale() (first run only) ─maps subtag─> general.language
```
