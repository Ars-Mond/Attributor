use serde::{Deserialize, Serialize};

// ── Metadata ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct Metadata {
    pub filename: String,
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: String,
    pub release_filename: String,
}

/// Save image metadata (EXIF / XMP / sidecar).
/// Implementation pending.
#[tauri::command]
fn save_metadata(_metadata: Metadata) {}

// ── File tree ─────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

/// Open a folder picker dialog and return a file-system tree
/// containing only supported image files and subdirectories.
#[tauri::command]
fn open_folder(app: tauri::AppHandle) -> Result<Option<FileNode>, String> {
    use tauri_plugin_dialog::DialogExt;

    let Some(folder) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None); // user cancelled
    };

    let path = std::path::PathBuf::from(folder.to_string());
    let node = scan_dir(&path).map_err(|e| e.to_string())?;
    Ok(Some(node))
}

fn scan_dir(path: &std::path::Path) -> std::io::Result<FileNode> {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .to_string();

    let mut children = Vec::new();

    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .collect();

        // Directories first, then files — both sorted alphabetically
        entries.sort_by(|a, b| {
            let a_dir = a.path().is_dir();
            let b_dir = b.path().is_dir();
            b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
        });

        for entry in entries {
            let child = entry.path();
            if child.is_dir() || is_supported_image(&child) {
                if let Ok(node) = scan_dir(&child) {
                    children.push(node);
                }
            }
        }
    }

    Ok(FileNode {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir: path.is_dir(),
        children,
    })
}

fn is_supported_image(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "tiff" | "tif" | "bmp" | "raw" | "cr2" | "nef" | "arw")
    )
}

// ── App entry ─────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![save_metadata, open_folder])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
