use serde::Deserialize;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![save_metadata])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
