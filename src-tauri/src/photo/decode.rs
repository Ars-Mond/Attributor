//! In-process image decode to RGBA pixels. Backend-only capability (FR-009):
//! the result is never transferred over IPC and is reserved for future features.

use std::path::Path;

use log::{debug, error, info};

/// Decodes the image at `path` into an in-memory RGBA buffer via the `image` crate.
pub(crate) fn decode_image(path: &Path) -> Result<image::RgbaImage, String> {
    info!("decode_image: {}", path.display());
    let img = image::open(path).map_err(|e| {
        let msg = format!("decode failed for {}: {e}", path.display());
        error!("{msg}");
        msg
    })?;
    let rgba = img.to_rgba8();
    debug!("decode_image: {}x{}", rgba.width(), rgba.height());
    Ok(rgba)
}
