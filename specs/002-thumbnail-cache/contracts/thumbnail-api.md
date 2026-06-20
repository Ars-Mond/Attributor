# Contract: Thumbnail Module API (Rust)

Module `crate::photo::thumbnail`. Internal backend contract (not an IPC surface). Fallible calls
return `Result<_, String>`, never panic; errors are logged at the call site.

## Types & constants

```rust
pub const LOW_MAX:  u32 = 360;   // longest side, low variant
pub const HIGH_MAX: u32 = 1920;  // longest side, high variant
pub const JPEG_QUALITY: u8 = 75; // strong compression, tune to SC-003

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnails {
    pub low: String,
    pub high: String,
}

enum Variant { Low, High }   // internal; max() -> LOW_MAX / HIGH_MAX
```

## Functions

| Function | Signature | Behavior |
|----------|-----------|----------|
| ensure_thumbnails | `fn ensure_thumbnails(source: &Path) -> Result<Thumbnails, String>` | Compute `_thumbnail` paths; create the folder if missing; for each variant reuse a valid existing file or generate it; return both paths. |
| (internal) generate | `fn generate(src: &DynamicImage, dst: &Path, max: u32) -> Result<(), String>` | Longest-side resize (no upscale) → `rgb8` → `JpegEncoder` at `JPEG_QUALITY` → write `dst`. |
| (internal) is_valid | `fn is_valid(path: &Path) -> bool` | True if the file exists and is non-empty (and decodes when later loaded — FR-011). |

## Guarantees (map to FR / SC)

- Low = 360 / High = 1920 on the longest side; aspect preserved; never upscaled — FR-001, FR-005, SC-007.
- Output is JPG with strong compression — FR-004, SC-003.
- `_thumbnail` created beside the source before writing — FR-003.
- Deterministic names `<file_name>.<low|high>.jpg`; same photo → same files — FR-012.
- Valid existing thumbnails reused; missing/invalid regenerated — FR-002, FR-011, SC-004.
- No path persisted to any DB/index — FR-007.
- JPEG/PNG/WebP sources, identical across OSes — FR-013, SC-008.
- Decode/write failures surface as `Err(String)` and are logged; no panic — FR-010, VI, IX.
- Heavy work in Rust, invoked off the UI thread by the command — FR-009, VIII.
