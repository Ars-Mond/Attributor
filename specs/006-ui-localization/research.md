# Phase 0 Research: Russian UI Localization

**Feature**: 006-ui-localization | **Date**: 2026-06-26 | **Spec**: [spec.md](./spec.md)

This document records the technical decisions that resolve the unknowns in the plan's Technical
Context. Each decision lists what was chosen, why, and the alternatives rejected.

## Decision 1 — Localization layer: custom typed runes module (no external i18n library)

**Decision**: Build a small, in-house localization layer in a dedicated `src/lib/i18n/` folder rather
than adopt an i18n library. It consists of: a `$state`-backed active locale, two typed message catalogs
(`en.ts`, `ru.ts`) that both implement one shared `Messages` TypeScript type, a reactive `t()` accessor
that reads the locale rune, a default-language→key fallback, and a tiny Russian plural helper.

**Rationale**:

- **Runes-native (Constitution II)**: the active locale is `$state`; `t()` is a plain function whose call
  inside markup / `$derived` tracks that rune, giving an immediate live switch with no restart or page
  reload (FR-004). No legacy `writable`/`readable` store drives component state. This mirrors the project's
  existing `SettingsRegistry` (`src/lib/settings/registry.svelte.ts`), itself a custom `$state`-backed
  reactive class exposing a `subscribe(key)` getter consumed inside `$derived` — a trusted in-repo pattern.
- **Compile-time completeness (FR-010, SC-005)**: because `en.ts` and `ru.ts` are typed `const en: Messages`
  / `const ru: Messages`, the existing `svelte-check` step flags any missing or extra key. A screen cannot
  reference an undefined text, and adding a language = adding one typed file (no screen edits).
- **Zero new frontend dependencies (Constitution X)**: reuses the existing `general.language` setting and
  `tauri-plugin-store`. Nothing added to `package.json`.
- **Fallback & plurals are trivial to own**: default-language→readable-id fallback (FR-007/SC-004) and the
  Russian one/few/many forms (FR-013) are a few dozen lines we control.
- **Preset split (FR-014) is free**: localized category labels live in the catalog; the inserted keyword
  values stay plain English constants in the preset definitions — already structurally separate today.

**Alternatives considered**:

| Option | Typed / completeness | Runes-native | Live switch | RU plurals | New deps | Verdict |
|--------|----------------------|--------------|-------------|------------|----------|---------|
| **Custom runes module** | Best (shared `Messages` + svelte-check) | Yes, native | Yes, instant | Yes (~15-line helper) | None | **Chosen** |
| svelte-i18n 4.0.1 | Weak (runtime string keys) | No — Svelte stores | Yes | Yes (ICU) | +1 npm, ~2y stale | Rejected: store-based (II), stale, dep for in-repo capability |
| @inlang/paraglide-js 2.x | Strong (compiled typed fns) | Awkward — not rune-reactive | Reloads page by default | Possible, not ergonomic | +1 npm + Vite build step | Rejected: reload-by-default + self-wired reactivity + build-pipeline dep; routed-web oriented |
| typesafe-i18n | Best-in-class | Not first-class | Yes | Yes | +1 npm + codegen/watcher | Rejected: unmaintained, codegen step, dep for type-safety we already get |

## Decision 2 — OS-language detection: `sys-locale` Rust crate + one typed Tauri command

**Decision**: Detect the OS language in the backend with the pure-Rust `sys-locale` crate (v0.3), exposed
through a single typed command `detect_os_locale() -> Result<String, String>` returning a BCP-47 tag (e.g.
`"ru-RU"`). The frontend maps the primary language subtag (`ru` → Russian, otherwise English) on first
launch only.

**Rationale**:

- **Pure Rust (Constitution I)**: `sys-locale` is pure Rust with zero dependencies on the Windows/macOS/Linux
  desktop targets (only `libc` on Android / `web-sys` on WASM, neither of which applies here).
- **Minimal footprint (Constitution X)**: one tiny crate (~29 KB) versus an npm + Rust-plugin pair.
- **Correctness on the primary target**: `navigator.language` is documented as unreliable in WebView2 on
  Windows — it does not reliably reflect the OS/system language, which is exactly the Russian-OS case FR-006
  cares about. Windows is Attributor's primary target, so the frontend-only route would silently mis-detect.
