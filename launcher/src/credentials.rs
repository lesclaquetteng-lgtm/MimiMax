//! Protected storage for Microsoft session tokens.
//!
//! The account catalogue remains readable JSON, but access and refresh tokens live in a separate
//! DPAPI-protected file on Windows. This keeps credentials out of renderer IPC payloads and
//! ordinary account backups. DPAPI uses the current Windows user scope.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::accounts::AccountStore;

const HEADER: &str = "AETHER-CREDENTIALS-V1";
const MAX_CREDENTIAL_FILE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsDocument {
    version: u32,
    accounts: Vec<AccountCredentials>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountCredentials {
    account_id: String,
    access_token: String,
    refresh_token: Option<String>,
}

/// The companion file for accounts.json.
pub fn file_path(accounts_file: &Path) -> PathBuf {
    accounts_file.with_extension("credentials")
}

/// True when the in-memory store contains at least one Microsoft session secret.
pub fn has_secrets(accounts: &AccountStore) -> bool {
    accounts.accounts.iter().any(|account| {
        !account.is_offline()
            && (!account.access_token.is_empty() || account.refresh_token.is_some())
    })
}

/// Stores only the session material that is not safe to keep in the account catalogue.
pub fn save(accounts_file: &Path, accounts: &AccountStore) -> Result<(), String> {
    let entries: Vec<AccountCredentials> = accounts
        .accounts
        .iter()
        .filter(|account| !account.is_offline())
        .filter(|account| !account.access_token.is_empty() || account.refresh_token.is_some())
        .map(|account| AccountCredentials {
            account_id: account.id.clone(),
            access_token: account.access_token.clone(),
            refresh_token: account.refresh_token.clone(),
        })
        .collect();

    let path = file_path(accounts_file);
    if entries.is_empty() {
        match std::fs::remove_file(&path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(format!("could not remove old Microsoft credentials: {error}"));
            }
        }
    }

    let mut document = CredentialsDocument {
        version: 1,
        accounts: entries,
    };
    let serialized = serde_json::to_vec(&document);
    for account in &mut document.accounts {
        account.access_token.zeroize();
        if let Some(token) = &mut account.refresh_token {
            token.zeroize();
        }
    }
    let mut plaintext = serialized
        .map_err(|error| format!("could not encode Microsoft credentials: {error}"))?;
    let protected = protect(&plaintext);
    plaintext.zeroize();
    let protected = protected?;

    let content = format!("{HEADER}\n{}\n", hex_encode(&protected));
    crate::settings::write_atomic(&path, &content)
        .map_err(|error| format!("could not securely save Microsoft credentials: {error}"))
}

/// Loads protected session material into the catalogue.
/// Returns Ok(false) when no companion file exists.
pub fn load(accounts_file: &Path, accounts: &mut AccountStore) -> Result<bool, String> {
    let path = file_path(accounts_file);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("could not read protected credentials: {error}")),
    };

    if content.len() > MAX_CREDENTIAL_FILE_BYTES {
        return Err("the protected credentials file is unexpectedly large".to_string());
    }

    let (header, encoded) = content
        .trim()
        .split_once('\n')
        .ok_or_else(|| "the protected credentials file has an invalid format".to_string())?;
    if header != HEADER {
        return Err("the protected credentials file has an unsupported version".to_string());
    }

    let ciphertext = hex_decode(encoded.trim())?;
    let mut plaintext = unprotect(&ciphertext)?;
    let parsed = serde_json::from_slice::<CredentialsDocument>(&plaintext)
        .map_err(|error| format!("could not decode protected credentials: {error}"));
    plaintext.zeroize();
    let document = parsed?;

    if document.version != 1 {
        return Err(format!(
            "the protected credentials version {} is not supported",
            document.version
        ));
    }

    for saved in document.accounts {
        if let Some(account) = accounts
            .accounts
            .iter_mut()
            .find(|account| account.id == saved.account_id && !account.is_offline())
        {
            account.access_token = saved.access_token;
            account.refresh_token = saved.refresh_token;
        }
    }

    Ok(true)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    if text.is_empty() || text.len() % 2 != 0 {
        return Err("the protected credentials payload is not valid hexadecimal".to_string());
    }

    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("the protected credentials payload is not valid hexadecimal".to_string()),
    }
}

