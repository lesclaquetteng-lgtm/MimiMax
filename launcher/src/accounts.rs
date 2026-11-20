//! The accounts of the player: offline accounts and Microsoft accounts.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use crate::http::{post_form, post_json, HttpError};
use crate::offline;

/// The public client of the Minecraft launcher, it is used by every third party launcher.
pub const MICROSOFT_CLIENT_ID: &str = "00000000402b5328";

const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// The grant type of the device code flow.
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// How long the launcher waits for the sign in when Microsoft does not say it.
const DEFAULT_DEVICE_CODE_LIFETIME: u64 = 900;
/// How much the polling interval grows when Microsoft asks the client to slow down.
const SLOW_DOWN_STEP: u64 = 5;
/// The number of seconds that are left on the token when the session is renewed.
const REFRESH_MARGIN_SECS: u64 = 300;
/// The grant type that renews a session with the refresh token.
const REFRESH_TOKEN_GRANT: &str = "refresh_token";

/// An account of the player.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// `offline` or `microsoft`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The unique identifier of the account.
    pub id: String,
    /// The name of the player.
    pub username: String,
    /// The identifier of the player.
    pub uuid: String,
    /// The token of the session, held only in memory or in the DPAPI vault.
    /// It is deliberately omitted from account JSON and renderer IPC responses.
    #[serde(default, skip_serializing)]
    pub access_token: String,
    /// The token that refreshes a Microsoft session, stored in the DPAPI vault.
    #[serde(default, skip_serializing)]
    pub refresh_token: Option<String>,
    /// The identifier of the player on Xbox live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xuid: Option<String>,
    /// The identifier of the client of a Microsoft account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// The moment the token of a Microsoft account expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// The profile this account plays with, `None` follows the active profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Account")
            .field("kind", &self.kind)
            .field("id", &self.id)
            .field("username", &self.username)
            .field("uuid", &self.uuid)
            .field("access_token", &if self.access_token.is_empty() { "" } else { "<redacted>" })
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("xuid", &self.xuid)
            .field("client_id", &self.client_id)
            .field("expires_at", &self.expires_at)
            .field("profile", &self.profile)
            .finish()
    }
}

impl Account {
    /// @return true when the account is an offline one.
    pub fn is_offline(&self) -> bool {
        self.kind == "offline"
    }
}

/// The store of the accounts.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub selected: Option<String>,
}

impl AccountStore {
    /// Creates an offline account.
    pub fn add_offline(&mut self, username: &str) -> Result<Account, offline::OfflineError> {
        let (username, uuid) = offline::create_offline_account(username)?;

        let account = Account {
            kind: "offline".to_string(),
            id: new_id(),
            username,
            uuid,
            access_token: "0".to_string(),
            refresh_token: None,
            xuid: None,
            client_id: None,
            expires_at: None,
            profile: None,
        };

        self.upsert(account.clone());
        Ok(account)
    }

    /// Chooses the profile of an account, `None` gives it back to the active profile.
    pub fn set_profile(&mut self, id: &str, profile: Option<&str>) -> Option<Account> {
        let account = self.accounts.iter_mut().find(|account| account.id == id)?;
        account.profile = profile.map(|name| name.to_string());
        Some(account.clone())
    }

    /// Adds an account, an account with the same name is replaced.
    ///
    /// The identifier of the replaced account is kept, so that the selection of the player stays
    /// valid.
    pub fn upsert(&mut self, account: Account) {
        if let Some(position) = self
            .accounts
            .iter()
            .position(|candidate| candidate.username.eq_ignore_ascii_case(&account.username))
        {
            let id = self.accounts[position].id.clone();
            let mut replacement = account;
            replacement.id = id;
            self.accounts[position] = replacement;
        } else {
            self.accounts.push(account);
        }
        self.selected = Some(self.accounts.last().map(|entry| entry.id.clone()).unwrap());
    }

    /// Removes an account.
    pub fn remove(&mut self, id: &str) {
        self.accounts.retain(|account| account.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = self.accounts.first().map(|account| account.id.clone());
        }
    }

    /// Selects the active account.
    pub fn select(&mut self, id: &str) {
        self.selected = Some(id.to_string());
    }

