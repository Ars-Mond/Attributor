# Implementation Plan: Russian UI Language (Localization)

**Branch**: `006-ui-localization` | **Date**: 2026-06-26 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/006-ui-localization/spec.md`

## Summary

Make the existing `general.language` setting actually localize the whole application and supply a complete
Russian translation, with a structure that stays open to more languages. The localization layer is a small,
in-house, **typed runes module** in `src/lib/i18n/` (no external i18n library): a `$state`-backed active
locale, two catalogs (`en.ts`, `ru.ts`) implementing one shared `Messages` type (so a missing key is a
compile error), a reactive `t()`/`tn()` accessor with default-language→key fallback, and a Russian
one/few/many plural helper. First-launch language is auto-detected from the OS via one pure-Rust command
(`detect_os_locale`, `sys-locale` crate) and is overridable; the choice persists in the existing setting.
See [research.md](./research.md) for the decisions and rejected alternatives.

## Technical Context

**Language/Version**: Rust (edition 2021) backend; TypeScript ~5.6 + Svelte 5 (runes) frontend; Tauri 2.

**Primary Dependencies**: Existing stack (SvelteKit, `@tauri-apps/*`, `tauri-plugin-store`). **One new Rust
crate**: `sys-locale = "0.3"` (pure Rust) for first-run OS-language detection. **No new npm dependency.**

**Storage**: Existing `general.language` key in `settings.json` via `tauri-plugin-store`. No new schema.

**Testing**: `npm run check` (svelte-check) is the primary automated guard — it enforces catalog completeness
(a missing/extra message key is a build error). `cargo test` for backend compile/command. Manual validation
per [quickstart.md](./quickstart.md). (vitest/playwright remain out of scope per prior project decision.)

**Target Platform**: Windows, macOS, Linux desktop (Tauri 2 webview).

**Project Type**: Desktop application (Tauri 2 + SvelteKit, single window).

**Performance Goals**: Language switch re-renders within one frame (instant, no restart/reload); no
measurable startup cost — the OS-detection command runs once, only on first launch.

**Constraints**: No app restart for a switch (FR-004); first painted frame already localized (init before
`win.show()`); pure-Rust backend (I); runes-only frontend (II); minimal/justified deps (X).

**Scale/Scope**: ~150–170 hardcoded UI strings today across menu, panels, dialogs, viewer, shortcuts, and
metadata/editor (largest), plus settings/shortcuts registry labels and the Help document; 2 locales (en, ru),
extensible to more.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | How this plan complies |
|-----------|--------|------------------------|
| I. Pure Rust Backend | ✅ | `sys-locale` is pure Rust, zero transitive deps on desktop targets. No C/FFI. |
| II. Modern Svelte 5 (Runes) | ✅ | Active locale is `$state`; `t()` tracked in markup/`$derived`; root `$effect` for sync. No legacy stores. |
| III. Themed SCSS Tokens | ✅ | No new styling beyond existing components; any touch uses existing tokens. |
| IV. Cross-Platform Parity | ✅ | Same detection/translation behavior on all OSes; `sys-locale` is cross-platform; no per-OS feature gating. |
| V. Reuse UI Primitives | ✅ | Reuses the existing settings `<select>`, dialogs, menu, file tree; no parallel components. |
| VI. Mandatory Logging | ✅ | `detect_os_locale` logs the warn/absent path (Rust `log`); frontend init logs invoke failure (`@tauri-apps/plugin-log`). |
| VII. Phase-Based Commits | ✅ | This plan is committed as the `plan` phase via `/speckit-git-commit`. |
| VIII. Rust Performance First | ✅ | Detection is one trivial Rust call on first run; no hot loop, no per-item IPC. |
| IX. Typed Tauri IPC | ✅ | `detect_os_locale() -> Result<String, String>`, never panics; no fields to camelCase. |
| X. Fixed Stack | ✅ | Zero new npm deps; one tiny justified Rust crate (see research.md Decision 2). |
| XI. Code Style | ✅ | English comments/identifiers; no inner brace spaces in TS; no padding alignment. |
| Comms & Docs | ✅ | Help document localized (`Help.en.md`/`Help.ru.md`), English as source of truth; `npm run check` after frontend edits. |

**Result**: PASS — no violations. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/006-ui-localization/
├── plan.md              # This file
├── research.md          # Phase 0 — decisions & alternatives
├── data-model.md        # Phase 1 — entities & state
├── quickstart.md        # Phase 1 — validation scenarios
├── contracts/
│   ├── i18n-frontend.md # Phase 1 — i18n module public API
│   └── locale-command.md# Phase 1 — detect_os_locale IPC contract
├── checklists/
│   └── requirements.md  # Spec quality checklist (from /speckit-specify, updated in /clarify)
└── tasks.md             # Phase 2 — created by /speckit-tasks (NOT here)
```

### Source Code (repository root)

```text
src/lib/i18n/                 # NEW — the dedicated typed translation folder
├── types.ts                 # Locale, LOCALES, DEFAULT_LOCALE, Messages, PluralForms, MessageKey/PluralKey
├── en.ts                    # const en: Messages  (also the fallback source)
├── ru.ts                    # const ru: Messages
├── catalog.ts               # Record<Locale, Messages>
├── plural.ts                # pluralCategory(loc, n) — CLDR one/few/many (ru), one/other (en)
├── store.svelte.ts          # $state active locale + setLocale + locale() + t() + tn() + interpolation
└── index.ts                 # re-exports + initLocale() (first-run detect + sync)

src/lib/                      # EDIT — replace hardcoded strings with t()/tn()
├── menu/{MenuBar,MenuItem,MenuTab}.svelte
├── panel/{FilesPanel,MetadataPanel,ImageViewerPanel}.svelte
├── reusable/{FileTree,InputContextMenu,KeywordSuggestions}.svelte
├── dialog/{ConfirmDialog,UnsavedChangesDialog,AboutDialog,HelpDialog}.svelte
├── shortcuts/{ShortcutsPage.svelte, registry.svelte.ts}   # action labels -> message keys resolved via t()
└── settings/{SettingsDialog.svelte, index.ts}             # section/setting labels -> keys resolved via t()

src/routes/
├── +page.svelte             # EDIT — call initLocale() after settings.load(), before win.show()
└── +layout.svelte           # EDIT — root $effect syncing setLocale(<- general.language)

static/ (or wherever Help loads)
├── Help.en.md               # EDIT/RENAME — English Help (source of truth)
└── Help.ru.md               # NEW — Russian Help; HelpDialog loads by active locale, English fallback

src-tauri/
├── Cargo.toml               # EDIT — add sys-locale = "0.3"
└── src/lib.rs               # EDIT — add detect_os_locale command + register in generate_handler!
```

**Structure Decision**: Single desktop app (existing layout). The only new directory is `src/lib/i18n/` —
the dedicated, typed translation collection the spec calls for (FR-010). Everything else is in-place edits to
existing components plus one tiny backend command; no new architectural layers, honoring "Reuse UI
Primitives" (V) and "Fixed Stack" (X).

## Complexity Tracking

> No constitution violations — table intentionally empty.
