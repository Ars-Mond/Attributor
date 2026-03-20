# ![Attributor](src-tauri/icons/logo.png)

# Attributor

Desktop application for editing photo metadata (EXIF/XMP) before submitting to stock photo agencies. Fill in title, description, keywords, and categories — then save directly into the image file (JPEG, PNG, WebP).

## Releases

Download the latest installer from the [Releases](../../releases) page.

## Usage

1. Click **Open Folder** and select a directory with your photos.
2. Select an image in the file tree on the left.
3. Edit the metadata fields: title, description, keywords, categories.
4. Optionally rename the file via the filename field.
5. Click **Save** — metadata is written directly into the image file.

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
