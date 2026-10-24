//! MCP settings owned by the backend: whether the server runs, its port, the access token and the AI-settings permission.
//! They live in `mcp.json` in the app-data directory, apart from the frontend settings, so the stdio bridge can read them without the app and the settings tools can never reach them.

use std::io::Write;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

/// Tauri bundle identifier; the app-data directory is named after it.
const APP_IDENTIFIER: &str = "loc.am.attributor";
/// File name of the MCP settings inside the app-data directory.
pub(crate) const CONFIG_FILE: &str = "mcp.json";
/// Port the server listens on unless the user picks another.
pub(crate) const DEFAULT_PORT: u16 = 47821;
/// Lowest port the user may pick (ports below need elevated rights on some systems).
pub(crate) const MIN_PORT: u16 = 1024;

/// Numbers the temporary files of [`write_atomic`].
static TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Persisted MCP settings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct McpConfig {
    /// Whether the HTTP server runs; the stdio bridge forwards to it, so it needs the server too.
    pub enabled: bool,
    pub port: u16,
    /// Bearer token every HTTP request must carry.
    pub token: String,
    /// Whether MCP clients may change the AI settings (provider, addresses, models, profiles, response format).
    pub allow_ai_settings: bool,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            token: String::new(),
            allow_ai_settings: false,
        }
    }
}

impl McpConfig {
    /// URL of the HTTP endpoint. It names 127.0.0.1 rather than `localhost`, which may resolve to `::1` first.
    pub(crate) fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }
}

/// Reads the settings; a missing or unreadable file yields the defaults.
pub(crate) fn load(path: &Path) -> McpConfig {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!("mcp: {} is invalid ({e}); using defaults", path.display());
            McpConfig::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => McpConfig::default(),
        Err(e) => {
            log::warn!("mcp: cannot read {} ({e}); using defaults", path.display());
            McpConfig::default()
        }
    }
}

/// Writes the settings atomically; on Unix the file is readable by the current user only, since it holds the token.
///
/// # Errors
///
/// Returns an error string when the file cannot be written.
pub(crate) fn save(path: &Path, cfg: &McpConfig) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    write_atomic(path, &json, true).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// A fresh random access token (256 bits, URL-safe base64 without padding).
///
/// # Errors
///
/// Returns an error string when the system has no secure random source.
pub(crate) fn new_token() -> Result<String, String> {
    let mut raw = [0u8; 32];
    SystemRandom::new()
        .fill(&mut raw)
        .map_err(|_| "no secure random source".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(raw))
}

/// Replaces `path` with `bytes` through a temporary file in the same directory, so readers never see a half-written file.
/// On Unix a replaced file keeps its permissions, and `private` makes a new file readable by the current user only.
///
/// # Errors
///
/// Returns the I/O error of creating, writing or renaming the temporary file.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8], private: bool) -> std::io::Result<()> {
    let invalid = || std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a file path");
    let dir = path.parent().ok_or_else(invalid)?;
    let name = path.file_name().ok_or_else(invalid)?.to_string_lossy();
    std::fs::create_dir_all(dir)?;
    // A unique name per write, so concurrent writers never share (and truncate) one temporary file.
    let n = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".{name}.{}-{n}.attributor-tmp", std::process::id()));
    let written = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        if private {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(not(unix))]
        let _ = private;
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        #[cfg(unix)]
        if let Ok(meta) = std::fs::metadata(path) {
            std::fs::set_permissions(&tmp, meta.permissions())?;
        }
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// The app-data directory as Tauri resolves it (`<data dir>/<identifier>`), for the stdio bridge, which runs without Tauri.
pub(crate) fn app_data_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(APP_IDENTIFIER))
}

#[cfg(windows)]
fn data_dir() -> Option<PathBuf> {
    non_empty_var("APPDATA")
}

#[cfg(target_os = "macos")]
fn data_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join("Library").join("Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn data_dir() -> Option<PathBuf> {
    non_empty_var("XDG_DATA_HOME")
        .filter(|dir| dir.is_absolute())
        .or_else(|| home_dir().map(|home| home.join(".local").join("share")))
}

/// The current user's home directory.
pub(crate) fn home_dir() -> Option<PathBuf> {
    non_empty_var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
}

/// An environment variable holding a path, ignored when empty.
pub(crate) fn non_empty_var(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "attributor-mcp-config-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_or_invalid_file_yields_defaults() {
        let dir = temp_dir("defaults");
        let path = dir.join(CONFIG_FILE);
        assert_eq!(load(&path), McpConfig::default());
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(load(&path), McpConfig::default());
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = temp_dir("round-trip");
        let path = dir.join("nested").join(CONFIG_FILE);
        let cfg = McpConfig {
            enabled: true,
            port: 50000,
            token: "abc".into(),
            allow_ai_settings: true,
        };
        save(&path, &cfg).unwrap();
        assert_eq!(load(&path), cfg);
        // Unknown and missing keys fall back to defaults per field.
        std::fs::write(&path, br#"{"enabled":true,"extra":1}"#).unwrap();
        let partial = load(&path);
        assert!(partial.enabled);
        assert_eq!(partial.port, DEFAULT_PORT);
    }

    #[test]
    fn write_atomic_replaces_and_leaves_no_temp_file() {
        let dir = temp_dir("atomic");
        let path = dir.join("file.json");
        write_atomic(&path, b"one", false).unwrap();
        write_atomic(&path, b"two", false).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name() != "file.json")
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn tokens_are_random_and_url_safe() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn url_uses_the_ipv4_loopback() {
        let cfg = McpConfig {
            port: 1234,
            ..McpConfig::default()
        };
        assert_eq!(cfg.url(), "http://127.0.0.1:1234/mcp");
    }
}
