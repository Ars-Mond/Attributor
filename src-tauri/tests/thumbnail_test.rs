use attributor_lib::photo_metadata::{ensure, ensure_thumbnails, thumbnail_dir_exists};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Temp working directory removed on drop (even on panic).
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir()
            .join(format!("attributor_thumb_{}_{:?}", tag, std::thread::current().id()));
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

/// Writes a solid-color source image of size `w`×`h` named `name` into `dir`.
fn make_source(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
    let p = dir.join(name);
    // RGB (no alpha) so the source encodes to JPEG as well as PNG/WebP.
    image::RgbImage::from_pixel(w, h, image::Rgb([100, 140, 180]))
        .save(&p)
        .unwrap_or_else(|e| panic!("encode {name}: {e}"));
    p
}

/// Decoded dimensions of a thumbnail file.
fn dims(path: &str) -> (u32, u32) {
    let img = image::open(path).expect("open thumbnail");
    (img.width(), img.height())
}

// ── Geometry: longest side, no upscale ────────────────────────────────────────

#[test]
fn test_high_longest_side_1920_landscape() {
    let dir = TempDir::new("high_ls");
    let src = make_source(dir.path(), "land.jpg", 4000, 2000);
    let t = ensure_thumbnails(&src).expect("ensure");
    assert_eq!(dims(&t.high), (1920, 960), "high: longest side 1920, aspect 2:1");
}

#[test]
fn test_low_longest_side_360_and_folder_created() {
    let dir = TempDir::new("low");
    let src = make_source(dir.path(), "a.jpg", 2000, 1000);
    let t = ensure_thumbnails(&src).expect("ensure");

    assert!(dir.path().join("_thumbnail").is_dir(), "_thumbnail folder created");
    assert_eq!(dims(&t.low), (360, 180), "low: longest side 360, aspect 2:1");
    assert!(t.low.ends_with("a.jpg.low.jpg"), "deterministic low name: {}", t.low);
    assert!(t.high.ends_with("a.jpg.high.jpg"), "deterministic high name: {}", t.high);
}

#[test]
fn test_no_upscale_when_source_smaller() {
    let dir = TempDir::new("noupscale");
    // Portrait, longest side 1000 < 1920 → high must not upscale.
    let src = make_source(dir.path(), "port.png", 500, 1000);
    let t = ensure_thumbnails(&src).expect("ensure");
    assert_eq!(dims(&t.high), (500, 1000), "high not upscaled");
    assert_eq!(dims(&t.low), (180, 360), "low still downscaled to longest side 360");
}

// ── Formats: JPEG / PNG / WebP ────────────────────────────────────────────────