    /// Finds an account by its identifier.
    pub fn find(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    /// Replaces an account by its identifier, the selection of the player is kept.
    pub fn replace(&mut self, account: Account) {
        if let Some(position) = self.accounts.iter().position(|entry| entry.id == account.id) {
            self.accounts[position] = account;
        }
    }
}

/// The session that is used to start the game.
#[derive(Clone)]
pub struct Session {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub offline: bool,
    pub xuid: String,
    pub client_id: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("username", &self.username)
            .field("uuid", &self.uuid)
            .field("access_token", &if self.access_token == "0" { "0" } else { "<redacted>" })
            .field("offline", &self.offline)
            .field("xuid", &self.xuid)
            .field("client_id", &self.client_id)
            .finish()
    }
}

impl From<&Account> for Session {
    fn from(account: &Account) -> Self {
        Session {
            username: account.username.clone(),
            uuid: account.uuid.clone(),
            access_token: if account.is_offline() {
                "0".to_string()
            } else {
                account.access_token.clone()
            },
            offline: account.is_offline(),
            xuid: account.xuid.clone().unwrap_or_default(),
            client_id: account.client_id.clone().unwrap_or_default(),
        }
    }
}

/// The endpoint that tells whether a Mojang account owns a name.
const NAME_LOOKUP_URL: &str = "https://api.mojang.com/users/profiles/minecraft/";

/// What Mojang knows about a name.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameStatus {
    /// The name that was asked, as the launcher wrote it.
    pub name: String,
    /// True when a Mojang account owns the name.
    pub premium: bool,
}

impl NameStatus {
    /// @return true when the name is owned by a Mojang account.
    ///
    /// A cracked server almost always protects the premium names: it refuses a client that
    /// presents a name Mojang owns, because that would be an impersonation of its owner. The
    /// player has to pick another name; no client can talk its way past that rule.
    pub fn is_taken_by_a_mojang_account(&self) -> bool {
        self.premium
    }
}

/// Asks Mojang whether an account owns a name.
///
/// The answer arrives before the game starts, which is the whole point: in game, such a server
/// only says `Unauthorized: You cannot join the server`, with no hint about the name.
pub fn name_status(name: &str) -> Result<NameStatus, HttpError> {
    let request = format!("{}{}", NAME_LOOKUP_URL, name);
    let (status, body) = crate::http::get_with_status(&request)?;
    read_name_answer(&request, name, status, &body)
}

/// Reads the answer of the name lookup.
///
/// `200` carries the profile of the owner, `204` and `404` mean nobody owns the name. Anything
/// else is an error, the caller must not read it as "the name is free".
fn read_name_answer(
    request: &str,
    name: &str,
    status: u16,
    body: &str,
) -> Result<NameStatus, HttpError> {
    match status {
        200 => {
            #[derive(Deserialize)]
            struct Profile {
                name: String,
            }

            let profile: Profile = serde_json::from_str(body)?;

            Ok(NameStatus {
                name: profile.name,
                premium: true,
            })
        }
        204 | 404 => Ok(NameStatus {
            name: name.to_string(),
            premium: false,
        }),
        other => Err(HttpError::Status {
            url: request.to_string(),
            status: other,
            body: Some(body.to_string()),
        }),
    }
}

/// The answer of the device code endpoint.
#[derive(Clone, Debug, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub interval: u64,
    #[serde(default)]
    pub expires_in: u64,
}

/// The answer of the token endpoint.
#[derive(Clone, Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// The error type of the Microsoft sign in.
#[derive(Debug)]
pub enum LoginError {
    /// The device code expired before the player finished the sign in.
    Expired,
    /// The player refused the sign in.
    Declined(String),
    /// Microsoft answered with an OAuth error that ends the flow.
    OAuth { code: String, description: String },
    /// The request itself failed.
    Http(HttpError),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::Expired => write!(
                formatter,
                "The Microsoft sign in expired before it was completed, start it again."
            ),
            LoginError::Declined(description) => {
                if description.is_empty() {
                    write!(formatter, "The Microsoft sign in was declined.")
                } else {
                    write!(formatter, "The Microsoft sign in was declined: {}", description)
                }
            }
            LoginError::OAuth { code, description } => write!(
                formatter,
                "Microsoft answered with the OAuth error '{}': {}",
                code, description
            ),
            LoginError::Http(error) => write!(formatter, "{}", error),
        }
    }
}

