//! Credential storage. API keys live in the OS credential store (Windows Credential Manager, macOS Keychain, Secret Service).
//! Larger secrets (the ChatGPT token bundle exceeds Windows' 2560-byte credential limit) are sealed with AES-256-GCM into a file whose random key is kept in the credential store.
//!
//! Every function blocks (the OS store may even show a prompt): call them inside `spawn_blocking`.
//! When the OS store is unavailable (e.g. Linux without a Secret Service), values are kept in memory for the session only.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use keyring::{Entry, Error};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};

use super::types::Provider;

/// Service name of every credential-store entry (the app identifier).
const SERVICE: &str = "loc.am.attributor";

/// File header of a sealed secret: magic, format version.
const SEALED_MAGIC: &[u8] = b"ATTRSEC1";

/// Longest API key accepted; Windows stores passwords as UTF-16 in at most 2560 bytes.
const MAX_KEY_CHARS: usize = 1024;

/// Serializes credential-store access: the Windows and Secret Service stores warn against concurrent use of one entry.
fn store_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Session-only values for when the OS credential store cannot be used.
fn memory() -> std::sync::MutexGuard<'static, HashMap<String, String>> {
    static MEMORY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    MEMORY
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Whether an error means "no usable credential store" rather than a problem with this entry.
fn store_unavailable(e: &Error) -> bool {
    matches!(
        e,
        Error::NoDefaultStore
            | Error::PlatformFailure(_)
            | Error::NoStorageAccess(_)
            | Error::NotSupportedByStore(_)
    )
}

/// Reads entry `name`; `None` when it was never set.
fn get(name: &str) -> Result<Option<String>, String> {
    let _guard = store_lock();
    if let Some(value) = memory().get(name) {
        return Ok(Some(value.clone()));
    }
    match Entry::new(SERVICE, name).and_then(|e| e.get_password()) {
        Ok(value) => Ok(Some(value)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) if store_unavailable(&e) => {
            log::warn!("credential store unavailable while reading {name}: {e}");
            Ok(None)
        }
        Err(e) => Err(format!("credential store error: {e}")),
    }
}

/// Writes entry `name`, falling back to session memory when the OS store is unavailable.
fn set(name: &str, value: &str) -> Result<(), String> {
    let _guard = store_lock();
    match Entry::new(SERVICE, name).and_then(|e| e.set_password(value)) {
        Ok(()) => {
            memory().remove(name);
            Ok(())
        }
        Err(e) if store_unavailable(&e) => {
            log::warn!("credential store unavailable, keeping {name} for this session only: {e}");
            memory().insert(name.to_string(), value.to_string());
            Ok(())
        }
        Err(e) => Err(format!("credential store error: {e}")),
    }
}

/// Removes entry `name`; removing a missing entry succeeds.
fn delete(name: &str) -> Result<(), String> {
    let _guard = store_lock();
    memory().remove(name);
    match Entry::new(SERVICE, name).and_then(|e| e.delete_credential()) {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) if store_unavailable(&e) => Ok(()),
        Err(e) => Err(format!("credential store error: {e}")),
    }
}

fn api_key_entry(provider: Provider) -> String {
    format!("api-key:{}", provider.id())
}

/// The stored API key of `provider`.
///
/// # Errors
///
/// Returns an error when the credential store fails for a reason other than being unavailable.
pub fn api_key(provider: Provider) -> Result<Option<String>, String> {
    get(&api_key_entry(provider))
}

/// [`api_key`] run off the async runtime.
///
/// # Errors
///
/// Returns an error when the credential store fails for a reason other than being unavailable.
pub async fn load_api_key(provider: Provider) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || api_key(provider))
        .await
        .map_err(|e| e.to_string())?
}

/// The optional key of a local server: a failing credential store is logged and treated as "no key", so it never blocks local inference.
pub async fn optional_api_key(provider: Provider) -> Option<String> {
    load_api_key(provider).await.unwrap_or_else(|e| {
        log::warn!("{} API key unavailable: {e}", provider.label());
        None
    })
}

/// Stores `key` as the API key of `provider`; an empty key removes the stored one.
///
/// # Errors
///
/// Returns an error for keys longer than 1024 characters or a failing credential store.
pub fn set_api_key(provider: Provider, key: &str) -> Result<(), String> {
    if key.is_empty() {
        return delete(&api_key_entry(provider));
    }
    if key.chars().count() > MAX_KEY_CHARS {
        return Err(format!("the key is longer than {MAX_KEY_CHARS} characters"));
    }
    set(&api_key_entry(provider), key)
}

