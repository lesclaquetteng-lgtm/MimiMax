//! The offline ("cracked") session support of the launcher.
//!
//! When a player joins a server that runs with `online-mode=false`, the server trusts the
//! profile the client sends. Both sides have to agree on the identity of the player, and
//! Minecraft solves this with a version 3 (MD5) UUID that is derived from the name:
//!
//! ```java
//! UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(StandardCharsets.UTF_8))
//! ```
//!
//! `net.minecraft.client.main.Main` applies exactly this when the launcher does not pass an
//! explicit `--uuid` argument. The launcher passes the very same value explicitly, which makes
//! the identity of the player visible in the interface and in the logs.

use md5::{Digest, Md5};
use std::fmt;

/// The error type of the offline session.
#[derive(Debug)]
pub enum OfflineError {
    /// The name of the player is not valid.
    InvalidUsername(String),
}

impl fmt::Display for OfflineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OfflineError::InvalidUsername(name) => write!(
                formatter,
                "Invalid username '{}': it must be 3 to 16 characters long and may only contain \
                 letters, digits and underscores.",
                name
            ),
        }
    }
}

impl std::error::Error for OfflineError {}

/// The prefix Minecraft uses for the identifier of an offline player.
const OFFLINE_PREFIX: &str = "OfflinePlayer:";

/// Computes the identifier Minecraft assigns to an offline player.
///
/// The result is byte for byte the same as `java.util.UUID.nameUUIDFromBytes`.
pub fn offline_player_uuid(username: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(format!("{}{}", OFFLINE_PREFIX, username).as_bytes());
    let mut digest = hasher.finalize();

    // Set the version (3) and the IETF variant bits, exactly like java.util.UUID#nameUUIDFromBytes.
    digest[6] = (digest[6] & 0x0f) | 0x30;
    digest[8] = (digest[8] & 0x3f) | 0x80;

    let hex: String = digest.iter().map(|byte| format!("{:02x}", byte)).collect();

    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Removes the dashes from an identifier, the launcher protocol expects the undashed form.
pub fn undash_uuid(uuid: &str) -> String {
    uuid.replace('-', "")
}

/// Minecraft's username rules: 3 to 16 characters, only letters, digits and underscores.
pub fn is_valid_username(username: &str) -> bool {
    let length = username.chars().count();
    if !(3..=16).contains(&length) {
        return false;
    }
    username
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Validates a username and returns the identifier of the offline player.
pub fn create_offline_account(username: &str) -> Result<(String, String), OfflineError> {
    if !is_valid_username(username) {
        return Err(OfflineError::InvalidUsername(username.to_string()));
    }
    Ok((username.to_string(), offline_player_uuid(username)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Produced with java.util.UUID.nameUUIDFromBytes("OfflinePlayer:" + name).
    const REFERENCE: [(&str, &str); 4] = [
        ("Notch", "b50ad385-829d-3141-a216-7e7d7539ba7f"),
        ("Steve", "5627dd98-e6be-3c21-b8a8-e92344183641"),
        ("Player123", "3da1e349-17b0-38ed-a07b-1e78c8ebd956"),
        ("aether_test", "192d7399-e38b-3155-afd3-d30d29bf7d22"),
    ];

    #[test]
    fn matches_the_reference_implementation_of_minecraft() {
        for (name, expected) in REFERENCE {
            assert_eq!(offline_player_uuid(name), expected, "uuid mismatch for {}", name);
        }
    }

    #[test]
    fn is_a_version_three_uuid() {
        let uuid = offline_player_uuid("Notch");
        assert_eq!(&uuid[14..15], "3", "the version nibble must be 3");
        assert!(
            ["8", "9", "a", "b"].contains(&&uuid[19..20]),
            "the variant nibble must be 8, 9, a or b"
        );
    }

    #[test]
    fn only_depends_on_the_name() {
        assert_eq!(offline_player_uuid("Steve"), offline_player_uuid("Steve"));
        assert_ne!(offline_player_uuid("Steve"), offline_player_uuid("steve"));
    }

    #[test]
    fn follows_the_username_rules() {
        assert!(is_valid_username("Notch"));
        assert!(is_valid_username("a_b_1"));
        assert!(!is_valid_username("ab"));
        assert!(!is_valid_username(&"a".repeat(17)));
        assert!(!is_valid_username("has space"));
        assert!(!is_valid_username("emoji\u{1F600}"));
    }

    #[test]
    fn removes_the_dashes() {
        assert_eq!(
            undash_uuid("b50ad385-829d-3141-a216-7e7d7539ba7f"),
            "b50ad385829d3141a2167e7d7539ba7f"
        );
    }

    #[test]
    fn rejects_invalid_names() {
        assert!(create_offline_account("ab").is_err());
        assert!(create_offline_account("Notch").is_ok());
    }
}
