use std::env;
use std::fs;
use std::path::PathBuf;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::hashing::sha256_hex;

type HmacSha256 = Hmac<Sha256>;

pub fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

pub const SIGNING_KEY_FILENAME: &str = "signing.key";

pub fn key_path() -> Result<PathBuf, String> {
    if let Some(appdata) = env::var_os("APPDATA") {
        let dir = PathBuf::from(appdata).join("Purgent");
        return Ok(dir.join(SIGNING_KEY_FILENAME));
    }
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME") {
        let dir = PathBuf::from(xdg).join("purgent");
        return Ok(dir.join(SIGNING_KEY_FILENAME));
    }
    if let Some(home) = env::var_os("HOME") {
        let dir = PathBuf::from(home).join(".config").join("purgent");
        return Ok(dir.join(SIGNING_KEY_FILENAME));
    }
    Err("cannot locate a user config directory for the signing key".to_string())
}

fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| format!("entropy source failed: {e:?}"))?;
    Ok(buf)
}

pub fn load_or_create_signing_key() -> Result<Vec<u8>, String> {
    let path = key_path()?;
    if path.exists() {
        return fs::read(&path).map_err(|e| format!("cannot read signing key: {e}"));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create keystore dir: {e}"))?;
    }
    let key = random_bytes(32)?;
    fs::write(&path, &key).map_err(|e| format!("cannot write signing key: {e}"))?;
    Ok(key)
}

pub fn identity_fingerprint(key: &[u8]) -> String {
    sha256_hex(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc4231_test_case_1() {
        let key = &[0x0b; 20];
        let data = b"Hi There";
        let expected = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
        assert_eq!(hmac_sha256_hex(key, data), expected);
    }

    #[test]
    fn generic_test_key_matches_sha256_of_key() {
        let key = b"01234567890123456789012345678901";
        assert_eq!(identity_fingerprint(key), sha256_hex(key));
    }

    #[test]
    fn key_file_is_created_once_and_reloaded_identically() {
        let dir = std::env::temp_dir().join(format!("purgent-signing-{}", uuid::Uuid::new_v4()));
        std::env::set_var("APPDATA", &dir);
        let a = load_or_create_signing_key().unwrap();
        let b = load_or_create_signing_key().unwrap();
        assert_eq!(a, b, "key must be stable across loads");
        assert_eq!(a.len(), 32);
        let path = key_path().unwrap();
        assert!(path.exists(), "key must be persisted");
        std::env::remove_var("APPDATA");
        let _ = fs::remove_dir_all(&dir);
    }
}