impl std::error::Error for LoginError {}

impl From<HttpError> for LoginError {
    fn from(error: HttpError) -> Self {
        LoginError::Http(error)
    }
}

/// Starts the sign in with Microsoft.
pub fn start_device_code() -> Result<DeviceCode, HttpError> {
    post_form(
        DEVICE_CODE_URL,
        &[
            ("client_id", MICROSOFT_CLIENT_ID),
            (
                "scope",
                "service::user.auth.xboxlive.com::MBI_SSL offline_access",
            ),
        ],
    )
}

/// Waits until the player finished the sign in.
///
/// The endpoint answers with an OAuth error code while the player has not finished yet:
///
/// * `authorization_pending` – the player did not finish, poll again after the interval.
/// * `slow_down` – the client polls too fast, the interval has to grow.
/// * `authorization_declined` / `access_denied` – the player refused, the flow ends.
/// * `expired_token` / `code_expired` – the code is no longer valid, the flow ends.
///
/// Every other code is reported to the caller instead of being treated as "still waiting".
pub fn poll_device_code(code: &DeviceCode) -> Result<TokenResponse, LoginError> {
    let lifetime = if code.expires_in > 0 {
        code.expires_in
    } else {
        DEFAULT_DEVICE_CODE_LIFETIME
    };
    let deadline = Instant::now() + Duration::from_secs(lifetime);
    let mut interval = code.interval.max(1);

    loop {
        if Instant::now() >= deadline {
            return Err(LoginError::Expired);
        }

        let answer = post_form::<TokenResponse>(
            TOKEN_URL,
            &[
                ("client_id", MICROSOFT_CLIENT_ID),
                ("grant_type", DEVICE_CODE_GRANT),
                ("device_code", &code.device_code),
            ],
        );

        match answer {
            Ok(token) => return Ok(token),
            Err(error) => {
                if let Some((name, description)) = oauth_error(&error) {
                    match name.as_str() {
                        "authorization_pending" => {}
                        "slow_down" => interval = interval.saturating_add(SLOW_DOWN_STEP),
                        "authorization_declined" | "access_denied" => {
                            return Err(LoginError::Declined(description));
                        }
                        "expired_token" | "code_expired" => return Err(LoginError::Expired),
                        _ => return Err(LoginError::OAuth { code: name, description }),
                    }
                } else if !is_pending_status(&error) {
                    return Err(LoginError::Http(error));
                }
            }
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(LoginError::Expired);
        }
        std::thread::sleep(Duration::from_secs(interval).min(remaining));
    }
}

/// Turns an HTTP failure of the sign in into the error the caller understands.
///
/// The token endpoint answers with an OAuth document, the code has to survive the conversion:
/// `invalid_grant` is how Microsoft reports a refresh token that was revoked or that expired.
fn login_error_from(error: HttpError) -> LoginError {
    match oauth_error(&error) {
        Some((code, description)) => LoginError::OAuth { code, description },
        None => LoginError::Http(error),
    }
}

/// Reads the OAuth error code and description of an answer.
fn oauth_error(error: &HttpError) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(error.body()?).ok()?;
    let code = value.get("error")?.as_str()?.to_string();
    let description = value
        .get("error_description")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    Some((code, description))
}

/// @return true when the answer only means that the player has not finished yet.
///
/// Some answers of the endpoint carry no body, the status code is then the only information.
fn is_pending_status(error: &HttpError) -> bool {
    matches!(
        error,
        HttpError::Status { status: 400 | 429, .. }
    )
}

/// The map of a device-flow error code to the wording used in the interface.
pub fn describe_device_flow_error(code: &str) -> &'static str {
    match code {
        "authorization_pending" => "The sign in has not been confirmed yet.",
        "slow_down" => "Microsoft asked the launcher to poll less often.",
        "authorization_declined" | "access_denied" => "The sign in was declined.",
        "expired_token" | "code_expired" => "The code expired, start the sign in again.",
        _ => "Microsoft returned an unexpected error.",
    }
}