- **Typed IPC (Constitution IX)**: the command returns `Result<String, String>`, never panics
  (`get_locale()` returns `Option`, mapped to `Ok(tag)` or a graceful default), and adds no fields needing
  camelCase. It logs on the unexpected/empty path (Constitution VI).
- **First-run only, overridable**: detection runs once, when `general.language` has no saved value. Any
  explicit user choice is persisted and always wins (FR-006), so a wrong guess is trivially corrected.

**Alternatives considered**:

- **`navigator.language` (frontend, zero-dep)** — rejected: unreliable in WebView2 on Windows (primary target).
- **`@tauri-apps/plugin-os` `locale()`** — rejected: adds both an npm package and a Rust plugin for one call;
  the project uses neither today (store/dialog/log/opener/clipboard/single-instance/prevent-default only).

## Decision 3 — Russian pluralization (CLDR cardinal, integers)

**Decision**: All app counts are integers (file counts, keyword counts), so `v = 0` always holds and
Russian's decimal `other` category never occurs — a clean one/few/many mapping. Each pluralized message key
carries the forms it needs: Russian `{one, few, many}`, English `{one, other}`; the helper selects by count
and the count is interpolated back into the chosen form (FR-008/FR-013/SC-006).

**Russian rule** (integer `n`, `m10 = n % 10`, `m100 = n % 100`):

- `one` — `m10 === 1 && m100 !== 11` (1, 21, 31, 101 → «1 файл»)
- `few` — `m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)` (2, 3, 4, 22… → «2 файла»)
- `many` — everything else: `m10 === 0`, `m10` in 5..9, or `m100` in 11..14 (0, 5..20, 25..30 → «5 файлов»)

**English rule**: `one` when `n === 1`, otherwise `other`.

## Decision 4 — Persistence, reactivity & first-render correctness

**Decision**: Reuse the existing `general.language` setting verbatim (FR-005, persisted via
`tauri-plugin-store`). Components read the active locale reactively through the i18n module's `$state`; a
single root-level `$effect` (in `+layout.svelte`) keeps that `$state` in sync with
`settings.subscribe('general.language')()`. An imperative `initLocale()` runs in `+page.svelte` `onMount`
**after `settings.load()` and before `win.show()`**, so the first painted frame is already in the right
language (no flash).

**First-run detection**: the registry only loads keys present in the stored `settings` object, so an absent
`general.language` key means "user never chose". `initLocale()` checks this (via a small registry helper that
reports whether a key was loaded from disk vs. defaulted), and on first run calls `detect_os_locale`, maps the
subtag, and `settings.set('general.language', mapped)`. On subsequent runs the saved value is used as-is.

**Rationale**: keeps the language a first-class setting (already shown in the settings dialog), avoids a
second persistence mechanism, and guarantees correct first paint by ordering the init before `win.show()`.

## Decision 5 — Scope of strings & registry-driven labels

**Inventory** (read-only scan, ~150–170 hardcoded UI strings): menu/menubar, files panel & file tree,
metadata/editor panel (largest, ~65), image viewer, confirm/unsaved/clear dialogs, about/help dialogs,
shortcuts page, settings dialog chrome, input context menu, preset category labels, and field
labels/validation messages. All are currently hardcoded in component templates/scripts; none externalized.

**Registry-driven labels** (settings & shortcuts): setting/section `label` and `description` strings and
shortcut action labels are registered once at module load, so they must be resolved at **render time** to
react to a language switch. Decision: these descriptors carry stable **message keys**, and the rendering
sites (`SettingsDialog.svelte`, `ShortcutsPage.svelte`) resolve them via `t()`. The language `<select>`
option labels stay literal endonyms ("English", "Русский") — language names are not translated.

**Enum-style display labels** (theme names, file-status `none/open/edit/batch`, view modes): the internal
enum value stays the key; only the displayed label is localized via `t()`.

**Help document body**: the Help dialog renders `Help.md`. Decision: load a locale-specific document
(`Help.en.md` / `Help.ru.md`) with English fallback, satisfying the constitution's bilingual-docs rule. The
dialog chrome (title, Close) is localized via `t()`. Authoring the Russian Help document is a feature task;
its prose is content, not a typed UI key.

**Out of scope** (confirmed): inserted keyword **values** (stay English, FR-014), user-entered metadata,
file names, backend-driven keyword suggestions, and the dynamically pulled app version/identifier.