#[cfg(windows)]
fn protect(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};
    use std::slice;

    #[repr(C)]
    struct DataBlob {
        size: u32,
        data: *mut u8,
    }

    #[link(name = "Crypt32")]
    extern "system" {
        fn CryptProtectData(
            input: *const DataBlob,
            description: *const u16,
            entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    let size = u32::try_from(plaintext.len())
        .map_err(|_| "the Microsoft credentials payload is too large".to_string())?;
    let input = DataBlob {
        size,
        data: plaintext.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        size: 0,
        data: null_mut(),
    };

    let ok = unsafe {
        CryptProtectData(
            &input,
            null(),
            null(),
            null_mut(),
            null_mut(),
            0x1,
            &mut output,
        )
    };

    if ok == 0 {
        return Err(format!(
            "Windows could not protect Microsoft credentials: {}",
            std::io::Error::last_os_error()
        ));
    }

    let result = if output.size == 0 || output.data.is_null() {
        Err("Windows returned an empty protected credentials payload".to_string())
    } else {
        Ok(unsafe { slice::from_raw_parts(output.data, output.size as usize).to_vec() })
    };

    if !output.data.is_null() {
        unsafe {
            LocalFree(output.data.cast::<c_void>());
        }
    }

    result
}

#[cfg(windows)]
fn unprotect(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};
    use std::slice;

    #[repr(C)]
    struct DataBlob {
        size: u32,
        data: *mut u8,
    }

    #[link(name = "Crypt32")]
    extern "system" {
        fn CryptUnprotectData(
            input: *const DataBlob,
            description: *mut *mut u16,
            entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    let size = u32::try_from(ciphertext.len())
        .map_err(|_| "the protected Microsoft credentials payload is too large".to_string())?;
    let input = DataBlob {
        size,
        data: ciphertext.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        size: 0,
        data: null_mut(),
    };

    let ok = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            null(),
            null_mut(),
            null_mut(),
            0x1,
            &mut output,
        )
    };

    if ok == 0 {
        return Err(format!(
            "Windows could not decrypt Microsoft credentials for this user: {}",
            std::io::Error::last_os_error()
        ));
    }

    let result = if output.size == 0 || output.data.is_null() {
        Err("Windows returned an empty Microsoft credentials payload".to_string())
    } else {
        Ok(unsafe { slice::from_raw_parts(output.data, output.size as usize).to_vec() })
    };

    if !output.data.is_null() {
        unsafe {
            // CryptUnprotectData allocated a plaintext buffer. Erase it before returning it to
            // the Windows heap; the owned Vec above is the only copy that remains for parsing.
            std::ptr::write_bytes(output.data, 0, output.size as usize);
            LocalFree(output.data.cast::<c_void>());
        }
    }

    result
}

#[cfg(not(windows))]
fn protect(_plaintext: &[u8]) -> Result<Vec<u8>, String> {
    Err("secure Microsoft account storage is currently supported on Windows only".to_string())
}

#[cfg(not(windows))]
fn unprotect(_ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    Err("Microsoft credentials protected with Windows DPAPI can only be read on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::Account;

    fn microsoft_store() -> AccountStore {
        AccountStore {
            accounts: vec![Account {
                kind: "microsoft".to_string(),
                id: "account-1".to_string(),
                username: "AetherTester".to_string(),
                uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                access_token: "access-token-never-write-as-plain-text".to_string(),
                refresh_token: Some("refresh-token-never-write-as-plain-text".to_string()),
                xuid: None,
                client_id: None,
                expires_at: Some(1_900_000_000),
                profile: None,
            }],
            selected: Some("account-1".to_string()),
        }
    }

    #[test]
    fn account_catalogue_serialization_never_contains_session_tokens() {
        let store = microsoft_store();
        let json = serde_json::to_string(&store).unwrap();

        assert!(!json.contains("access-token-never-write-as-plain-text"));
        assert!(!json.contains("refresh-token-never-write-as-plain-text"));
        assert!(!json.contains("accessToken"));
        assert!(!json.contains("refreshToken"));
        assert!(json.contains("AetherTester"));
    }

    #[test]
    fn empty_store_does_not_leave_a_credentials_file_behind() {
        let root = std::env::temp_dir().join(format!("aether-credentials-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let accounts_file = root.join("accounts.json");

        save(&accounts_file, &AccountStore::default()).unwrap();
        assert!(!file_path(&accounts_file).exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_builds_refuse_to_persist_microsoft_tokens_in_clear_text() {
        let root = std::env::temp_dir().join(format!("aether-credentials-unsupported-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let accounts_file = root.join("accounts.json");

        let error = save(&accounts_file, &microsoft_store())
            .expect_err("there is no secure platform vault on this target");

        assert!(error.contains("supported on Windows only"));
        assert!(!file_path(&accounts_file).exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn hexadecimal_encoding_round_trips_and_rejects_bad_input() {
        assert_eq!(hex_encode(&[0, 1, 15, 16, 255]), "00010f10ff");
        assert_eq!(hex_decode("00010f10ff").unwrap(), vec![0, 1, 15, 16, 255]);
        assert!(hex_decode("0").is_err());
        assert!(hex_decode("zz").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_credentials_round_trip_without_writing_tokens_to_the_catalogue() {
        let root = std::env::temp_dir().join(format!("aether-credentials-dpapi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let accounts_file = root.join("accounts.json");
        let mut saved = microsoft_store();

        save(&accounts_file, &saved).unwrap();
        let text = std::fs::read_to_string(file_path(&accounts_file)).unwrap();
        assert!(!text.contains("access-token-never-write-as-plain-text"));
        assert!(!text.contains("refresh-token-never-write-as-plain-text"));

        saved.accounts[0].access_token.clear();
        saved.accounts[0].refresh_token = None;
        assert!(load(&accounts_file, &mut saved).unwrap());
        assert_eq!(saved.accounts[0].access_token, "access-token-never-write-as-plain-text");
        assert_eq!(
            saved.accounts[0].refresh_token.as_deref(),
            Some("refresh-token-never-write-as-plain-text")
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
