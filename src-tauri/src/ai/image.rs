//! Image payloads for vision requests. Ollama receives the original file bytes; every other provider receives a downscaled JPEG so cloud size limits are met and local servers get a modest payload.

use std::path::Path;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::metadata::Orientation;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageReader, RgbImage};

/// Longest side, in pixels, of the image sent to providers other than Ollama.
/// Stays within every provider's limit (Claude accepts up to 2576 px; recent OpenAI models reject images above 30 000 patches instead of downscaling them) while keeping the token cost low.
pub const MAX_EDGE: u32 = 1568;

/// Encoding qualities tried in order until the JPEG fits [`MAX_JPEG_BYTES`].
const JPEG_QUALITIES: &[u8] = &[90, 82, 74, 66];

/// Largest JPEG sent. Base64 grows it by a third, and ChatGPT sign-in rejects request bodies above about 1 MiB.
const MAX_JPEG_BYTES: usize = 700_000;

/// A base64-encoded image together with its MIME type.
pub struct ImagePayload {
    pub mime: &'static str,
    pub base64: String,
}

impl ImagePayload {
    /// `data:` URL form used by OpenAI-style `image_url` / `input_image` content parts.
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime, self.base64)
    }
}

/// Base64 of the unmodified file (standard alphabet, no `data:` prefix), as Ollama's `images` array expects.
pub fn original_base64(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Decodes the photo, applies its EXIF orientation, flattens transparency onto white, downscales it to fit `max_edge` and encodes it as JPEG of at most [`MAX_JPEG_BYTES`] (lowering the quality, then the size, for very detailed photos).
///
/// CPU-bound: run it inside `spawn_blocking`.
///
/// # Errors
///
/// Returns an error string if the file cannot be read, decoded or encoded.
pub fn prepare_jpeg(path: &str, max_edge: u32) -> Result<ImagePayload, String> {
    let rgb = flatten_onto_white(&fit(decode_oriented(Path::new(path))?, max_edge));
    let jpeg = encode_within_budget(rgb, MAX_JPEG_BYTES)?;
    Ok(ImagePayload {
        mime: "image/jpeg",
        base64: base64::engine::general_purpose::STANDARD.encode(jpeg),
    })
}

/// Downscales `img` so its longest side is at most `max_edge` (never upscales).
fn fit(img: DynamicImage, max_edge: u32) -> DynamicImage {
    let longest = img.width().max(img.height());
    if longest <= max_edge {
        return img;
    }
    // `thumbnail` averages source pixels, which is fast and alias-free for large reductions; a filtered resize looks better for small ones.
    if longest >= max_edge.saturating_mul(2) {
        img.thumbnail(max_edge, max_edge)
    } else {
        img.resize(max_edge, max_edge, image::imageops::FilterType::CatmullRom)
    }
}

fn encode_jpeg(rgb: &RgbImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("image encode failed: {e}"))?;
    Ok(buf)
}

/// Encodes `rgb` at the highest quality that fits `budget` bytes; if even the lowest quality is too large, shrinks the image by a quarter and tries again.
fn encode_within_budget(mut rgb: RgbImage, budget: usize) -> Result<Vec<u8>, String> {
    loop {
        for &quality in JPEG_QUALITIES {
            let jpeg = encode_jpeg(&rgb, quality)?;
            if jpeg.len() <= budget || rgb.width().max(rgb.height()) <= 256 {
                log::debug!(
                    "prepared image: {}x{}, quality {quality}, {} bytes",
                    rgb.width(),
                    rgb.height(),
                    jpeg.len()
                );
                return Ok(jpeg);
            }
        }
        let (w, h) = (rgb.width() * 3 / 4, rgb.height() * 3 / 4);
        rgb = image::imageops::resize(
            &rgb,
            w.max(1),
            h.max(1),
            image::imageops::FilterType::CatmullRom,
        );
    }
}

/// Decodes `path` and rotates/flips it according to its EXIF orientation tag, so the model sees the photo upright.
fn decode_oriented(path: &Path) -> Result<DynamicImage, String> {
    let fail = |e: &dyn std::fmt::Display| format!("decode failed for {}: {e}", path.display());
    let reader = ImageReader::open(path)
        .map_err(|e| fail(&e))?
        .with_guessed_format()
        .map_err(|e| fail(&e))?;
    let mut decoder = reader.into_decoder().map_err(|e| fail(&e))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| fail(&e))?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Converts to RGB, compositing any alpha channel over white (dropping alpha would expose arbitrary colors behind transparent pixels).
fn flatten_onto_white(img: &DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    for (dst, src) in out.pixels_mut().zip(rgba.pixels()) {
        let [r, g, b, a] = src.0;
        let a = u32::from(a);
        // The blend of two values in 0..=255 stays in 0..=255, so the narrowing cast never truncates.
        let blend = |c: u8| ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8;
        dst.0 = [blend(r), blend(g), blend(b)];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image(name: &str) -> String {
        format!("{}/../test_images/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn prepare_jpeg_fits_max_edge_and_is_jpeg() {
        for name in [
            "0T2A6727.jpg",
            "1662451436_803a15.png",
            "1662451436_e25f90.webp",
        ] {
            let payload = prepare_jpeg(&test_image(name), 512).unwrap();
            assert_eq!(payload.mime, "image/jpeg");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&payload.base64)
                .unwrap();
            let img = image::load_from_memory(&bytes).unwrap();
            assert!(
                img.width().max(img.height()) <= 512,
                "{name}: {}x{}",
                img.width(),
                img.height()
            );
            assert!(payload.data_url().starts_with("data:image/jpeg;base64,"));
        }
    }

    #[test]
    fn budget_lowers_quality_then_size() {
        // Noise does not compress, so a 1000×1000 image cannot fit 50 kB at any quality without shrinking.
        let mut seed = 1u32;
        let noisy = RgbImage::from_fn(1000, 1000, |_, _| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let [a, b, c, _] = seed.to_le_bytes();
            image::Rgb([a, b, c])
        });
        let jpeg = encode_within_budget(noisy, 50_000).unwrap();
        assert!(jpeg.len() <= 50_000);
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert!(decoded.width() < 1000);
    }

    #[test]
    fn flatten_composites_transparent_pixels_over_white() {
        let mut rgba = image::RgbaImage::new(2, 1);
        rgba.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
        rgba.put_pixel(1, 0, image::Rgba([10, 20, 30, 255]));
        let rgb = flatten_onto_white(&DynamicImage::ImageRgba8(rgba));
        assert_eq!(rgb.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(rgb.get_pixel(1, 0).0, [10, 20, 30]);
    }
}
