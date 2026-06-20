use attributor_lib::photo_metadata::{read_metadata, write_metadata, Metadata, Photo};
use std::fs;
use std::path::{Path, PathBuf};

use little_exif::exif_tag::ExifTag;
use little_exif::exif_tag_format::Utf16String;
use little_exif::iptc::IptcData;
use little_exif::metadata::Metadata as LeMetadata;
use little_exif::xmp::XmpData;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn test_image_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../test_images/test_img_exif.jpg")
}

/// RAII guard that deletes the file on drop (even if the test panics).
struct TempFile(PathBuf);

impl TempFile {
    fn new(path: PathBuf) -> Self {
        TempFile(path)
    }
    fn path_str(&self) -> String {
        self.0.to_str().unwrap().to_string()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Copies the EXIF test JPEG to a unique temp path; returns the guard and its path.
fn temp_copy_of_test_jpeg(tag: &str) -> (TempFile, String) {
    let dst = std::env::temp_dir()
        .join(format!("attributor_{}_{:?}.jpg", tag, std::thread::current().id()));
    fs::copy(test_image_path(), &dst).expect("failed to copy test image to temp dir");
    let guard = TempFile::new(dst);
    let path = guard.path_str();
    (guard, path)
}

/// Minimal XMP packet carrying a single `dc:title`.
fn xmp_with_title(title: &str) -> Vec<u8> {
    format!(
        "<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\
<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{title}</rdf:li></rdf:Alt></dc:title>\
</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end='w'?>"
    )
    .into_bytes()
}

const TITLE: &str = "Fuzzy hanging leaves of fluffy sumac on a green background resemble auroras";
const EXPECTED_KW_COUNT: usize = 36;

// ── Read real file ────────────────────────────────────────────────────────────

#[test]
fn test_read_jpeg_fields() {
    let path = test_image_path().to_str().unwrap().to_string();
    let m = read_metadata(path).expect("read_metadata should succeed");

    assert_eq!(m.title, TITLE, "title mismatch");
    assert_eq!(m.description, TITLE, "description mismatch");
    assert_eq!(
        m.keywords.len(),
        EXPECTED_KW_COUNT,
        "expected {EXPECTED_KW_COUNT} unique keywords, got {}",
        m.keywords.len()
    );
    assert!(
        m.keywords.iter().any(|k| k == "Background"),
        "keywords should contain 'Background'"
    );
    assert!(
        m.keywords.iter().any(|k| k == "copy space"),
        "keywords should contain 'copy space'"
    );
    assert_eq!(m.category, "", "category should be empty (no XMP in this file)");
}

// ── Write / read round-trip (JPEG) ────────────────────────────────────────────

#[test]
fn test_write_read_round_trip_jpeg() {
    let (_guard, path) = temp_copy_of_test_jpeg("rt");

    let new_meta = Metadata {
        title: "Round-trip Title".to_string(),
        description: "Round-trip description text.".to_string(),
        keywords: vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
        category: "nature".to_string(),
    };

    write_metadata(path.clone(), new_meta).expect("write_metadata should succeed");
    let result = read_metadata(path).expect("read_metadata after write should succeed");

    assert_eq!(result.title, "Round-trip Title");
    assert_eq!(result.description, "Round-trip description text.");
    assert_eq!(result.keywords, vec!["alpha", "beta", "gamma"]);
    assert_eq!(result.category, "nature");
}

// ── US1: merge precedence EXIF > IPTC > XMP ───────────────────────────────────

#[test]
fn test_exif_wins_over_iptc_and_xmp() {
    let (_guard, path) = temp_copy_of_test_jpeg("prec");

    // Seed the same logical field (title) with a different value in each block.
    let mut le = LeMetadata::new_from_path(Path::new(&path)).unwrap_or_else(|_| LeMetadata::new());
    le.set_tag(ExifTag::XPTitle(Utf16String::from("EXIF_TITLE")));
    let mut iptc = IptcData::new();
    iptc.set_field(2, 5, b"IPTC_TITLE".to_vec()); // 2:05 Object Name
    le.set_iptc(iptc);
    le.set_xmp(XmpData::from_raw(xmp_with_title("XMP_TITLE")));
    le.write_to_file(Path::new(&path)).expect("seed write should succeed");

    let m = read_metadata(path).expect("read should succeed");
    assert_eq!(m.title, "EXIF_TITLE", "EXIF must win over IPTC and XMP");
}

// ── US2: cleared field removed from every block ───────────────────────────────

#[test]
fn test_cleared_field_removed_from_all_blocks() {
    let (_guard, path) = temp_copy_of_test_jpeg("clear");

    // Full set first.
    write_metadata(
        path.clone(),
        Metadata {
            title: "T".into(),
            description: "D".into(),
            keywords: vec!["k1".into()],
            category: "c".into(),
        },
    )
    .expect("initial write");

    // Clear title and keywords; keep description and category.
    write_metadata(
        path.clone(),
        Metadata {
            title: String::new(),
            description: "D".into(),
            keywords: vec![],
            category: "c".into(),
        },
    )
    .expect("second write");

    let m = read_metadata(path.clone()).expect("read");
    assert_eq!(m.title, "", "cleared title should be empty");
    assert!(m.keywords.is_empty(), "cleared keywords should be empty");
    assert_eq!(m.description, "D", "untouched field stays");

    // Block-level: managed tags must be gone, not blanked.
    let le = LeMetadata::new_from_path(Path::new(&path)).expect("le read");
    assert!(le.get_tag_by_hex(0x9c9b, None).next().is_none(), "EXIF XPTitle removed");
    assert!(le.get_tag_by_hex(0x9c9e, None).next().is_none(), "EXIF XPKeywords removed");
    if let Some(iptc) = le.get_iptc() {
        assert!(iptc.get_fields(2, 5).is_empty(), "IPTC ObjectName removed");
        assert!(iptc.get_fields(2, 25).is_empty(), "IPTC Keywords removed");
    }
}

// ── US2: metadata save preserves image pixels ─────────────────────────────────

#[test]
fn test_save_preserves_pixels() {
    let (_guard, path) = temp_copy_of_test_jpeg("pixels");

    let before = Photo::open(&path).unwrap().decode_image().expect("decode before");
    write_metadata(
        path.clone(),
        Metadata {
            title: "X".into(),
            description: "Y".into(),
            keywords: vec!["z".into()],
            category: "c".into(),
        },
    )
    .expect("write");
    let after = Photo::open(&path).unwrap().decode_image().expect("decode after");

    assert_eq!(before.dimensions(), after.dimensions(), "dimensions unchanged");
    assert_eq!(before.as_raw(), after.as_raw(), "pixels must be unchanged by a metadata save");
}

// ── US3: in-process RGBA decode ───────────────────────────────────────────────

#[test]
fn test_decode_image_returns_rgba() {
    let path = test_image_path().to_str().unwrap().to_string();
    let size_before = fs::metadata(&path).unwrap().len();

    let img = Photo::open(&path).unwrap().decode_image().expect("decode");
    assert!(img.width() > 0 && img.height() > 0, "non-empty image");
    assert_eq!(
        img.as_raw().len(),
        (img.width() * img.height() * 4) as usize,
        "RGBA = 4 bytes per pixel"
    );

    // Decoding must not modify the file.
    assert_eq!(fs::metadata(&path).unwrap().len(), size_before, "file unchanged by decode");
}

// ── PNG / WebP round-trips (generated fixtures) ───────────────────────────────

fn round_trip_generated(ext: &str) {
    let path = std::env::temp_dir()
        .join(format!("attributor_rt_{}_{:?}.{}", ext, std::thread::current().id(), ext));
    let guard = TempFile::new(path.clone());

    image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 255]))
        .save(&path)
        .unwrap_or_else(|e| panic!("failed to encode test {ext}: {e}"));

    let p = guard.path_str();
    let meta = Metadata {
        title: "PT".into(),
        description: "PD".into(),
        keywords: vec!["a".into(), "b".into()],
        category: "cat".into(),
    };
    write_metadata(p.clone(), meta).expect("write");

    let read = read_metadata(p).expect("read");
    assert_eq!(read.title, "PT", "{ext} title round-trip");
    assert_eq!(read.description, "PD", "{ext} description round-trip");
    assert_eq!(read.keywords, vec!["a", "b"], "{ext} keywords round-trip");
    assert_eq!(read.category, "cat", "{ext} category (XMP) round-trip");
}

#[test]
fn test_png_round_trip() {
    round_trip_generated("png");
}

#[test]
fn test_webp_round_trip() {
    round_trip_generated("webp");
}
