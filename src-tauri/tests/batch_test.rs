use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use attributor_lib::batch::save_batch;
use attributor_lib::events::{BatchProgress, ItemStatus};
use attributor_lib::photo_metadata::read_metadata;
use attributor_lib::SaveRequest;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Temp working directory removed on drop (even on panic).
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir()
            .join(format!("attributor_batch_{}_{:?}", tag, std::thread::current().id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_jpeg(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    image::RgbImage::from_pixel(64, 48, image::Rgb([120, 130, 140]))
        .save(&p)
        .unwrap_or_else(|e| panic!("encode {name}: {e}"));
    p
}

/// A batch item that keeps the file's own stem (so no rename happens).
fn req(path: &Path, title: &str, keywords: &[&str]) -> SaveRequest {
    SaveRequest {
        filepath: path.to_string_lossy().to_string(),
        filename: path.file_stem().unwrap().to_string_lossy().to_string(),
        title: title.into(),
        description: "desc".into(),
        keywords: keywords.iter().map(|s| s.to_string()).collect(),
        categories: String::new(),
        release_filename: String::new(),
        editorial: false,
        mature_content: false,
        illustration: false,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn test_batch_saves_all_concurrently() {
    let dir = TempDir::new("all");
    let files: Vec<PathBuf> = (0..6).map(|i| make_jpeg(dir.path(), &format!("p{i}.jpg"))).collect();
    let items: Vec<SaveRequest> = files
        .iter()
        .enumerate()
        .map(|(i, p)| req(p, &format!("Title {i}"), &["alpha", "beta"]))
        .collect();

    let cancel = Arc::new(AtomicBool::new(false));
    let progress: Mutex<Vec<BatchProgress>> = Mutex::new(Vec::new());
    let results = save_batch(items, &cancel, |m| progress.lock().unwrap().push(m));

    assert_eq!(results.len(), 6);
    assert!(
        results.iter().all(|s| matches!(s, ItemStatus::Ok { .. })),
        "all items saved: {results:?}"
    );
    assert_eq!(progress.lock().unwrap().len(), 6, "one progress message per file");

    // Each file reads back its metadata — identical to a single-file save.
    for (i, p) in files.iter().enumerate() {
        let meta = read_metadata(p.to_string_lossy().to_string()).expect("read back");
        assert_eq!(meta.title, format!("Title {i}"), "title round-trips for {}", p.display());
        assert!(
            meta.keywords.contains(&"alpha".to_string()) && meta.keywords.contains(&"beta".to_string()),
            "keywords round-trip for {}: {:?}",
            p.display(),
            meta.keywords
        );
    }
}

#[test]
fn test_batch_best_effort_one_failure() {
    let dir = TempDir::new("besteffort");
    let good: Vec<PathBuf> = (0..3).map(|i| make_jpeg(dir.path(), &format!("g{i}.jpg"))).collect();

    let mut items: Vec<SaveRequest> = good.iter().map(|p| req(p, "Shared", &["k1"])).collect();
    // A path under a non-existent subdirectory — its write must fail.
    let bad = dir.path().join("missing_dir").join("bad.jpg");
    items.insert(2, req(&bad, "Shared", &["k1"]));

    let cancel = Arc::new(AtomicBool::new(false));
    let results = save_batch(items, &cancel, |_| {});

    assert_eq!(results.len(), 4, "every item accounted for");
    let oks = results.iter().filter(|s| matches!(s, ItemStatus::Ok { .. })).count();
    let fails = results.iter().filter(|s| matches!(s, ItemStatus::Failed { .. })).count();
    assert_eq!(oks, 3, "all writable files saved: {results:?}");
    assert_eq!(fails, 1, "the unwritable file is reported failed: {results:?}");

    // The writable files really were written despite the sibling failure.
    for p in &good {
        let meta = read_metadata(p.to_string_lossy().to_string()).expect("read back");
        assert_eq!(meta.title, "Shared");
    }
}

#[test]
fn test_batch_cancel_skips_unstarted() {
    let dir = TempDir::new("cancel");
    let files: Vec<PathBuf> = (0..4).map(|i| make_jpeg(dir.path(), &format!("c{i}.jpg"))).collect();
    let items: Vec<SaveRequest> = files.iter().map(|p| req(p, "Should Not Persist", &["x"])).collect();

    // Pre-cancelled: no item should start, so nothing is written.
    let cancel = Arc::new(AtomicBool::new(true));
    let results = save_batch(items, &cancel, |_| {});

    assert_eq!(results.len(), 4, "every item accounted for");
    assert!(
        results.iter().all(|s| matches!(s, ItemStatus::Cancelled)),
        "a pre-cancelled batch leaves every item Cancelled: {results:?}"
    );

    // No metadata was written to any file.
    for p in &files {
        let meta = read_metadata(p.to_string_lossy().to_string()).expect("read back");
        assert!(meta.title.is_empty(), "cancelled item must not be written: {}", p.display());
    }
}
