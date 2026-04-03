use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::DynamicImage;
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter, Cursor};
use std::path::{Path, PathBuf};

use crate::types::ReadResult;
use crate::xmp::{parse_xmp, read_jpeg_xmp_fast, read_png_xmp_fast, read_webp_xmp_fast};

pub struct PhotoProcessor {
    cache_dir: PathBuf,
}

/// Paths to the two thumbnail sizes. Each entry is either a cache file
/// (resized copy) or the original image path (when the original is already
/// within the target size and no resize is needed).
pub struct ThumbnailPaths {
    pub thumb_360: PathBuf,
    pub thumb_1920: PathBuf,
}

pub struct ProcessResult {
    pub metadata: ReadResult,
    pub thumbnails: ThumbnailPaths,
}

impl PhotoProcessor {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self { cache_dir: cache_dir.into() }
    }

    /// Check cache without processing the image.
    /// Returns thumbnail paths from the sidecar if it exists, or `None` if the
    /// image has not been processed yet.
    pub fn cached_thumbs(&self, path: &Path) -> Option<ThumbnailPaths> {
        let meta = fs::metadata(path).ok()?;
        let key = cache_key(path, &meta);
        let sidecar = self.cache_dir.join(format!("{key}.paths"));
        let content = fs::read_to_string(&sidecar).ok()?;
        let mut lines = content.lines();
        let t360 = PathBuf::from(lines.next()?);
        let t1920 = PathBuf::from(lines.next()?);
        Some(ThumbnailPaths { thumb_360: t360, thumb_1920: t1920 })
    }

    /// Process an image in a single file read:
    /// 1. Check `{key}.paths` sidecar — if present, stream only the XMP header (fast path).
    /// 2. Otherwise, read the full file once, extract XMP, decode, resize, write cache.
    ///
    /// The sidecar approach works for images of any size:
    /// - Large images  → thumbnail files are written and referenced from the sidecar.
    /// - Small images  → original path is stored in the sidecar; no redundant copy.
    pub fn process(&self, path: &Path) -> Result<ProcessResult, String> {
        let meta = fs::metadata(path).map_err(|e| e.to_string())?;
        let key = cache_key(path, &meta);
        let sidecar = self.cache_dir.join(format!("{key}.paths"));

        if sidecar.exists() {
            // Fast path: read cached thumbnail paths from sidecar, stream XMP only.
            let content = fs::read_to_string(&sidecar).map_err(|e| e.to_string())?;
            let mut lines = content.lines();
            let thumb_360 = PathBuf::from(lines.next().unwrap_or(""));
            let thumb_1920 = PathBuf::from(lines.next().unwrap_or(""));

            let file = File::open(path).map_err(|e| e.to_string())?;
            let mut reader = BufReader::new(file);
            let xmp = read_xmp(&mut reader, path)?;
            let metadata = xmp.as_deref().map(parse_xmp).unwrap_or_default();
            return Ok(ProcessResult {
                metadata,
                thumbnails: ThumbnailPaths { thumb_360, thumb_1920 },
            });
        }

        // Slow path: one syscall reads the whole file.
        let bytes = fs::read(path).map_err(|e| e.to_string())?;

        // Extract XMP from the in-memory buffer — zero extra I/O.
        let mut cursor = Cursor::new(bytes.as_slice());
        let xmp = read_xmp(&mut cursor, path)?;
        let metadata = xmp.as_deref().map(parse_xmp).unwrap_or_default();

        let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;

        let thumb_360_cache = self.cache_dir.join(format!("{key}_360.jpg"));
        let thumb_1920_cache = self.cache_dir.join(format!("{key}_1920.jpg"));

        // If the original is already within the target size, reference it directly
        // instead of writing a redundant copy.
        let thumb_360 = match make_thumbnail(&img, 360) {
            None => path.to_path_buf(),
            Some(resized) => {
                save_jpeg(&resized, &thumb_360_cache, 85)?;
                thumb_360_cache
            }
        };

        let thumb_1920 = match make_thumbnail(&img, 1920) {
            None => path.to_path_buf(),
            Some(resized) => {
                save_jpeg(&resized, &thumb_1920_cache, 85)?;
                thumb_1920_cache
            }
        };

        // Write sidecar so future calls take the fast path regardless of image size.
        write_sidecar(&sidecar, &thumb_360, &thumb_1920)?;

        Ok(ProcessResult {
            metadata,
            thumbnails: ThumbnailPaths { thumb_360, thumb_1920 },
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn cache_key(path: &Path, meta: &fs::Metadata) -> String {
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    meta.modified().ok().hash(&mut h);
    meta.len().hash(&mut h);
    format!("{:016x}", h.finish())
}

fn write_sidecar(sidecar: &Path, t360: &Path, t1920: &Path) -> Result<(), String> {
    if let Some(parent) = sidecar.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = format!("{}\n{}\n", t360.display(), t1920.display());
    fs::write(sidecar, content).map_err(|e| e.to_string())
}

/// Dispatch to the correct format-specific XMP reader based on file extension.
fn read_xmp<R: std::io::Read + std::io::Seek>(
    r: &mut R,
    path: &Path,
) -> Result<Option<Vec<u8>>, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => read_jpeg_xmp_fast(r).map_err(|e| e.to_string()),
        Some("png") => read_png_xmp_fast(r).map_err(|e| e.to_string()),
        Some("webp") => read_webp_xmp_fast(r).map_err(|e| e.to_string()),
        _ => Ok(None),
    }
}

/// Returns `None` if the image's longest side is already within `target` pixels.
/// Otherwise returns a resized copy that fits in a `target × target` box
/// with the original aspect ratio preserved.
fn make_thumbnail(img: &DynamicImage, target: u32) -> Option<DynamicImage> {
    if img.width().max(img.height()) <= target {
        return None;
    }
    Some(img.resize(target, target, FilterType::Lanczos3))
}

fn save_jpeg(img: &DynamicImage, path: &Path, quality: u8) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(file);
    let encoder = JpegEncoder::new_with_quality(&mut writer, quality);
    img.write_with_encoder(encoder).map_err(|e| e.to_string())
}
