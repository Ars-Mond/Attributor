# Changelog

## [1.0.0] - 2026-06-30

First stable release.

### Added
- **AI auto-attribution (Ollama)** — generate title, description, keywords, categories, and content flags (editorial / mature content / illustration) from a local Ollama vision model; single-photo and batch modes with a cancelable, blocking progress overlay; installed/local/cloud model picker, per-model profiles (prompt + parameters), a configurable response-format heading, and one-click Ollama install / model pull
- **Local SQLite metadata store** — metadata is kept in an app database as you work (new "in app" file status); **Save** writes it into the image file, **Reset** reverts the photo to the file; external file changes are detected via a content fingerprint (size + mtime + xxHash) and resolved with a keep-app-or-file prompt (single and batch apply-to-all)
- **Russian UI localization** — full English / Russian interface, switchable in settings and auto-detected from the OS on first run
- **Appearance settings** — Light / Dark / System theme and adjustable interface font size
- **Configurable keyboard shortcuts** — view and rebind shortcuts, including the file-navigation arrows
- **Attribution flags** — Editorial / Mature content / Illustration, filled by AI attribution and kept in the app store
- **Configurable photo & thumbnail caching** — per-view caching options with optional lazy generation
- **Single-instance** — launching the app a second time focuses the existing window instead of opening a new one
- Stock keyword presets moved into an on-demand popup opened from an icon button next to the keyword input

### Changed
- Metadata now flows through the local store: edits and AI results are saved to the app database first and committed to the image file on **Save** (previously every edit wrote the file directly)
- The footer Cancel control is now **Reset** (reverts the photo to the file) and asks for confirmation
- Frontend logging is routed through the Tauri log plugin (no `console.*`)

## [0.3.0] - 2026-03-28

### Added
- Multi-select files in the file browser for batch metadata editing
- Batch keyword editing: shows which files contain each keyword; promote (●) and remove (×) per-keyword controls
- Batch keywords: drag-to-reorder, copy/paste buttons, comma-separated bulk input
- Alt+click on a file in batch mode previews it in the viewer without changing selection
- Table, Content, and Icons view modes for the file browser with vertical / horizontal layout toggle
- Icons view: full-bleed images filling the cell, hover overlay with filename tooltip
- Word and character count shown below title and description fields; keyword count shown in the keywords header
- About dialog with app version (read from `tauri.conf.json` at runtime), description, and license
- Custom right-click context menu (Copy / Paste) for all text input and textarea fields
- Reusable `MarkdownPopup` component: renders Markdown with configurable size (supports px, %, vh, vw, etc.), position, and fully styled action buttons
- Help dialog loading `Help.md` via `MarkdownPopup`

### Changed
- FilesPanel view preferences (mode, layout, sort) extracted into a persistent Svelte store
- Icon selection highlight switched from box-shadow to outline so it remains visible over images
- Closed dock panels now restore to their original position and size when re-opened

### Fixed
- Icon selection highlight was hidden beneath the photo — replaced with `outline: 2px solid` overlay

## [0.2.1] - 2026-03-23

### Fixed
- Apostrophes and special characters (e.g. `mother's day`) in keywords now preserved correctly
  — XMP parser now accumulates text per `<rdf:li>` element and handles `Event::GeneralRef`
  for proper XML entity resolution (`&apos;` → `'`, `&amp;` → `&`, etc.)
- Duplicate keywords from XMP files are now filtered on read

### Added
- Log file output: logs are saved to `%AppData%\Local\loc.am.attributor\logs\attributor.log` (example, windows)
- Frontend errors and uncaught exceptions forwarded to Tauri log backend


## [0.2.0] - 2026-03-22

### Added
- Light / Dark theme switcher with persistent selection
  — Themes defined via CSS custom properties; new themes can be added in `_themes.scss`
  — Window frame follows selected theme via Tauri `setTheme()`
- Theme switcher dropdown in the viewer toolbar (top-right corner)
- Persist description textarea height across sessions
- Persist collapsible sections state (Stock Keywords, Optional) across sessions
- Fuzzy keyword autocomplete with suggestions dropdown (arrow-key navigation)
- Clipboard copy / paste for keyword list
- Keyword drag & drop reordering

### Changed
- Window is hidden on startup and shown only after full UI initialization (theme, panels, last file) to prevent flash of unstyled content


## [0.1.2] - 2026-03-21

### Added
- Session restore: window size, panel widths, last opened folder and file
  are saved and restored on next launch
- Keyword preset groups (Nature, People, Urban, Concepts, Animals, Seasons)
- Keywords suggestions extracted into a separate `KeywordSuggestions` component

### Changed
- Stock keywords presets section layout reworked


## [0.1.1] - 2026-03-20

### Added
- Minimum window size enforced (1280 × 720)
- Context menu disabled globally
- File navigation with Arrow Up / Down keys
- Arrow key navigation inside keyword suggestions popup works independently of file-panel navigation

### Fixed
- Folder dialog no longer freezes the UI (runs on a separate thread)
- Atomic file write to prevent data loss on save (TOCTOU fix)
- Critical data loss bug in PNG XMP write (`set_png_xmp`)


## [0.1.0] - 2026-03-16

### Added
- Initial release
- Three-panel layout: Metadata editor · Image viewer · File browser
- XMP metadata read / write for JPEG, PNG, WebP
- Fields: filename (rename on save), title, description, keywords, categories, release filename
- Auto-save toggle with 1-second debounce
- Unsaved changes dialog when switching files
- File-gone toast when the open file is deleted or renamed externally
- Resizable panels with persisted widths
- Folder watcher with debounced refresh
