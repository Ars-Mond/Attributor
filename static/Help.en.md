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
| **Categories** | Content categories (`photoshop:Category`). |
| **Release Filename** | Name of the model/property release document, if applicable. |
| **Editorial / Mature content / Illustration** | Attribution flags (checkboxes). |

Categories, Release Filename and the three flags are in the collapsible **Optional** section under the keywords.

The **word / character counter** next to Title and Description helps you stay within agency limits.

**Attribution flags** describe the image for stock agencies and are filled in automatically by AI attribution (you can also toggle them yourself):

- **Editorial** — real events/places, visible logos, brands, or recognizable people (vs. clean commercial content).
- **Mature content** — nudity, sexual themes, or violence.
- **Illustration** — digitally created or heavily edited artwork (vs. a straight photograph).

Release Filename and the three flags have no place inside the image file — they are kept only in Attributor's metadata database (see **Saving & the Metadata Store**).

### Saving & the Metadata Store

Attributor keeps a local **metadata database** as a working copy of every photo's metadata. It protects your edits (and AI attribution) and lets you reopen a photo instantly without re-reading the file.

- **Editing** a field or running **AI attribution** is saved to the database immediately, **without changing the image file**. The status shows 🟣 **in app**.
- **Save Changes** writes the metadata into the actual image file; the status becomes 🟢 **open**.
- **Reset** (the button between *Attribute via …* and *Save Changes*) discards the changes kept only in the app and restores this photo's metadata from the file. A confirmation is shown first.
- **Auto-save** writes changes into the image file automatically after 1 second of inactivity.

The status indicator in the panel header shows:

- 🟢 **open** — the file and the app database agree (written to the file)
- 🟡 **edit** — unsaved edits in the form
- 🟣 **in app** — saved in the app, not yet written to the image file
- 🔵 **batch** — multiple files selected

If a photo's file is changed by another program, Attributor detects it when you open the photo by comparing a fast content fingerprint (size + modification time + content hash).

---

## AI Auto-Attribution

Attributor can fill metadata automatically using a vision model. Choose the provider in **Settings → AI attribution**; each provider keeps its own address, key and active model, so you can switch providers freely.

### Providers

