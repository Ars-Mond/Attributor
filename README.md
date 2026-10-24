# ![Attributor](banner.png)

# Attributor

[![Русский](https://img.shields.io/badge/README-Русский-blue)](README.ru.md)

![Version](https://img.shields.io/badge/version-1.4.0-blue)
![License](https://img.shields.io/badge/license-AGPL--3.0-green)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)
![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange)
[![Ko-fi](https://img.shields.io/badge/Ko--fi-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/ars_mond)

Desktop application for editing photo metadata (EXIF/XMP) before submitting to stock photo agencies. Fill in title, description, keywords, and categories by hand or generate them with an AI vision model running on your computer or in the cloud, then write them into the image file without re-encoding. Edits are kept in a local database as you work and committed to the file when you save.

**Supported formats:** JPEG · PNG · WebP

## Features

- **Metadata editing** — title, description, keywords, categories, and release filename, written into the image as XMP/EXIF/IPTC without re-encoding
- **AI auto-attribution** — generate title, description, keywords, categories, and content flags (editorial / mature / illustration) with a vision model — locally via Ollama, llama.cpp, LM Studio, Jan, vLLM or Unsloth, or in the cloud via the OpenAI API, your ChatGPT plan (Sign in with ChatGPT) or the Claude API — for a single photo or a whole batch
- **AI privacy** — local providers send the photo only to the server at their base URL, usually on your computer (Ollama `:cloud` models send it to Ollama's cloud); cloud providers (OpenAI API, ChatGPT, Claude API) receive a copy of the photo; Ollama gets the original file, every other provider a JPEG downscaled to 1568 px; API keys are kept in the OS credential store, never in the settings file
- **Model profiles** — prompt, thinking mode, and run options per provider and model, with a `base` fallback profile; profiles of **Base (all providers)** apply to every provider, and the most specific profile wins; profiles can be duplicated and reordered by drag-and-drop
- **Local metadata store** — edits and AI results are kept in an app database as you work (status “in app”) and written into the file on **Save**; **Reset** reverts to the file, and external file changes are detected with a keep-app-or-file prompt
- **MCP server for AI assistants** — while Attributor is open, Claude Desktop, Claude Code and ChatGPT / Codex can open folders, read and change metadata (single photos or batches), save it, run AI attribution and adjust settings; one-click client setup in **Settings → MCP**, loopback-only with an access token, AI settings only with your permission
- **CSV export** — export the stored metadata to CSV spreadsheets, one file per photo-stock, into a chosen folder; configurable per-stock presets (columns, value types, per-preset delimiter) with ready-made presets for Shutterstock, iStock, Adobe Stock, and Envato
- **Multilingual UI** — English and Russian, switchable in settings and auto-detected from the OS on first run
- **Batch mode** — select multiple files and edit shared metadata at once; keywords show which files contain them, with per-keyword promote/remove controls
- **Keyword autocomplete** — fuzzy search over 1 000+ stock photography terms with drag-and-drop reordering, clipboard copy/paste, and comma-separated bulk input
- **Auto-save** — optional 1-second debounce auto-save for single-file editing
- **File watcher** — detects external changes (renames, deletions) and refreshes the file tree automatically
- **Flexible dock layout** — three resizable, closable, and reorderable panels (Control · View · Hierarchy)
- **Multiple view modes** — table, content, and icons view with vertical/horizontal layout for the file browser
- **Rename on save** — changing the filename field renames the actual file on disk

## Usage

See the full **[User Guide](static/Help.en.md)** for detailed instructions. A Russian version is available from the app's Help menu.

AI attribution is set up in **Settings → AI attribution** — choose the provider, set its base URL, paste an API key or click **Continue with ChatGPT**, and pick the active model — and tuned in **Settings → Model profiles**. The OpenAI API and Claude API need an API key; ChatGPT uses your ChatGPT Plus or Pro plan, and requests count against its usage limits. The attribute button in the Control panel names the active provider (**Attribute via …**).

To let an AI assistant work with Attributor, open **Settings → MCP**, turn on the server, click **Install** next to Claude Desktop, Claude Code or ChatGPT / Codex, and restart that client. Attributor must stay open while the assistant works. Assistants can change AI settings only after you allow it on that page.

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
- vLLM does not run natively on Windows: run it in WSL2, Docker, or on another machine and enter that server's address as vLLM's base URL
- ChatGPT in the browser cannot connect to the MCP server on your computer; in the ChatGPT desktop app the Attributor tools are available in the Codex and Work tabs

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

## Support

If Attributor is useful to you, consider supporting its development on Ko-fi:

[![Ko-fi](https://img.shields.io/badge/Ko--fi-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/ars_mond)

## Copyright

All images, logos, photos, screenshots, illustrations, and other graphic assets included in this repository — except application icons and UI design elements — are copyright © their respective owners. All rights reserved. Reproduction or redistribution without explicit written permission is prohibited.

## License

This application (excluding the copyrighted graphic assets noted above) is distributed under the [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0).
