use attributor_lib::photo_metadata::{read_metadata, write_metadata, Metadata};
use std::path::{Path, PathBuf};
use std::fs;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn test_image_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test_images/test_img_exif.jpg")
}

/// RAII guard that deletes the file on drop (even if the test panics).
struct TempFile(PathBuf);

impl TempFile {
    fn new(path: PathBuf) -> Self { TempFile(path) }
    fn path_str(&self) -> String { self.0.to_str().unwrap().to_string() }
}

impl Drop for TempFile {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

const TITLE: &str = "Fuzzy hanging leaves of fluffy sumac on a green background resemble auroras";
const EXPECTED_KW_COUNT: usize = 36;

// ── Test 1: read real file ────────────────────────────────────────────────────

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
    assert_eq!(m.keywords[0], "Background", "first keyword mismatch");
    assert!(
        m.keywords.iter().any(|k| k == "copy space"),
        "keywords should contain 'copy space'"
    );
    assert_eq!(m.category, "", "category should be empty (no XMP in this file)");
}

// ── Test 2: write / read round-trip ──────────────────────────────────────────

#[test]
fn test_write_read_round_trip_jpeg() {
    let src = test_image_path();
    let dst_path = std::env::temp_dir()
        .join(format!("attributor_metadata_test_{:?}.jpg", std::thread::current().id()));

    let _guard = TempFile::new(dst_path.clone());

    fs::copy(&src, &dst_path).expect("failed to copy test image to temp dir");

    let new_meta = Metadata {
        title: "Round-trip Title".to_string(),
        description: "Round-trip description text.".to_string(),
        keywords: vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
        category: "nature".to_string(),
    };

    write_metadata(_guard.path_str(), new_meta).expect("write_metadata should succeed");

    let result = read_metadata(_guard.path_str()).expect("read_metadata after write should succeed");

    assert_eq!(result.title, "Round-trip Title");
    assert_eq!(result.description, "Round-trip description text.");
    assert_eq!(result.keywords, vec!["alpha", "beta", "gamma"]);
    assert_eq!(result.category, "nature");
}
