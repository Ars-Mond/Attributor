//! File fingerprint: size + mtime + full-file xxh3 hash. The hash is authoritative for content
//! identity — a hash match means the file is unchanged even if mtime differs (the stored mtime is
//! then silently refreshed). Only a hash difference is a real content change.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use xxhash_rust::xxh3::Xxh3;

/// Identity of a file's content at a point in time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub size: u64,
    pub mtime: i64, // Unix nanoseconds
    pub hash: u64,  // xxh3-64 of the whole file
}

/// Compute the fingerprint: stat for size + mtime, then stream the whole file through xxh3-64.
pub fn compute(path: &Path) -> Result<Fingerprint, String> {
    let meta = std::fs::metadata(path).map_err(|e| {
        log::warn!("fingerprint: stat failed for {}: {e}", path.display());
        e.to_string()
    })?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);

    let mut file = File::open(path).map_err(|e| {
        log::warn!("fingerprint: open failed for {}: {e}", path.display());
        e.to_string()
    })?;
    let mut hasher = Xxh3::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(Fingerprint { size, mtime, hash: hasher.digest() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_depends_on_content() {
        let path = std::env::temp_dir().join(format!("attributor_fp_test_{}.bin", std::process::id()));
        std::fs::write(&path, b"alpha").unwrap();
        let a = compute(&path).unwrap();
        // Same content → same hash and size.
        std::fs::write(&path, b"alpha").unwrap();
        let b = compute(&path).unwrap();
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.size, b.size);
        // Different content → different hash.
        std::fs::write(&path, b"beta!").unwrap();
        let c = compute(&path).unwrap();
        assert_ne!(a.hash, c.hash);
        std::fs::remove_file(&path).ok();
    }
}
