//! Recursive folder scan: build the `FileNode` tree, exclude `_thumbnail` cache folders,
//! and record each photo's deterministic thumbnail paths. No thumbnails are generated here
//! (that is the pipeline's job) — the scan returns fast.

use std::path::Path;

use log::warn;

use super::FileNode;

pub fn scan_dir(path: &Path) -> std::io::Result<FileNode> {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .to_string();

    let mut children = Vec::new();
    let mut thumb_low = None;
    let mut thumb_high = None;

    let is_dir = path.metadata().map(|m| m.is_dir()).unwrap_or(false);

    if is_dir {
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    warn!("scan_dir: skipping entry in {}: {err}", path.display());
                    None
                }
            })
            .collect();

        entries.sort_by(|a, b| {
            let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
        });

        for entry in entries {
            let child = entry.path();
            let child_is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            // Hide the thumbnail cache folder from the hierarchy.
            if child_is_dir && entry.file_name().to_str() == Some("_thumbnail") {
                continue;
            }
            if child_is_dir || is_supported_image(&child) {
                match scan_dir(&child) {
                    Ok(node) => children.push(node),
                    Err(err) => warn!("scan_dir: skipping {}: {err}", child.display()),
                }
            }
        }
    } else if is_supported_image(path) {
        // Deterministic paths only — generation is performed by the pipeline.
        let t = crate::photo::thumbnail_paths(path);
        thumb_low = Some(t.low);
        thumb_high = Some(t.high);
    }

    Ok(FileNode {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir,
        children,
        thumb_low,
        thumb_high,
    })
}

pub(crate) fn is_supported_image(path: &Path) -> bool {
    is_supported_image_name(&path.to_string_lossy())
}

pub(crate) fn is_supported_image_name(name: &str) -> bool {
    matches!(
        Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp")
    )
}

#[cfg(test)]
mod tests {
    use super::scan_dir;
    use std::fs;

    #[test]
    fn scan_dir_excludes_thumbnail_folder() {
        let base = std::env::temp_dir()
            .join(format!("attributor_folder_{:?}", std::thread::current().id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("_thumbnail")).unwrap();
        fs::create_dir_all(base.join("sub")).unwrap();

        let node = scan_dir(&base).expect("scan_dir");

        assert!(node.children.iter().any(|c| c.name == "sub"), "regular subfolder kept");
        assert!(
            !node.children.iter().any(|c| c.name == "_thumbnail"),
            "_thumbnail folder must be excluded"
        );

        let _ = fs::remove_dir_all(&base);
    }
}