- **Ollama** runs models on this computer. If it is not installed, click **Install Ollama**; then pick a vision model under **Download a model** and click **Download**. Ollama is started automatically when it is needed. Cloud models (tagged `:cloud`) and a remote **Base URL** send the photo to that server.
- **Local servers** — **llama.cpp** (`llama-server`), **LM Studio**, **Jan**, **vLLM** and **Unsloth** (Studio / Desktop) — are used through their OpenAI-compatible API. Set the **Base URL** to the server's address including `/v1` (it starts with the server's default address), make sure the server offers a vision model, and paste an **API key (optional)** only if the server requires one (Unsloth requires a key by default). LM Studio is started on demand through its `lms` CLI. vLLM does not run natively on Windows: point the base URL at WSL2, Docker or another machine. The photo is sent to the server at the base URL — usually this computer.
- **OpenAI API** and **Claude API** — paste an **API key** (**Get a key** opens the provider's key page).
- **ChatGPT** — click **Continue with ChatGPT** to use your ChatGPT Plus or Pro plan through OpenAI's *Sign in with ChatGPT*; the sign-in page opens in the browser. **Use another account** signs in with a different ChatGPT account or workspace. Requests count against the plan's usage limits: **Manage usage** opens the page where you can see and cap them. **Sign out** forgets the sign-in.
- Ollama receives the original photo file. Every other provider receives a JPEG copy downscaled to 1568 px; for the cloud providers (OpenAI API, ChatGPT, Claude API) that copy is uploaded to the provider for analysis.
- API keys are stored in the operating system's credential store, never in the settings file; the ChatGPT sign-in is kept in a file encrypted with a key from that store.
- **Status** shows whether the provider is ready (**Check** tests it again). Pick or type the model in **Active model**; **Refresh** reloads the models the provider reports. **Answer-format heading** is placed before the JSON schema appended to every prompt, and **JSON response format** is that schema (for debugging only).

### Attributing Photos

- Click **Attribute via …** in the Control panel footer — the button names the active provider, e.g. *Attribute via LM Studio*. It analyzes the open photo and fills Title, Description, Keywords, Categories and the attribution flags (Editorial / Mature content / Illustration): Title, Description, Categories and the flags are replaced, and the suggested keywords are added to the existing ones.
- The button is available once a model is selected in **Active model** and the provider is set up: Ollama installed, an API key saved for OpenAI API or Claude API, or signed in to ChatGPT. Local servers are contacted when you click the button.
- A blocking progress bar with a **Cancel** button is shown while the model runs; **Cancel** stops the request.
- In **Batch mode**, the button processes every selected photo in turn. A photo that fails is skipped and the batch goes on; **Cancel** stops the batch, and photos already processed keep their results.
- Results are saved to the app database (status 🟣 **in app**), not to the files. Review them, then click **Save Changes** (**Save N Files** in Batch mode) to write them into the files.

### Model Profiles

**Settings → Model profiles** holds the prompt, thinking mode and run options used for attribution. Each profile has a **Provider** and a **Model id**.

- The most specific profile is used: the active provider's profile for the active model, a **Base (all providers)** profile for the model, the provider's profile with the model id `base` (its fallback for any model without its own profile), then Base's `base` profile. With no matching profile, or an empty prompt, the built-in stock-metadata prompt is used.
- The **Thinking mode** values and the **Run options (JSON)** depend on the profile's provider: the dialog offers that provider's thinking values and describes the run options it accepts. Choosing another provider clears a thinking value it does not offer. Run options of a Base profile are sent as-is to whichever provider runs the request.
- **Keep alive** is an Ollama setting: it is shown for Ollama and Base profiles and used only when Ollama runs the request.
- **Create** adds a profile; **Edit** and **Delete** change or remove it. **Duplicate** opens a copy of a profile as a new one with a numbered name (*Base* → *Base 2*); save it after choosing another provider or model id, since each provider and model id pair can have only one profile. Drag the grip to the left of a profile's name to change the order of the list.
- **Reset to defaults** replaces the list with the default profiles: **Base**, **Base Ollama**, **Qwen2.5 VL 3b** and **Gemma4 Cloud**.

---

## Keywords

### Adding Keywords

- Type a keyword in the input field and press **Enter**.
- Type multiple keywords separated by **`, `** (comma-space) to add them all at once.
- Select from the **autocomplete dropdown** that appears as you type (fuzzy search over 1 000+ stock terms).
- Click the **Stock Keywords** icon button (next to the keyword input) to open a popup of predefined tags grouped by category; click any tag to add it.

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

## Export to CSV

Export the metadata stored in the app to CSV spreadsheets — one file per photo-stock — ready to upload to agencies. The data comes from the app's metadata store, not re-read from the photo files, so review and save your photos first.

### Configure stock presets

Attributor ships ready-made presets for **Shutterstock**, **iStock**, **Adobe Stock**, and **Envato**. Open **File → Settings → CSV Export** to use them as-is, edit them, or create your own. Each preset has:

- **Name** — the label shown in settings.
- **Stock identifier** — used as the CSV file name (`<identifier>.csv`); must be unique and valid as a file name.
- **Column delimiter** — comma, semicolon, or tab (default comma).
- **Fields** — the ordered list of columns. For each field set the **CSV column** header and the **value** it draws from: none (a constant **default value**), file name, title, description, keywords, category, release filename, or the editorial / mature-content / illustration flags (with a **yes/no** vs **true/false** format). Reorder fields with the up/down arrows. A preset must have at least one field.

### Run the export

Choose **File → Export to CSV** and pick a **destination folder**. The app writes one `<identifier>.csv` per preset there (overwriting existing files of the same name). The scope is your current selection, or — if nothing is selected — every photo in the current folder, excluding sub-folders. Photos with no saved metadata in the app are skipped; the result message reports how many files were written, how many photos were exported, and how many were skipped.

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

## Settings

Open **File → Settings** (or press `Ctrl+,`).

| Section | What you can change |
|---------|---------------------|
| **General** | Interface language; read nested folders. |
| **Editor** | Auto-save on/off and its delay. |
| **Appearance** | Theme — **System / Light / Dark** — and the interface font size. |
| **Caching** | Photo and thumbnail caching options. |
| **AI attribution** | AI provider and its connection: status, base URL, API key or ChatGPT sign-in, active model (and, for Ollama, install and model download), answer-format heading and JSON response format. |
| **Model profiles** | Attribution profiles per provider and model (prompt, thinking mode, run options and, for Ollama, keep alive); **Base (all providers)** profiles apply to every provider. Create, duplicate, edit, delete, and drag to reorder. |
| **CSV Export** | Per-stock CSV presets: name, identifier, column delimiter, and the ordered field list. |
| **Shortcuts** | View and rebind keyboard shortcuts, including the file-navigation arrows. |

---

## Tips

- **Rename on save** — changing the Filename field renames the actual file when you save. The panel and file tree update automatically.
- **File watcher** — if files are moved or deleted externally while Attributor is open, the tree refreshes automatically and a notification appears if the currently open file is gone.
- **Auto-save + batch** — Auto-save is disabled in Batch mode to prevent accidental overwrites. Use the **Save N Files** button explicitly.
