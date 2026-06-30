# Contract: Photo Module Public API (Rust)

Module `crate::photo`. Internal backend contract (not an IPC surface). All fallible methods return
`Result<_, String>` and never panic; errors are logged at the call site.

## Types

```rust
pub struct Photo { /* path + detected file type */ }

#[derive(Serialize, Default, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub category: String,
}
```

## Methods

| Method | Signature | Behavior |
|--------|-----------|----------|
| open | `fn open(path: impl AsRef<Path>) -> Result<Photo, String>` | Detect file type; error on unsupported format. No file body loaded. |
| read_metadata | `fn read_metadata(&self) -> Result<Metadata, String>` | Streaming `Seek+Read` via `little_exif::Metadata::new_from_path`; merge EXIF→IPTC→XMP with EXIF-wins precedence; keywords = union. Missing/corrupt blocks skipped+logged; no metadata ⇒ empty `Metadata`. |
| save_metadata | `fn save_metadata(&self, meta: &Metadata) -> Result<(), String>` | Write each non-empty field to every format-supported block (EXIF/IPTC/XMP), duplicated; remove cleared fields from all blocks; preserve image pixels and unrelated tags; persist via `write_to_file`. |
| decode_image | `fn decode_image(&self) -> Result<image::RgbaImage, String>` | Decode pixels to RGBA in-process via the `image` crate. No metadata parsed, file unmodified, never crosses IPC. Reserved for future features. |

## Guarantees (map to FR / SC)

- read_metadata streams; peak memory independent of file size — FR-008, SC-004.
- Precedence EXIF > IPTC > XMP; empty never overrides — FR-003, SC-001.
- save duplicates to all supported blocks; skips unsupported (block,format) with a log — FR-005, FR-011, SC-002.
- save preserves pixels byte-for-byte and unrelated tags — FR-006, FR-007, SC-003.
- cleared field removed from every block — FR-015, SC-008.
- decode_image independent of metadata and IPC-free — FR-009.
- no operation panics; each error is logged and surfaced as `Err(String)` — FR-012, VI, IX.