#[test]
fn test_all_formats_produce_thumbnails() {
    let dir = TempDir::new("fmt");
    for name in ["x.jpg", "x.png", "x.webp"] {
        let src = make_source(dir.path(), name, 800, 400);
        let t = ensure_thumbnails(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(Path::new(&t.low).is_file(), "{name}: low exists");
        assert!(Path::new(&t.high).is_file(), "{name}: high exists");
        assert_eq!(dims(&t.low), (360, 180), "{name}: low geometry");
    }
}

// ── Reuse, regeneration, graceful failure ─────────────────────────────────────

#[test]
fn test_reuse_no_regeneration() {
    let dir = TempDir::new("reuse");
    let src = make_source(dir.path(), "r.jpg", 1000, 1000);

    let t1 = ensure_thumbnails(&src).expect("first");
    let m1 = fs::metadata(&t1.high).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    let t2 = ensure_thumbnails(&src).expect("second");
    let m2 = fs::metadata(&t2.high).unwrap().modified().unwrap();

    assert_eq!(m1, m2, "a valid thumbnail must be reused, not regenerated");
}

#[test]
fn test_regenerate_on_invalid() {
    let dir = TempDir::new("invalid");
    let src = make_source(dir.path(), "i.jpg", 800, 800);
    let t = ensure_thumbnails(&src).expect("first");

    fs::write(&t.low, b"").expect("corrupt low to 0 bytes");
    let t2 = ensure_thumbnails(&src).expect("second");

    let mut magic = [0u8; 2];
    fs::File::open(&t2.low).unwrap().read_exact(&mut magic).unwrap();
    assert_eq!(magic, [0xFF, 0xD8], "invalid thumbnail regenerated to a real JPEG");
}

#[test]
fn test_corrupt_source_errors_gracefully() {
    let dir = TempDir::new("corrupt");
    let src = dir.path().join("bad.jpg");
    fs::write(&src, b"this is not a real jpeg").unwrap();

    let result = ensure_thumbnails(&src);
    assert!(result.is_err(), "corrupt source must return Err (no panic)");
}

// ── Concurrency: two producers racing on the same photo (viewer ↔ pipeline) ────

#[test]
fn test_concurrent_generation_same_photo_is_safe() {
    let dir = TempDir::new("concurrent");
    let src = make_source(dir.path(), "race.jpg", 2400, 1200);

    // Many threads generate the SAME photo's thumbnails at once — this mirrors the viewer's
    // get_thumbnails racing the folder pipeline (or a rescan overlapping the prior run). Each
    // call must succeed with a valid result; the unique temp name + rename tolerance prevent a
    // shared-temp corruption / lost-rename race between concurrent writers.
    let results = std::thread::scope(|s| {
        let handles: Vec<_> = (0..8).map(|_| s.spawn(|| ensure_thumbnails(&src))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect::<Vec<_>>()
    });

    for r in &results {
        let t = r.as_ref().expect("every concurrent ensure_thumbnails must succeed");
        for p in [&t.low, &t.high] {
            let mut magic = [0u8; 2];
            fs::File::open(p).unwrap().read_exact(&mut magic).unwrap();
            assert_eq!(magic, [0xFF, 0xD8], "concurrent output must be a valid JPEG: {p}");
        }
    }

    // Destination geometry is intact (no half-written/interleaved thumbnail).
    assert_eq!(dims(&results[0].as_ref().unwrap().high), (1920, 960), "high geometry under concurrency");

    // Every writer cleaned up its unique temp — no leftovers in the cache folder.
    let leftovers: Vec<_> = fs::read_dir(dir.path().join("_thumbnail"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "no leftover .tmp files: {leftovers:?}");
}

// ── Per-size generation (feature 005: ensure(low, high)) ──────────────────────

#[test]
fn test_ensure_high_only() {
    let dir = TempDir::new("high_only");
    let src = make_source(dir.path(), "h.jpg", 2000, 1000);
    let t = ensure(&src, false, true).expect("ensure high");

    assert!(Path::new(&t.high).is_file(), "high generated");
    assert!(!Path::new(&t.low).exists(), "low NOT generated");
    assert_eq!(dims(&t.high), (1920, 960));

    // A valid high is reused, not regenerated.
    let m1 = fs::metadata(&t.high).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let t2 = ensure(&src, false, true).expect("ensure high again");
    let m2 = fs::metadata(&t2.high).unwrap().modified().unwrap();
    assert_eq!(m1, m2, "valid high reused, not regenerated");
}

#[test]
fn test_ensure_low_only() {
    let dir = TempDir::new("low_only");
    let src = make_source(dir.path(), "l.jpg", 2000, 1000);
    let t = ensure(&src, true, false).expect("ensure low");

    assert!(Path::new(&t.low).is_file(), "low generated");
    assert!(!Path::new(&t.high).exists(), "high NOT generated");
    assert_eq!(dims(&t.low), (360, 180));
}

#[test]
fn test_ensure_both() {
    let dir = TempDir::new("both");
    let src = make_source(dir.path(), "b.jpg", 2000, 1000);
    let t = ensure(&src, true, true).expect("ensure both");

    assert!(Path::new(&t.low).is_file() && Path::new(&t.high).is_file(), "both generated");
    assert_eq!(dims(&t.low), (360, 180));
    assert_eq!(dims(&t.high), (1920, 960));
}

#[test]
fn test_ensure_none_is_noop() {
    let dir = TempDir::new("none");
    let src = make_source(dir.path(), "n.jpg", 800, 600);
    let t = ensure(&src, false, false).expect("ensure none");

    assert!(!Path::new(&t.low).exists() && !Path::new(&t.high).exists(), "nothing generated");
    assert!(!dir.path().join("_thumbnail").exists(), "no _thumbnail folder created");
}

#[test]
fn test_thumbnail_dir_exists() {
    let dir = TempDir::new("dir_exists");
    let src = make_source(dir.path(), "x.jpg", 400, 300);
    assert!(!thumbnail_dir_exists(dir.path(), false), "missing while the folder has a photo and no cache");

    ensure(&src, true, false).expect("ensure low");
    assert!(thumbnail_dir_exists(dir.path(), false), "_thumbnail exists after generation");

    fs::remove_dir_all(dir.path().join("_thumbnail")).expect("delete cache");
    assert!(!thumbnail_dir_exists(dir.path(), false), "detected after deletion");
}
