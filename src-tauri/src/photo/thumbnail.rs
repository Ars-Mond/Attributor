//! Thumbnail generation and cache. Produces a low (360px longest side) and a high
//! (1920px longest side) JPG for a photo, stored in a `_thumbnail` folder beside it.
//! Valid existing thumbnails are reused; writes are atomic (temp file then rename) so a
//! crash or concurrent write never leaves a half-written thumbnail.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

/// True if the photo cache looks present: `folder` has a `_thumbnail` subfolder whenever it
/// directly contains photos, and — when `recursive` — so does every nested subfolder. Lets the UI
/// detect a cache deleted on disk and regenerate. An unreadable directory is treated as present so
/// transient errors don't trigger spurious regeneration.
pub fn thumbnail_dir_exists(folder: &Path, recursive: bool) -> bool {
    let entries = match fs::read_dir(folder) {
        Ok(e) => e,
        Err(_) => return true,
    };

    let mut has_image = false;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().and_then(|n| n.to_str()) != Some(THUMB_DIR) {
                subdirs.push(p);
            }
        } else if is_supported_image(&p) {
            has_image = true;
        }
    }

    if has_image && !folder.join(THUMB_DIR).is_dir() {
        return false;
    }
    if recursive {
        for sub in subdirs {
            if !thumbnail_dir_exists(&sub, true) {
                return false;
            }
        }
    }
    true
}

/// Whether `path` has a supported image extension (jpg/jpeg/png/webp), case-insensitive.
fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp")
    )
}

/// Deterministic thumbnail paths for a source photo — computed only, no file I/O.
/// Lets callers (e.g. the folder scanner) record paths before the files exist.
pub fn thumbnail_paths(source: &Path) -> Thumbnails {
    Thumbnails {
        low: thumb_path(source, Variant::Low).to_string_lossy().into_owned(),
        high: thumb_path(source, Variant::High).to_string_lossy().into_owned(),
    }
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

/// Ensure only the requested sizes: reuse each valid cached size, decode the source at most
/// once (iff a requested size is missing), and generate only the requested missing sizes.
/// Returns both deterministic paths regardless of which were generated. Requesting neither
/// size touches no files.
pub fn ensure(source: &Path, low: bool, high: bool) -> Result<Thumbnails, String> {
    let low_path = thumb_path(source, Variant::Low);
    let high_path = thumb_path(source, Variant::High);

    let need_low = low && !is_valid(&low_path);
    let need_high = high && !is_valid(&high_path);

    if need_low || need_high {
        let dir = thumb_dir(source);
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

        let img = image::open(source).map_err(|e| {
            let msg = format!("decode {}: {e}", source.display());
            error!("{msg}");
            msg
        })?;

        if need_low {
            generate(&img, &low_path, Variant::Low.max())?;
        }
        if need_high {
            generate(&img, &high_path, Variant::High.max())?;
        }
        debug!("ensure: generated for {} (low={need_low}, high={need_high})", source.display());
    }

    Ok(Thumbnails {
        low: low_path.to_string_lossy().into_owned(),
        high: high_path.to_string_lossy().into_owned(),
    })
}

/// Ensure both sizes (the folder-scan default). Equivalent to `ensure(source, true, true)`.
pub fn ensure_thumbnails(source: &Path) -> Result<Thumbnails, String> {
    ensure(source, true, true)
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
    // Unique temp name per call (pid + monotonic counter): two concurrent generations of
    // the SAME thumbnail (e.g. the viewer's get_thumbnails racing the folder pipeline, or a
    // rescan overlapping the prior run) must not share one temp file, or their interleaved
    // writes would corrupt it. Each writer fills its own temp, then renames into place.
    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
    let uniq = format!("{}.{}", std::process::id(), TMP_SEQ.fetch_add(1, Ordering::Relaxed));
    let tmp = parent.join(format!(".{fname}.{uniq}.tmp"));

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

    fs::rename(&tmp, dst).or_else(|e| {
        // Always drop our temp; the destination is what matters.
        let _ = fs::remove_file(&tmp);
        // A concurrent writer may have produced `dst` first; on Windows `rename` refuses to
        // overwrite an existing file. If that file is a valid thumbnail, the race is benign —
        // the other writer won, and our output is identical.
        if is_valid(dst) {
            Ok(())
        } else {
            let msg = format!("rename thumbnail {}: {e}", dst.display());
            error!("{msg}");
            Err(msg)
        }
    })
}