/// The answer of the Xbox live endpoint.
#[derive(Clone, Debug, Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

/// The claims of the Xbox live answer.
#[derive(Clone, Debug, Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

/// A single claim.
#[derive(Clone, Debug, Deserialize)]
struct Xui {
    uhs: String,
}

/// The answer of the XSTS endpoint.
#[derive(Clone, Debug, Deserialize)]
struct XstsResponse {
    #[serde(rename = "Token")]
    token: String,
}

/// The answer of the Minecraft login endpoint.
#[derive(Clone, Debug, Deserialize)]
struct MinecraftLogin {
    access_token: String,
    expires_in: Option<u64>,
}

/// The profile of the player.
#[derive(Clone, Debug, Deserialize)]
struct MinecraftProfile {
    id: String,
    name: String,
}

/// Finishes the sign in and returns the Minecraft account.
///
/// The token of Microsoft walks the Xbox live, XSTS and Minecraft endpoints. The refresh token is
/// kept with the account so that the session can be renewed later without the player.
pub fn complete_microsoft_login(token: &TokenResponse) -> Result<Account, HttpError> {
    let microsoft_token = &token.access_token;

    // 1. Xbox live
    let xbl = post_json::<XblResponse>(
        XBL_URL,
        &serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", microsoft_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )?;

    // 2. XSTS
    let xsts = post_json::<XstsResponse>(
        XSTS_URL,
        &serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl.token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )?;

    // 3. Minecraft
    //
    // `DisplayClaims.xui` is a list and Microsoft has been seen to send it empty, for an account
    // without a gamertag or when a proxy rewrites the answer. Indexing it directly aborted the
    // process (`panic = "abort"`), so the missing claim is reported as a normal error.
    let user_hash = xbl
        .display_claims
        .xui
        .first()
        .map(|claim| claim.uhs.trim().to_string())
        .filter(|uhs| !uhs.is_empty())
        .ok_or_else(|| {
            HttpError::Json(
                "Microsoft did not return an Xbox user hash: DisplayClaims.xui is empty".to_string(),
            )
        })?;

    let identity = format!("XBL3.0 x={};{}", xsts.token, user_hash);
    let minecraft = post_json::<MinecraftLogin>(
        MC_LOGIN_URL,
        &serde_json::json!({ "identityToken": identity }),
    )?;

    // 4. The profile
    let profile: MinecraftProfile = {
        let request = ureq::AgentBuilder::new().build();
        request
            .get(MC_PROFILE_URL)
            .set("authorization", &format!("Bearer {}", minecraft.access_token))
            .call()
            .map_err(|error| HttpError::Io(std::io::Error::other(error.to_string())))
            .and_then(|response| {
                response
                    .into_json()
                    .map_err(|error| HttpError::Json(error.to_string()))
            })
    }?;

    Ok(Account {
        kind: "microsoft".to_string(),
        id: new_id(),
        username: profile.name,
        uuid: profile.id,
        access_token: minecraft.access_token,
        refresh_token: token.refresh_token.clone(),
        xuid: Some(user_hash),
        client_id: Some(String::new()),
        profile: None,
        expires_at: Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0)
                + minecraft.expires_in.unwrap_or(86400),
        ),
    })
}

/// Renews the session of a Microsoft account.
///
/// The refresh token is exchanged for a new access token, which is then turned into a fresh
/// Minecraft session. The identifier of the account is kept, so the selection stays valid.
pub fn refresh_account(account: &Account) -> Result<Account, LoginError> {
    let refresh_token = account.refresh_token.clone().ok_or(LoginError::Expired)?;

    let token: TokenResponse = post_form(
        TOKEN_URL,
        &[
            ("client_id", MICROSOFT_CLIENT_ID),
            ("grant_type", REFRESH_TOKEN_GRANT),
            ("refresh_token", &refresh_token),
            ("scope", "service::user.auth.xboxlive.com::MBI_SSL offline_access"),
        ],
    )
    .map_err(login_error_from)?;

    let mut refreshed = complete_microsoft_login(&token)?;
    refreshed.id = account.id.clone();

    // Microsoft does not always send a new refresh token, the previous one stays valid.
    if refreshed.refresh_token.is_none() {
        refreshed.refresh_token = Some(refresh_token);
    }

    Ok(refreshed)
}

