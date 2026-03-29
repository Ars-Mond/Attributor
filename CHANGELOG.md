# Changelog

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
