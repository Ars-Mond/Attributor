# Feature Specification: Russian UI Language (Localization)

**Feature Branch**: `006-ui-localization`

**Created**: 2026-06-26

**Status**: Draft

**Input**: User description: "Add a Russian language option to the Tauri UI. Likely needs a localization library/plugin for the frontend, since more languages may be added later. Good practice: a dedicated folder holding the translation objects/structures (typed). The settings already have a language-switching block. Only Russian is planned for now (don't write other languages), but the ability to add another language later must remain."

## Clarifications

### Session 2026-06-26

- Q: Which interface language should the app use on a first-ever launch (before any choice is saved)? → A: Auto-detect from the operating system — start in Russian when the OS language is Russian, otherwise English.
- Q: How should the keyword-preset buttons (categories like Nature/Architecture) be localized? → A: Translate the button labels (they are UI chrome), but keep the keyword values they insert into metadata in English (they are stock-photo data).
- Q: How should Russian plurals be handled (1 файл / 2 файла / 5 файлов)? → A: Use the full, correct Russian plural forms (one/few/many), not a single simplified form.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read the whole app in Russian (Priority: P1)

A user who prefers Russian opens the settings, picks Russian as the interface language, and the entire
app interface — menus, panels, the settings screen, dialogs, buttons, tooltips, field labels, and
status/toast messages — is shown in Russian. Switching back to English restores the English interface.

**Why this priority**: This is the entire point of the feature — a usable Russian interface. Without it
there is nothing to deliver.

**Independent Test**: Set the language to Russian → every standard screen (menu, files panel, metadata
panel, viewer, settings, every dialog) reads in Russian with no leftover English. Set it back to
English → everything reads in English.

**Acceptance Scenarios**:

1. **Given** the language is English, **When** the user selects Russian in settings, **Then** the visible interface text switches to Russian without restarting the app.
2. **Given** the language is Russian, **When** the user opens any dialog, menu, or panel, **Then** its labels, buttons, and messages are in Russian.
3. **Given** the language is Russian, **When** the user selects English, **Then** the interface returns to English.

---

### User Story 2 - Language choice is remembered (Priority: P2)

A user sets the interface to Russian, closes the app, and reopens it later. The app comes back up in
Russian without having to choose again.

**Why this priority**: A language preference that resets every launch is effectively unusable; it must
persist. Builds directly on US1.

**Independent Test**: Select Russian, restart the app → it starts in Russian. Select English, restart →
it starts in English.

**Acceptance Scenarios**:

1. **Given** the user selected Russian, **When** the app is restarted, **Then** it starts with the interface in Russian.
2. **Given** a first-ever launch (no prior choice), **When** the app starts, **Then** it starts in the operating-system language if that language is supported (Russian), otherwise in English.

---

### User Story 3 - Ready for more languages without rework (Priority: P3)

As a maintainer, adding a future language (e.g. German) requires only providing that language's set of
translated texts and registering it as a choice — no edits to the individual screens or components, and
no risk of a half-translated screen, because the structure keeps every text in one organized place.

**Why this priority**: The user explicitly wants the design to stay open to more languages; getting the
structure right now avoids an expensive retrofit later. Not user-visible on its own, so lowest priority.

**Independent Test**: Add a throwaway extra language set, register it; it appears as a selectable option
and switches the UI, with no changes needed in any screen/component. (Only Russian and English ship.)

**Acceptance Scenarios**:

1. **Given** the localization structure, **When** a maintainer adds a new language's translation set, **Then** it becomes selectable and drives the UI without changing the screens that display text.
2. **Given** a translation set, **When** a text is missing from it, **Then** the UI shows a graceful fallback (the default-language text) rather than a blank or a raw identifier.

---

### Edge Cases

