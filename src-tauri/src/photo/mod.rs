//! Unified photo abstraction: streaming metadata read/merge, multi-block write,
//! and in-process image decode. Backs the `read_metadata` / `save_metadata` commands.

mod decode;
mod read;
mod thumbnail;
mod write;
mod xmp;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use little_exif::filetype::{get_file_type, FileExtension};
use serde::Serialize;

pub use thumbnail::{ensure_thumbnails, Thumbnails};

/// The four logical metadata fields the application edits and stores.
#[derive(Serialize, Default, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub category: String,
}

/// A single photo file on disk (JPEG / PNG / WebP). Entry point for every operation.
pub struct Photo {
    path: PathBuf,
    file_type: FileExtension,
}

impl Photo {
    /// Opens a photo, detecting and validating its format. No file body is loaded.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let file_type = get_file_type(&path).map_err(|e| e.to_string())?;
        match file_type {
            FileExtension::JPEG | FileExtension::PNG { .. } | FileExtension::WEBP => {}
            _ => return Err(format!("Unsupported format: {}", path.display())),
        }
        Ok(Photo { path, file_type })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn is_jpeg(&self) -> bool {
        matches!(self.file_type, FileExtension::JPEG)
    }

    /// Reads and merges metadata from every block with EXIF > IPTC > XMP precedence.
    pub fn read_metadata(&self) -> Result<Metadata, String> {
        read::read_metadata(self)
    }

    /// Writes metadata into every supported block (duplicated), removing cleared fields.
    pub fn save_metadata(&self, meta: &Metadata) -> Result<(), String> {
        write::save_metadata(self, meta)
    }

    /// Decodes the image to in-memory RGBA pixels. Backend-only; never crosses IPC.
    pub fn decode_image(&self) -> Result<image::RgbaImage, String> {
        decode::decode_image(&self.path)
    }
}

// ── Free-function wrappers (command layer + integration tests) ──────────────

pub fn read_metadata(filepath: String) -> Result<Metadata, String> {
    Photo::open(&filepath)?.read_metadata()
}

pub fn write_metadata(filepath: String, metadata: Metadata) -> Result<(), String> {
    Photo::open(&filepath)?.save_metadata(&metadata)
}

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Merges several keyword lists into one trimmed, de-duplicated, order-preserving list.
pub(crate) fn unique_keywords<I>(sources: I) -> Vec<String>
where
    I: IntoIterator<Item = Vec<String>>,
{
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for source in sources {
        for kw in source {
            let kw = kw.trim().to_string();
            if !kw.is_empty() && seen.insert(kw.clone()) {
                result.push(kw);
            }
        }
    }
    result
}

pub(crate) fn split_semicolons(s: String) -> Vec<String> {
    s.split(';')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

pub(crate) fn non_empty(s: String) -> Option<String> {
    let trimmed = s.trim_matches(|c: char| c.is_whitespace() || c == '\0').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
