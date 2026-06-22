use attributor_lib::folder::{scan_dir, PhotoFolder};
use attributor_lib::photo_metadata::ensure_thumbnails;
use std::fs;
use std::path::{Path, PathBuf};

// ── Helpers ───────────────────────────────────────────────────────────────────

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir()
            .join(format!("attributor_folder_{}_{:?}", tag, std::thread::current().id()));
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

/// Writes an RGB image at `rel` (creating parent dirs) and returns its path.
fn make_image(dir: &Path, rel: &str, w: u32, h: u32) -> PathBuf {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    image::RgbImage::from_pixel(w, h, image::Rgb([90, 120, 150]))
        .save(&p)
        .unwrap_or_else(|e| panic!("encode {rel}: {e}"));
    p
}

// ── US1: scan is fast, excludes _thumbnail, sets deterministic paths, no generation ──

#[test]
fn test_scan_excludes_thumbnail_and_sets_deterministic_paths() {
    let dir = TempDir::new("scan");
    make_image(dir.path(), "a.jpg", 100, 80);
    make_image(dir.path(), "sub/b.png", 100, 80);
    fs::create_dir_all(dir.path().join("_thumbnail")).unwrap();
    fs::write(dir.path().join("_thumbnail/stray.low.jpg"), [0xFF, 0xD8]).unwrap();

    let tree = scan_dir(dir.path()).expect("scan");

    assert!(!tree.children.iter().any(|c| c.name == "_thumbnail"), "_thumbnail excluded");
    let a = tree.children.iter().find(|c| c.name == "a.jpg").expect("a.jpg node");
    assert!(a.thumb_low.as_deref().unwrap().ends_with("a.jpg.low.jpg"));
    assert!(a.thumb_high.as_deref().unwrap().ends_with("a.jpg.high.jpg"));
    assert!(
        !Path::new(a.thumb_low.as_deref().unwrap()).exists(),
        "scan must NOT generate thumbnail files"
    );
}

// ── US3: enumerate photo paths ───────────────────────────────────────────────

#[test]
fn test_photo_paths_enumerates_excluding_thumbnail() {
    let dir = TempDir::new("enum");
    make_image(dir.path(), "a.jpg", 60, 60);
    make_image(dir.path(), "sub/b.png", 60, 60);
    make_image(dir.path(), "sub/c.webp", 60, 60);

    let tree = scan_dir(dir.path()).unwrap();
    let paths = PhotoFolder::photo_paths(&tree);

    assert_eq!(paths.len(), 3, "all photos enumerated: {paths:?}");
    assert!(paths.iter().all(|p| !p.contains("_thumbnail")));
}

// ── US2: generation creates then reuses ──────────────────────────────────────

#[test]
fn test_generation_creates_and_reuses() {
    let dir = TempDir::new("gen");
    make_image(dir.path(), "g.jpg", 800, 400);

    let tree = scan_dir(dir.path()).unwrap();
    let paths = PhotoFolder::photo_paths(&tree);

    for p in &paths {
        ensure_thumbnails(Path::new(p)).expect("generate");
    }
    let high = dir.path().join("_thumbnail/g.jpg.high.jpg");
    assert!(high.exists(), "thumbnail created");
    let m1 = fs::metadata(&high).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    for p in &paths {
        ensure_thumbnails(Path::new(p)).expect("generate again");
    }
    let m2 = fs::metadata(&high).unwrap().modified().unwrap();
    assert_eq!(m1, m2, "valid thumbnails reused, not regenerated");
}

// ── US3: corrupt photo is enumerated but fails generation gracefully ─────────

#[test]
fn test_corrupt_photo_errors_gracefully() {
    let dir = TempDir::new("corrupt");
    let bad = dir.path().join("bad.jpg");
    fs::write(&bad, b"not a jpeg").unwrap();

    let tree = scan_dir(dir.path()).unwrap();
    assert_eq!(PhotoFolder::photo_paths(&tree).len(), 1, "corrupt file still enumerated");
    assert!(ensure_thumbnails(&bad).is_err(), "generation returns Err (pool would log+skip)");
}

// ── US3 (FR-009): rescan picks up a newly added photo (then it gets scheduled) ──

#[test]
fn test_rescan_picks_up_added_photo() {
    let dir = TempDir::new("rescan");
    make_image(dir.path(), "first.jpg", 60, 60);

    let t1 = scan_dir(dir.path()).unwrap();
    assert_eq!(PhotoFolder::photo_paths(&t1).len(), 1);

    make_image(dir.path(), "second.png", 60, 60);
    let t2 = scan_dir(dir.path()).unwrap();
    assert_eq!(PhotoFolder::photo_paths(&t2).len(), 2, "rescan picks up the new photo");
}