- A text has no Russian translation (a key was missed) → the UI falls back to the English text (or, failing that, a readable identifier), never a blank space or broken layout.
- The language is switched while a dialog or panel is already open → its visible text updates to the new language without needing to reopen it.
- Texts that embed dynamic values (counts, file names, "Saving 3 of 12", "N keywords") → the dynamic parts are preserved and correctly placed within the translated text.
- Russian text is often longer than English → longer strings wrap or truncate within existing controls without breaking the layout.
- Quantity-dependent text (e.g. "1 file" vs "5 files") → reads naturally in the selected language (Russian has multiple plural forms).
- User-entered content and domain data (file names, metadata values the user typed, photo keyword values) are NOT translated — only the application's own interface text is localized.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The application MUST display all of its own user-facing interface text (menus, panel labels and headers, the settings screen, dialogs, buttons, tooltips, placeholders, and status/toast/validation messages) in the language selected in settings.
- **FR-002**: The settings MUST offer a language selector that includes at least English and Russian (the existing language block is reused).
- **FR-003**: Russian translations MUST be provided for every user-facing interface text the app currently shows in English.
- **FR-004**: Changing the selected language MUST update the visible interface immediately, without restarting the application.
- **FR-005**: The selected language MUST persist across application restarts.
- **FR-006**: On a first-ever launch (no saved choice), the application MUST start in the operating-system language when it is a supported language (Russian OS → Russian); otherwise it MUST fall back to the default language (English). Once the user makes an explicit choice, that saved choice takes precedence over OS detection.
- **FR-007**: When a text is missing in the selected language, the UI MUST fall back to the default-language text (and, if that is also missing, a readable identifier) so no blank or broken text is shown.
- **FR-008**: Texts that contain dynamic values MUST keep those values intact and correctly positioned within the translated text across languages.
- **FR-009**: Adding a new language later MUST require only providing that language's translation set and registering it as a selectable option — no changes to the individual screens/components that display text.
- **FR-010**: The interface text MUST be kept in a single, organized, typed collection (separate from the screens), so missing or extra texts are detectable and a screen cannot silently reference an undefined text.
- **FR-011**: Only English and Russian are delivered in this feature; no other language data is created.
- **FR-012**: Localization MUST cover only the application's own interface text, not user-entered content or domain data (file names, metadata values, keyword values).
- **FR-013**: Quantity-dependent texts MUST use the selected language's correct plural forms — for Russian, the full one/few/many forms (e.g. 1 файл / 2 файла / 5 файлов), not a single simplified form.
- **FR-014**: Keyword-preset category buttons MUST have their labels localized as interface text, while the keyword values those buttons insert into metadata MUST remain in English (stock-photo data, not UI chrome).

### Key Entities *(include if feature involves data)*

- **Language**: A selectable interface language (currently English and Russian), identified by a stable code and shown by a human-readable name in the selector.
- **Translation set**: The complete collection of interface texts for one language, organized by text key; one per supported language.
- **Text key**: A stable identifier for a single piece of interface text, shared across all languages; screens reference texts by key, not by literal string.
- **Active language**: The currently selected language, persisted in settings and driving which translation set is displayed; on a first launch (before a choice is saved) it is initialized from the operating-system language when supported, otherwise the default (English).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With Russian selected, 100% of the application's standard screens (menu, files/metadata/viewer panels, settings, every dialog) display in Russian with no leftover English interface text.
- **SC-002**: Switching the language in settings changes the visible interface language with no app restart.
- **SC-003**: The selected language is retained across an app restart in 100% of cases.
- **SC-004**: No user-facing interface text is ever shown blank or as a raw identifier in either language (missing texts fall back to the default language).
- **SC-005**: A new language can be made selectable by adding exactly one translation set, with zero edits to individual screens/components.
- **SC-006**: Dynamic values inside texts (counts, names) are preserved and correctly placed in both languages, and quantity-dependent texts use the correct plural form for the selected language (Russian one/few/many).
- **SC-007**: On a first-ever launch, the interface language matches the operating-system language when it is supported (Russian), and otherwise defaults to English.

## Assumptions

- **Default language**: On a first-ever launch the app detects the operating-system language — if it is Russian the interface starts in Russian, otherwise it starts in English. English is the fallback for any unsupported OS language. After the user makes an explicit choice, that saved choice takes precedence over OS detection.
- **Existing selector**: The settings already contain a language block with English/Russian; this feature makes that selection actually drive the interface text and supplies the Russian texts.
- **Scope of "the UI"**: All of the application's own chrome — menus, panels, settings, dialogs, buttons, tooltips, placeholders, toasts, and validation messages. It excludes user content and domain data (file names, the user's metadata values, stock-photo keyword values), which stay as entered.
- **Pluralization**: Quantity-dependent texts use the selected language's full natural plural forms — for Russian the complete one/few/many forms — so counts always read correctly.
- **Two languages only**: English (existing) and Russian ship; the structure stays open to more, but no other language data is authored here.
- **Out of scope**: translating user data or domain vocabulary (including the keyword values inserted by preset buttons); right-to-left layouts; per-window or per-document language; live community/contributed translations.
