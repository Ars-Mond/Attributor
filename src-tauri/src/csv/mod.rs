//! CSV export (feature 009). Reads photo metadata from the intermediate store (read-only, never the
//! files) and writes one CSV file per configured stock preset into a chosen folder. All heavy work
//! runs in a single `spawn_blocking` behind one IPC call (Constitution VIII). The writer is a small
//! hand-rolled RFC 4180 encoder — no new dependency (research R1).

mod cell;
mod writer;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::store::{DbState, StoredMetadata};

/// What an app value a column draws from (FR-023). Fixed set; camelCase across the IPC boundary.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum AppValueType {
    None,
    FileName,
    Title,
    Description,
    Keywords,
    Category,
    Editorial,
    MatureContent,
    Illustration,
}

/// Per-preset column delimiter (FR-034). UTF-8 + RFC 4180 quoting stay global.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub enum Delimiter {
    #[default]
    Comma,
    Semicolon,
    Tab,
}

impl Delimiter {
    fn byte(self) -> u8 {
        match self {
            Delimiter::Comma => b',',
            Delimiter::Semicolon => b';',
            Delimiter::Tab => b'\t',
        }
    }
}

/// How a bool renders in a cell (FR-011).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub enum BoolFormat {
    #[default]
    YesNo,
    TrueFalse,
}

impl BoolFormat {
    fn render(self, v: bool) -> &'static str {
        match (self, v) {
            (BoolFormat::YesNo, true) => "yes",
            (BoolFormat::YesNo, false) => "no",
            (BoolFormat::TrueFalse, true) => "true",
            (BoolFormat::TrueFalse, false) => "false",
        }
    }
}

/// One column in a preset (data-model.md). `default_value` is used only for `None`; `bool_format`
/// only for bool types — both always present for a uniform serialized shape.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CsvField {
    pub csv_column: String,
    pub value_type: AppValueType,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub bool_format: BoolFormat,
}

/// One stock's export configuration (data-model.md).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CsvPreset {
    pub id: String,
    pub name: String,
    pub identifier: String,
    #[serde(default)]
    pub delimiter: Delimiter,
    pub fields: Vec<CsvField>,
}

/// Outcome of one export run (FR-031).
#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub files_written: usize,
    pub photos_exported: usize,
    pub skipped: usize,
}

/// Defensive file-name safety check for a stock identifier (research R9). The dialog enforces this
/// already (FR-020); this guard prevents a malformed identifier from ever reaching the filesystem.
/// Rejects illegal characters, control chars, leading/trailing dots-or-spaces, and reserved Windows
/// device names — the strictest common subset so a preset is portable across every supported OS.
fn is_safe_identifier(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    if id
        .chars()
        .any(|c| matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 0x20)
    {
        return false;
    }
    if id != id.trim_matches(|c: char| c == '.' || c == ' ') {
        return false;
    }
    let stem = id.split('.').next().unwrap_or(id).to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return false;
    }
    if let Some(rest) = stem.strip_prefix("COM").or_else(|| stem.strip_prefix("LPT")) {
        if rest.len() == 1 && matches!(rest.as_bytes()[0], b'1'..=b'9') {
            return false;
        }
    }
    true
}

/// Build the CSV files. Fetch each path's record once (skip + count those with none), then for each
/// preset write `<identifier>.csv` with its delimiter. Pure (no IPC); runs inside `spawn_blocking`.
fn export(db: &DbState, dir: &str, paths: &[String], presets: &[CsvPreset]) -> Result<ExportSummary, String> {
    let dir = Path::new(dir);

    // Read every in-scope photo's record once; reuse across all presets. Missing → skipped (FR-035).
    let mut records: Vec<(&str, StoredMetadata)> = Vec::new();
    let mut skipped = 0usize;
    for p in paths {
        match db.fetch(p) {
            Some(meta) => records.push((p.as_str(), meta)),
            None => skipped += 1,
        }
    }

    let mut files_written = 0usize;
    for preset in presets {
        // Empty presets are prevented at config (FR-036); skip defensively if one slips through.
        if preset.fields.is_empty() {
            continue;
        }
        if !is_safe_identifier(&preset.identifier) {
            log::warn!("export_csv: skipping preset with unsafe identifier: {:?}", preset.identifier);
            continue;
        }

        let delim = preset.delimiter.byte();
        let mut out = String::new();

        let header: Vec<&str> = preset.fields.iter().map(|f| f.csv_column.as_str()).collect();
        writer::write_row(&mut out, &header, delim);

        for (path, meta) in &records {
            let row: Vec<String> = preset.fields.iter().map(|f| cell::cell(f, path, meta)).collect();
            writer::write_row(&mut out, &row, delim);
        }

        let file = dir.join(format!("{}.csv", preset.identifier));
        std::fs::write(&file, out.as_bytes()).map_err(|e| {
            log::error!("export_csv: write failed for {}: {e}", file.display());
            format!("failed to write {}: {e}", file.display())
        })?;
        files_written += 1;
    }

    Ok(ExportSummary { files_written, photos_exported: records.len(), skipped })
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Open a native folder picker; return the chosen destination directory, or `None` on cancel.
#[tauri::command]
pub async fn pick_export_dir(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |result| {
        let _ = tx.send(result);
    });
    let Some(folder) = rx.await.map_err(|e| e.to_string())? else {
        log::info!("pick_export_dir: cancelled");
        return Ok(None);
    };
    let path = folder.into_path().map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Write one CSV per preset into `dir`, using store records for `paths`. One IPC round-trip; all work
/// in a single blocking task after `db.share()` (Constitution VIII). When the store is unavailable,
/// every photo is reported as skipped (graceful, not an error).
#[tauri::command]
pub async fn export_csv(
    dir: String,
    paths: Vec<String>,
    presets: Vec<CsvPreset>,
    db: tauri::State<'_, DbState>,
) -> Result<ExportSummary, String> {
    let db = db.share();
    tokio::task::spawn_blocking(move || export(&db, &dir, &paths, &presets))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_identifier_accepts_and_rejects() {
        assert!(is_safe_identifier("shutterstock"));
        assert!(is_safe_identifier("adobe_stock-2"));
        assert!(is_safe_identifier("com10")); // not a reserved device name

        assert!(!is_safe_identifier(""));
        assert!(!is_safe_identifier("bad/name"));
        assert!(!is_safe_identifier("bad:name"));
        assert!(!is_safe_identifier("bad*name"));
        assert!(!is_safe_identifier("name ")); // trailing space
        assert!(!is_safe_identifier(".name")); // leading dot
        assert!(!is_safe_identifier("CON"));
        assert!(!is_safe_identifier("con"));
        assert!(!is_safe_identifier("com1"));
        assert!(!is_safe_identifier("LPT9"));
    }
}
