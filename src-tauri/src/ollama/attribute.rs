//! Attribution: run a vision inference and map the strict JSON onto the editor's metadata.
//! Single mode returns the parsed result (frontend applies it); batch mode merges into each file's
//! existing metadata and saves it, sequentially (Ollama serializes inference), streaming progress.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::events::{BatchProgress, ItemStatus};
use crate::types::SaveRequest;

use super::client;
use super::types::{AttributionConfig, AttributionResult};

/// Parse and validate the model's strict-JSON `response` string into the applied fields.
/// The editorial/mature_content/illustration flags are present in the schema but ignored here.
fn parse_result(raw: &str) -> Result<AttributionResult, String> {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).map_err(|e| format!("invalid JSON from model: {e}"))?;
    let str_field = |key: &str| -> Result<String, String> {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("response missing string field '{key}'"))
    };
    let arr_field = |key: &str| -> Result<Vec<String>, String> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .ok_or_else(|| format!("response missing array field '{key}'"))
    };
    Ok(AttributionResult {
        title: str_field("title")?,
        description: str_field("description")?,
        keywords: arr_field("keywords")?,
        categories: arr_field("categories")?,
    })
}

/// Single-image attribution → parsed result (the frontend applies it to the form).
pub async fn attribute_one(path: &str, cfg: &AttributionConfig) -> Result<AttributionResult, String> {
    let image = client::image_to_base64(path)?;
    let raw = client::generate(cfg, image).await?;
    parse_result(&raw)
}

/// Attribute one file and persist it: read existing metadata, overwrite text/categories, append+dedupe
/// keywords, then save via the shared per-file writer. Returns the final path.
async fn attribute_and_save(path: &str, cfg: &AttributionConfig) -> Result<String, String> {
    let result = attribute_one(path, cfg).await?;
    let path = path.to_string();
    tokio::task::spawn_blocking(move || {
        let existing = crate::photo::read_metadata(path.clone()).unwrap_or_default();
        let mut keywords = existing.keywords;
        for kw in result.keywords {
            let k = kw.trim().to_lowercase();
            if !k.is_empty() && !keywords.iter().any(|e| e.eq_ignore_ascii_case(&k)) {
                keywords.push(k);
            }
        }
        let stem = Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        crate::batch::save_one(SaveRequest {
            filepath: path,
            filename: stem,
            title: result.title,
            description: result.description,
            keywords,
            categories: result.categories.join(", "),
            release_filename: String::new(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sequentially attribute and save every path, streaming one `BatchProgress` per file. A failed item is
/// recorded and the loop continues; cancellation stops before the next item (already-saved files remain).
pub async fn attribute_batch(
    paths: &[String],
    cfg: &AttributionConfig,
    cancel: &Arc<AtomicBool>,
    progress: impl Fn(BatchProgress),
) -> Vec<ItemStatus> {
    let mut out = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let status = if cancel.load(Ordering::Relaxed) {
            ItemStatus::Cancelled
        } else {
            match attribute_and_save(path, cfg).await {
                Ok(p) => ItemStatus::Ok { path: p },
                Err(error) => {
                    log::warn!("attribute batch item {index} failed: {error}");
                    ItemStatus::Failed { error }
                }
            }
        };
        progress(BatchProgress { index, status: status.clone() });
        out.push(status);
    }
    out
}