/// The AES-256 key protecting the sealed file `name`, created on first use.
fn sealing_key(name: &str, create: bool) -> Result<Option<LessSafeKey>, String> {
    let entry = format!("vault-key:{name}");
    let raw = match get(&entry)? {
        Some(encoded) => STANDARD
            .decode(encoded)
            .map_err(|e| format!("corrupt vault key: {e}"))?,
        None if create => {
            let mut raw = vec![0u8; AES_256_GCM.key_len()];
            SystemRandom::new()
                .fill(&mut raw)
                .map_err(|_| "no secure random source".to_string())?;
            set(&entry, &STANDARD.encode(&raw))?;
            raw
        }
        None => return Ok(None),
    };
    let key =
        UnboundKey::new(&AES_256_GCM, &raw).map_err(|_| "invalid vault key length".to_string())?;
    Ok(Some(LessSafeKey::new(key)))
}

/// Encrypts `plaintext` with `key`: `MAGIC | nonce | ciphertext+tag`, with `aad` bound into the tag.
fn seal(key: &LessSafeKey, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| "no secure random source".to_string())?;
    let mut sealed = plaintext.to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(aad),
        &mut sealed,
    )
    .map_err(|_| "encryption failed".to_string())?;
    Ok([SEALED_MAGIC, &nonce[..], &sealed].concat())
}

/// Reverses [`seal`]; `None` when the data is not a sealed file or fails authentication (wrong key, tampering).
fn open(key: &LessSafeKey, aad: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    let rest = data.strip_prefix(SEALED_MAGIC)?;
    let (nonce, ciphertext) = rest.split_at_checked(NONCE_LEN)?;
    let nonce = Nonce::assume_unique_for_key(nonce.try_into().ok()?);
    let mut buf = ciphertext.to_vec();
    let plain = key.open_in_place(nonce, Aad::from(aad), &mut buf).ok()?;
    Some(plain.to_vec())
}

/// Seals `plaintext` into `<dir>/<name>.bin`, written atomically (temp file, then rename).
///
/// # Errors
///
/// Returns an error when the credential store, the random source or the file system fails.
pub fn write_sealed(dir: &Path, name: &str, plaintext: &[u8]) -> Result<(), String> {
    let key = sealing_key(name, true)?.ok_or("vault key unavailable")?;
    let data = seal(&key, name.as_bytes(), plaintext)?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{name}.bin"));
    let tmp = dir.join(format!(".{name}.bin.tmp"));
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    restrict_permissions(&tmp);
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Reads and decrypts `<dir>/<name>.bin`. A missing, undecryptable or corrupt file reads as `None`, so the caller treats it as signed out.
///
/// # Errors
///
/// Returns an error only when the credential store fails.
pub fn read_sealed(dir: &Path, name: &str) -> Result<Option<Vec<u8>>, String> {
    let Ok(data) = std::fs::read(dir.join(format!("{name}.bin"))) else {
        return Ok(None);
    };
    let Some(key) = sealing_key(name, false)? else {
        return Ok(None);
    };
    let plain = open(&key, name.as_bytes(), &data);
    if plain.is_none() {
        log::warn!("sealed secret {name} could not be decrypted; ignoring it");
    }
    Ok(plain)
}

/// Deletes `<dir>/<name>.bin` and its key.
///
/// # Errors
///
/// Returns an error when the file exists but cannot be removed, or the credential store fails.
pub fn delete_sealed(dir: &Path, name: &str) -> Result<(), String> {
    match std::fs::remove_file(dir.join(format!("{name}.bin"))) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    delete(&format!("vault-key:{name}"))
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        log::warn!("could not restrict permissions of {}: {e}", path.display());
    }
}

/// On Windows the per-user profile directory is already private to the user.
#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> LessSafeKey {
        LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &[7u8; 32]).unwrap())
    }

    #[test]
    fn seal_round_trips_and_detects_tampering() {
        let key = test_key();
        let sealed = seal(&key, b"chatgpt", b"secret tokens").unwrap();
        assert!(sealed.starts_with(SEALED_MAGIC));
        assert_eq!(open(&key, b"chatgpt", &sealed).unwrap(), b"secret tokens");

        let mut tampered = sealed.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(open(&key, b"chatgpt", &tampered).is_none());
        assert!(
            open(&key, b"other", &sealed).is_none(),
            "the file name is bound into the tag"
        );
        assert!(open(&key, b"chatgpt", b"garbage").is_none());
    }

    #[test]
    fn seal_uses_a_fresh_nonce_each_time() {
        let key = test_key();
        assert_ne!(
            seal(&key, b"a", b"x").unwrap(),
            seal(&key, b"a", b"x").unwrap()
        );
    }

    #[test]
    fn overlong_keys_are_rejected_before_touching_the_store() {
        let long = "k".repeat(MAX_KEY_CHARS + 1);
        assert!(set_api_key(Provider::Openai, &long).is_err());
    }
}