/// @return true when the session of the account has to be renewed before it is used.
///
/// An offline account never expires. A Microsoft account is renewed a few minutes before the token
/// of the game becomes invalid, and an account that carries no expiry is renewed right away.
pub fn needs_refresh(account: &Account) -> bool {
    if account.is_offline() {
        return false;
    }

    match account.expires_at {
        Some(expires_at) => expires_at <= now_seconds().saturating_add(REFRESH_MARGIN_SECS),
        None => true,
    }
}

/// The number of seconds since the Unix epoch.
fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Creates a random identifier.
///
/// The seeds of `RandomState` come from the random source of the operating system, they are used
/// here instead of pulling a dependency for sixteen bytes. The clock and the identifier of the
/// process are mixed in, and the result keeps the shape of a version 4 uuid like the identifiers
/// that are already stored.
fn new_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(std::process::id() as u64);
        hasher.write_u128(now_seconds() as u128);
        chunk.copy_from_slice(&hasher.finish().to_be_bytes());
    }

    // The bits of the version and of the variant of a version 4 uuid.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let hex = crate::http::hex_digest(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_an_offline_account_with_the_identifier_of_minecraft() {
        let mut store = AccountStore::default();
        let account = store.add_offline("Notch").unwrap();
        assert_eq!(account.uuid, "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert!(account.is_offline());
        assert_eq!(store.selected.as_deref(), Some(account.id.as_str()));
    }

    #[test]
    fn debug_output_never_exposes_microsoft_session_tokens() {
        let mut store = AccountStore::default();
        let mut account = store.add_offline("Notch").unwrap();
        account.kind = "microsoft".to_string();
        account.access_token = "secret-access-token".to_string();
        account.refresh_token = Some("secret-refresh-token".to_string());

        let account_debug = format!("{account:?}");
        assert!(!account_debug.contains("secret-access-token"));
        assert!(!account_debug.contains("secret-refresh-token"));
        assert!(account_debug.contains("<redacted>"));

        let session = Session::from(&account);
        let session_debug = format!("{session:?}");
        assert!(!session_debug.contains("secret-access-token"));
        assert!(session_debug.contains("<redacted>"));
    }

    #[test]
    fn the_identifiers_are_distinct_and_shaped_like_a_uuid() {
        let mut seen = std::collections::HashSet::new();

        for _ in 0..1000 {
            let id = new_id();

            assert_eq!(id.len(), 36, "{}", id);
            assert!(
                id.chars().all(|value| value.is_ascii_hexdigit() || value == '-'),
                "{}",
                id
            );
            assert_eq!(&id[14..15], "4", "the uuid is not a version 4: {}", id);
            assert!(
                matches!(&id[19..20], "8" | "9" | "a" | "b"),
                "the variant of {} is not the RFC one",
                id
            );
            assert!(seen.insert(id), "an identifier was repeated");
        }
    }

    #[test]
    fn replaces_an_account_with_the_same_name() {
        let mut store = AccountStore::default();
        store.add_offline("Notch").unwrap();
        store.add_offline("Notch").unwrap();
        assert_eq!(store.accounts.len(), 1);
    }

    #[test]
    fn removes_an_account() {
        let mut store = AccountStore::default();
        let account = store.add_offline("Notch").unwrap();
        store.remove(&account.id);
        assert!(store.accounts.is_empty());
        assert!(store.selected.is_none());
    }

    fn status(status: u16, body: &str) -> HttpError {
        HttpError::Status {
            url: "https://login.microsoftonline.com/consumers/oauth2/v2.0/token".to_string(),
            status,
            body: Some(body.to_string()),
        }
    }

    #[test]
    fn reads_the_oauth_error_of_a_pending_sign_in() {
        let error = status(
            400,
            "{\"error\":\"authorization_pending\",\"error_description\":\"AADSTS70016\"}",
        );
        let (code, description) = oauth_error(&error).unwrap();

        assert_eq!(code, "authorization_pending");
        assert_eq!(description, "AADSTS70016");
        assert!(is_pending_status(&error));
    }

    #[test]
    fn a_refused_sign_in_is_terminal() {
        let error = status(
            400,
            "{\"error\":\"authorization_declined\",\"error_description\":\"declined by the player\"}",
        );
        let (code, description) = oauth_error(&error).unwrap();

        assert_eq!(code, "authorization_declined");
        assert_eq!(description, "declined by the player");
        assert_eq!(
            describe_device_flow_error(&code),
            "The sign in was declined."
        );
    }

    #[test]
    fn a_rejected_refresh_token_is_an_oauth_error() {
        let error = status(
            400,
            "{\"error\":\"invalid_grant\",\"error_description\":\"AADSTS70008: The refresh token expired.\"}",
        );

        match login_error_from(error) {
            LoginError::OAuth { code, description } => {
                assert_eq!(code, "invalid_grant");
                assert!(description.starts_with("AADSTS70008"), "{}", description);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn only_an_expired_microsoft_session_is_refreshed() {
        let mut account = Account {
            kind: "microsoft".to_string(),
            id: "id".to_string(),
            username: "Notch".to_string(),
            uuid: "uuid".to_string(),
            access_token: "token".to_string(),
            refresh_token: Some("refresh".to_string()),
            xuid: None,
            client_id: None,
            expires_at: Some(now_seconds() + 3600),
            profile: None,
        };

        assert!(!needs_refresh(&account), "a valid token was renewed");

        account.expires_at = Some(now_seconds() - 1);
        assert!(needs_refresh(&account), "an expired token was kept");

        account.expires_at = None;
        assert!(needs_refresh(&account), "a token without expiry was kept");

        account.kind = "offline".to_string();
        assert!(!needs_refresh(&account), "an offline account was renewed");
    }

    #[test]
    fn a_refreshed_account_keeps_its_identifier() {
        let mut store = AccountStore::default();
        let mut account = store.add_offline("Notch").unwrap();
        account.username = "Notch2".to_string();

        let mut replacement = account.clone();
        replacement.access_token = "a new token".to_string();
        store.replace(replacement);

        assert_eq!(store.accounts.len(), 1);
        assert_eq!(store.accounts[0].access_token, "a new token");
        assert_eq!(store.selected.as_deref(), Some(account.id.as_str()));
    }

    #[test]
    fn a_name_owned_by_a_mojang_account_is_read_as_premium() {
        let answer = r#"{"id":"069a79f444e94726a5befca90e38aaf5","name":"Notch"}"#;
        let status = read_name_answer("https://example.invalid/Notch", "notch", 200, answer)
            .expect("a profile is a valid answer");

        assert!(status.premium);
        assert_eq!(status.name, "Notch", "the name of the owner was lost");
        assert!(status.is_taken_by_a_mojang_account());
    }

    #[test]
    fn a_name_nobody_owns_is_not_premium() {
        for status in [204, 404] {
            let answer = read_name_answer("https://example.invalid/Somebody", "Somebody", status, "")
                .expect("an empty answer is a valid answer");

            assert!(!answer.premium, "a free name was read as premium on {status}");
            assert_eq!(answer.name, "Somebody");
        }
    }

    #[test]
    fn a_broken_lookup_is_never_read_as_a_free_name() {
        let error = read_name_answer("https://example.invalid/Notch", "Notch", 429, "")
            .expect_err("a rate limit is not an answer about the name");

        assert!(error.is_retryable(), "a rate limit has to be retried");
        assert_eq!(error.body(), Some(""));
    }

    #[test]
    fn an_answer_without_body_is_only_pending_on_400() {
        let pending = HttpError::Status {
            url: "https://example.invalid".to_string(),
            status: 400,
            body: None,
        };
        let missing = HttpError::Status {
            url: "https://example.invalid".to_string(),
            status: 404,
            body: None,
        };

        assert!(is_pending_status(&pending));
        assert!(!is_pending_status(&missing));
        assert!(oauth_error(&pending).is_none());
    }
}
