//! Thumbnail generation and cache. Produces a low (360px longest side) and a high
//! (1920px longest side) JPG for a photo, stored in a `_thumbnail` folder beside it.
//! Valid existing thumbnails are reused; writes are atomic (temp file then rename) so a
//! crash or concurrent write never leaves a half-written thumbnail.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder};
use log::{debug, error};
use serde::Serialize;

pub const LOW_MAX: u32 = 360;
pub const HIGH_MAX: u32 = 1920;
pub const JPEG_QUALITY: u8 = 75;

const THUMB_DIR: &str = "_thumbnail";

/// Absolute paths to a photo's two cached thumbnails.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnails {
    pub low: String,
    pub high: String,
}

#[derive(Clone, Copy)]
enum Variant {
    Low,
    High,
}

impl Variant {
    fn max(self) -> u32 {
        match self {
            Variant::Low => LOW_MAX,
            Variant::High => HIGH_MAX,
        }
    }
    fn suffix(self) -> &'static str {
        match self {
            Variant::Low => "low",
            Variant::High => "high",
        }
    }
}

/// The `_thumbnail` folder beside the source photo.
fn thumb_dir(source: &Path) -> PathBuf {
    source.parent().unwrap_or_else(|| Path::new(".")).join(THUMB_DIR)
}

/// Deterministic thumbnail path: `<dir>/_thumbnail/<file_name>.<variant>.jpg`.
/// The full source file name (incl. extension) keeps same-stem files (`a.jpg`/`a.png`) distinct.
fn thumb_path(source: &Path, variant: Variant) -> PathBuf {
    let name = source.file_name().and_then(|n| n.to_str()).unwrap_or("thumb");
    thumb_dir(source).join(format!("{name}.{}.jpg", variant.suffix()))
}

/// A thumbnail is valid if it exists and begins with the JPEG SOI marker (`FF D8`).
/// Cheap (2-byte read) — suitable for checking every photo during a folder scan, and
/// enough to reject empty/garbage files so they get regenerated (FR-011).
fn is_valid(path: &Path) -> bool {
    match fs::File::open(path) {
        Ok(mut f) => {
            let mut magic = [0u8; 2];
            f.read_exact(&mut magic).is_ok() && magic == [0xFF, 0xD8]
        }
        Err(_) => false,
    }
}

/// Reuse valid existing thumbnails or generate the missing ones; return both paths.
pub fn ensure_thumbnails(source: &Path) -> Result<Thumbnails, String> {
    let low = thumb_path(source, Variant::Low);
    let high = thumb_path(source, Variant::High);

    let need_low = !is_valid(&low);
    let need_high = !is_valid(&high);

    if need_low || need_high {
        let dir = thumb_dir(source);
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

        let img = image::open(source).map_err(|e| {
            let msg = format!("decode {}: {e}", source.display());
            error!("{msg}");
            msg
        })?;

        if need_low {
            generate(&img, &low, Variant::Low.max())?;
        }
        if need_high {
            generate(&img, &high, Variant::High.max())?;
        }
        debug!("ensure_thumbnails: generated for {}", source.display());
    }

    Ok(Thumbnails {
        low: low.to_string_lossy().into_owned(),
        high: high.to_string_lossy().into_owned(),
    })
}

/// Longest-side resize (no upscale) → rgb8 → JPEG at `JPEG_QUALITY`, written atomically
/// (temp file in the same folder, then rename into place).
fn generate(src: &DynamicImage, dst: &Path, max: u32) -> Result<(), String> {
    let longest = src.width().max(src.height());
    let resized = if longest > max {
        src.resize(max, max, image::imageops::FilterType::Lanczos3)
    } else {
        src.clone()
    };
    let rgb = resized.to_rgb8();

    let parent = dst.parent().ok_or_else(|| format!("no parent dir for {}", dst.display()))?;
    let fname = dst.file_name().and_then(|n| n.to_str()).unwrap_or("thumb.jpg");
    let tmp = parent.join(format!(".{fname}.tmp"));

    let write_result = (|| -> Result<(), String> {
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        JpegEncoder::new_with_quality(&mut file, JPEG_QUALITY)
            .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            .map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        error!("thumbnail encode failed for {}: {e}", dst.display());
        return Err(e);
    }

    fs::rename(&tmp, dst).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        let msg = format!("rename thumbnail {}: {e}", dst.display());
        error!("{msg}");
        msg
    })
}
