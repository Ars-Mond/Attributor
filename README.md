# ![Attributor](banner.png)

# Attributor

![Version](https://img.shields.io/badge/version-0.3.0-blue)
![License](https://img.shields.io/badge/license-AGPL--3.0-green)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)
![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange)

Desktop application for editing photo metadata (EXIF/XMP) before submitting to stock photo agencies. Fill in title, description, keywords, and categories — then save directly into the image file without re-encoding.

**Supported formats:** JPEG · PNG · WebP

## Features

- **Metadata editing** — title, description, keywords, categories, and release filename written directly into the image as XMP
- **Batch mode** — select multiple files and edit shared metadata at once; keywords show which files contain them, with per-keyword promote/remove controls
- **Keyword autocomplete** — fuzzy search over 1 000+ stock photography terms with drag-and-drop reordering, clipboard copy/paste, and comma-separated bulk input
- **Auto-save** — optional 1-second debounce auto-save for single-file editing
- **File watcher** — detects external changes (renames, deletions) and refreshes the file tree automatically
- **Flexible dock layout** — three resizable, closable, and reorderable panels (Control · View · Hierarchy)
- **Multiple view modes** — table, content, and icons view with vertical/horizontal layout for the file browser
- **Rename on save** — changing the filename field renames the actual file on disk

## Usage

See the full **[User Guide](static/Help.md)** for detailed instructions.

## Platforms

| OS              | Builds                                 |
|-----------------|----------------------------------------|
| Windows 10 / 11 | Installer (`.msi`) · Portable (`.exe`) |
| macOS           | `.dmg`                                 |
| Linux           | `AppImage` · `.deb`                    |

Download the latest release from the [Releases](../../releases) page.
See [CHANGELOG.md](CHANGELOG.md) for version history.

## Known Limitations

- Icons view mode displays only the root-level files of the opened folder (no subdirectory traversal)
- TIFF and RAW formats are not supported
- Auto-save is disabled in batch mode to prevent accidental overwrites

## For Developers

**Stack:** Rust · Tauri 2 · SvelteKit · TypeScript · Vite · SCSS

**Prerequisites:** [Rust](https://rustup.rs) · [Node.js](https://nodejs.org) · [pnpm](https://pnpm.io) · [Tauri prerequisites](https://tauri.app/start/prerequisites/)

```sh
# Install frontend dependencies
pnpm install

# Run in development mode
cargo tauri dev

# Build release
cargo tauri build

# Regenerate app icon from source image
cargo tauri icon src-tauri/icons/logo.png
```

## Copyright

All images, logos, photos, screenshots, illustrations, and other graphic assets included in this repository — except application icons and UI design elements — are copyright © their respective owners. All rights reserved. Reproduction or redistribution without explicit written permission is prohibited.

## License

This application (excluding the copyrighted graphic assets noted above) is distributed under the [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0).
