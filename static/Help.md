# Attributor — User Guide

Attributor is a desktop application for editing **XMP/EXIF metadata** of stock photos before submitting them to agencies such as Shutterstock, Adobe Stock, Getty Images, and others. It lets you embed title, description, keywords, and categories directly into image files without re-encoding them, preserving full image quality.

**Supported formats:** JPEG · PNG · WebP

---

## Getting Started

### Opening a Folder

Go to **File → Open directory…** or press the folder icon in the Hierarchy panel. Attributor scans the selected folder and displays all supported image files in the file tree.

The last opened folder is remembered and restored automatically on the next launch.

### Selecting a File

Click any file in the Hierarchy panel to open it. The image appears in the **View** panel and its existing metadata loads into the **Control** panel.

If a file has unsaved changes, Attributor will prompt you to **Save**, **Discard**, or **Cancel** before switching to another file.

---

## Editing Metadata

All metadata fields are in the **Control** panel.

| Field | Description |
|-------|-------------|
| **Filename** | Stem of the file name (without extension). Renaming happens on save. |
| **Title** | Short descriptive title of the image (stored as `dc:title`). |
| **Description** | Detailed description for search engines and buyers (`dc:description`). |
| **Keywords** | Tags that describe the image content (`dc:subject`). |
| **Categories** | Optional content categories (`photoshop:Category`). |
| **Release Filename** | Name of the model/property release document, if applicable. |

The **word / character counter** next to Title and Description helps you stay within agency limits.

### Saving

Click **Save Changes** or enable **Auto-save** to write changes to the image file automatically after 1 second of inactivity. The status indicator in the panel header shows:

- 🟢 **open** — file loaded, no changes
- 🟡 **edit** — unsaved changes
- 🔵 **batch** — multiple files selected

---

## Keywords

### Adding Keywords

- Type a keyword in the input field and press **Enter**.
- Type multiple keywords separated by **`, `** (comma-space) to add them all at once.
- Select from the **autocomplete dropdown** that appears as you type (fuzzy search over 1 000+ stock terms).
- Click any **Stock Keywords** preset to add predefined tags by category.

### Managing Keywords

- **Drag** chips to reorder them.
- Click **×** on a chip to remove it.
- **Copy** — copies all keywords to the clipboard as a comma-separated list.
- **Paste** — parses clipboard text and adds all found keywords.

---

## Batch Mode

Select **two or more files** in the Hierarchy panel (using `Shift`+click or `Ctrl`+click) to enter Batch mode. The Control panel switches to a multi-file editing view.

### How Batch Editing Works

- **Title / Description / Categories** — each field has a checkbox. Check it to apply the value to all selected files on save.
- If the files already have different values for a field, the placeholder shows **(mixed values)** and the checkbox is unchecked by default.
- **Keywords** — the merged union of all files' keywords is shown:
  - **Solid chip** — keyword present in **all** selected files.
  - **Dashed grey chip** — keyword present in **some** files only. A filled circle ● appears before it — click the circle to promote the keyword to all files.
- Click **×** on any chip to remove that keyword from all files on save.
- Drag chips to reorder them.
- **Copy** — copies only the common (solid) keywords.
- **Paste** — adds keywords to all selected files.

### Previewing in Batch Mode

The View panel shows the **last selected file**. To preview a specific file from your selection without changing it, hold **Alt** and click the file in the Hierarchy panel.

### Saving Batch Changes

Click **Save N Files** to apply all changes. Each file is updated individually: only checked fields are overwritten; unchecked fields retain their original values.

---

## Docking System

Attributor has three panels that can be freely rearranged:

| Panel | Default location | Purpose |
|-------|-----------------|---------|
| **Control** | Left | Metadata editing form |
| **View** | Centre | Image preview |
| **Hierarchy** | Right | File tree / browser |

### Resizing Panels

Drag the divider between any two panels to resize them. The layout is saved automatically.

### Closing and Restoring Panels

Click the **×** on a panel's title bar to close it. Restore it from **Windows → Show Control / Show Hierarchy** in the menu bar. Restored panels return to their last position.

### Rearranging Panels

Drag a panel's title bar and drop it onto a **drop zone** (highlighted edges) of another panel to split the space and reposition it anywhere in the layout.

---

## Hierarchy Panel View Modes

Use the toolbar in the Hierarchy panel to switch how files are displayed.

### View Modes

| Icon | Mode | Description |
|------|------|-------------|
| ☰ | **Table** | Compact list with file names and folder tree. |
| ▤ | **Content** | Thumbnail + file name row. |
| ⊞ | **Icons** | Large thumbnail grid. |

### Layout Direction (Content and Icons modes)

| Icon | Direction | Behaviour |
|------|-----------|-----------|
| — | **Vertical** | Items stack top-to-bottom; each image fills the full panel width. |
| ǀ | **Horizontal** | Items line up left-to-right in a single scrollable row; each image fills the full panel height. Scroll with the mouse wheel. |

---

## Keyboard Shortcuts

### File Navigation

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move selection up / down in the file list |
| `Shift` + `↑` / `↓` | Extend range selection |
| `Shift` + click | Select a range of files |
| `Ctrl` + click | Toggle a single file in/out of selection |
| `Alt` + click | Preview the clicked file in View panel (batch mode only, selection unchanged) |

### Metadata Editing

| Key / Input | Action |
|-------------|--------|
| `Enter` | Add the typed keyword |
| `, ` (comma-space) | Add the preceding text as a keyword and continue typing |
| `Esc` | Close autocomplete suggestions / close dialogs |
| `↑` / `↓` in suggestions | Navigate the autocomplete list |

### Context Menu (text fields)

Right-click any text input or textarea to open the context menu:

| Item | Shortcut | Action |
|------|----------|--------|
| **Copy** | `Ctrl+C` | Copy the selected text to clipboard |
| **Paste** | `Ctrl+V` | Paste clipboard text at the cursor position |

---

## Tips

- **Rename on save** — changing the Filename field renames the actual file when you save. The panel and file tree update automatically.
- **File watcher** — if files are moved or deleted externally while Attributor is open, the tree refreshes automatically and a notification appears if the currently open file is gone.
- **Auto-save + batch** — Auto-save is disabled in Batch mode to prevent accidental overwrites. Use the **Save N Files** button explicitly.
