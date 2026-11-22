//! The Aether launcher.
//!
//! The launcher installs Minecraft with the Fabric loader and the Aether client and starts the game
//! with an offline account or a Microsoft account.

pub mod accounts;
pub mod credentials;
pub mod game;
pub mod hosting;
pub mod http;
pub mod installer;
pub mod logs;
pub mod minecraft;
pub mod mods;
pub mod offline;
pub mod paths;
pub mod pending;
pub mod performance;
pub mod servers;
pub mod settings;

use std::path::PathBuf;

use accounts::{Account, AccountStore, Session};
use game::LaunchOptions;
use installer::InstallProgress;
use minecraft::{
    fetch_fabric_loaders, fetch_fabric_profile, fetch_manifest, fetch_version_json,
    merge_version_json, released_versions, VersionEntry,
};
use paths::Paths;
use settings::{detect_java, find_client_jar};
use zeroize::Zeroize;

#[cfg(test)]
pub mod test_server;

pub use settings::Settings;

/// Turns the error of a sign in into a message for the player.
fn login_error(error: accounts::LoginError) -> String {
    match &error {
        accounts::LoginError::OAuth { code, .. } if code == "invalid_grant" => {
            "the Microsoft session expired, sign in again with this account".to_string()
        }
        _ => error.to_string(),
    }
}

/// Drops the sessions that cannot be protected, then rewrites the catalogue without them.
///
/// A vault that cannot be written or read must never leave the tokens in clear text on disk: the
/// account stays, the player signs in again.
fn drop_secrets(accounts_file: &std::path::Path, accounts: &mut AccountStore) {
    for account in &mut accounts.accounts {
        account.access_token.zeroize();
        if let Some(token) = &mut account.refresh_token {
            token.zeroize();
        }
        account.refresh_token = None;
    }

    if let Err(error) = write_account_catalogue(accounts_file, accounts) {
        eprintln!("Aether: could not rewrite the account catalogue: {error}");
    }
}

/// Moves the tokens of a legacy catalogue into the vault, then rewrites the catalogue without them.
///
/// The protected file is written first: an interruption leaves the tokens recoverable there, and
/// the next start scrubs the catalogue as soon as the vault loads.
pub(crate) fn migrate_legacy_secrets(
    accounts_file: &std::path::Path,
    accounts: &mut AccountStore,
) -> Result<(), String> {
    credentials::save(accounts_file, accounts)?;
    write_account_catalogue(accounts_file, accounts)
}

/// Writes the non-secret account catalogue. The Account serializer intentionally omits session tokens.
fn write_account_catalogue(path: &std::path::Path, accounts: &AccountStore) -> Result<(), String> {
    let content = serde_json::to_string_pretty(accounts)
        .map_err(|error| format!("could not encode the account catalogue: {error}"))?;
    settings::write_atomic(path, &content)
        .map_err(|error| format!("could not save the account catalogue: {error}"))
}

/// The launcher, it owns the settings, the accounts and the game.
pub struct Launcher {
    pub paths: Paths,
    pub settings: Settings,
    pub accounts: AccountStore,
    // A corrupt or foreign-user DPAPI blob must not be silently overwritten by an unrelated
    // settings change. A new Microsoft login can replace it with a fresh protected vault.
    secrets_unavailable: bool,
}

impl Launcher {
    /// Opens the launcher and loads the settings and the accounts.
    pub fn open() -> Launcher {
        let paths = Paths::resolve();
        paths.ensure().ok();

        let mut settings = Settings::load(&paths.settings_file);
        if settings.java_path.is_empty() {
            if let Some(java) = detect_java("", settings.required_java_version) {
                settings.java_path = java.to_string_lossy().to_string();
            }
        }

        let mut accounts = match std::fs::read_to_string(&paths.accounts_file) {
            Ok(mut content) => {
                let parsed = serde_json::from_str::<AccountStore>(&content).unwrap_or_default();
                // Legacy account catalogues may contain plaintext tokens; erase the input buffer
                // after deserializing so a second plaintext copy is not left to normal deallocation.
                content.zeroize();
                parsed
            }
            Err(_) => AccountStore::default(),
        };

        let credentials_exist = credentials::file_path(&paths.accounts_file).is_file();
        let mut secrets_unavailable = false;

        if credentials_exist {
            match credentials::load(&paths.accounts_file, &mut accounts) {
                Ok(_) => {
                    // A previous interrupted migration may have written the vault but not yet
                    // removed legacy plaintext token fields from accounts.json.
                    if let Err(error) = write_account_catalogue(&paths.accounts_file, &accounts) {
                        eprintln!(
                            "Aether: could not scrub legacy account tokens from disk: {error}"
                        );
                    }
                }
                Err(error) => {
                    eprintln!(
                        "Aether: {error}. Sign in to the Microsoft account again to restore it."
                    );
                    secrets_unavailable = true;
                    drop_secrets(&paths.accounts_file, &mut accounts);
                }
            }
        }

        let legacy_secrets = !credentials_exist && credentials::has_secrets(&accounts);
        let mut launcher = Launcher {
            paths,
            settings,
            accounts,
            secrets_unavailable,
        };

        // Upgrade older accounts.json files that kept tokens in clear text. The DPAPI file is
        // written first; only then is the public catalogue rewritten without the tokens.
        if legacy_secrets {
            if let Err(error) =
                migrate_legacy_secrets(&launcher.paths.accounts_file, &mut launcher.accounts)
            {
                eprintln!(
                    "Aether: could not migrate Microsoft credentials: {error}. Sign in again."
                );
                launcher.secrets_unavailable = true;
                drop_secrets(&launcher.paths.accounts_file, &mut launcher.accounts);
            }
        }

        launcher
    }

    /// Saves the account catalogue separately from the protected Microsoft session secrets.
    pub fn save(&mut self) -> Result<(), String> {
        let contains_secrets = credentials::has_secrets(&self.accounts);
        if !self.secrets_unavailable || contains_secrets {
            credentials::save(&self.paths.accounts_file, &self.accounts)?;
            self.secrets_unavailable = false;
        }

        self.settings
            .save(&self.paths.settings_file)
            .map_err(|error| format!("could not save settings: {error}"))?;
        write_account_catalogue(&self.paths.accounts_file, &self.accounts)
    }

    /// The profile that applies to the selected account, the one the interface shows.
    pub fn active_profile(&self) -> settings::Profile {
        settings::Profile::from_settings(&self.settings)
    }

    /// Adds a profile, it replaces one with the same name.
    pub fn add_profile(&mut self, profile: settings::Profile) -> Result<(), String> {
        self.settings.put_profile(profile);
        self.save()
    }

    /// Removes a profile, the accounts that chose it fall back to the active one.
    pub fn remove_profile(&mut self, name: &str) -> Result<(), String> {
        self.settings.remove_profile(name)?;

        for account in &mut self.accounts.accounts {
            if account
                .profile
                .as_deref()
                .is_some_and(|chosen| chosen.eq_ignore_ascii_case(name))
            {
                account.profile = None;
            }
        }

        self.save()
    }

    /// The jar of the Aether client.
    pub fn client_jar(&self) -> Option<PathBuf> {
        find_client_jar(&self.paths)
    }

    /// The mods that the game will load, and the ones that are parked next to them.
    pub fn mods(&self) -> Vec<mods::ModEntry> {
        mods::list(&self.paths.mods)
    }

    /// Enables or disables a mod, the file is renamed and not touched any further.
    pub fn set_mod_enabled(&self, name: &str, enabled: bool) -> Result<mods::ModEntry, String> {
        mods::set_enabled(&self.paths.mods, name, enabled)
    }

    /// The last lines of the log of the installations.
    pub fn install_log(&self, lines: usize) -> Result<logs::Tail, std::io::Error> {
        logs::read_tail(&logs::install_log(&self.paths), lines)
    }

    /// The runs of the game that left a log, the newest first.
    pub fn sessions(&self, limit: usize) -> Vec<logs::SessionEntry> {
        logs::sessions(&self.paths, limit)
    }

    /// The last lines of one run of the game.
    pub fn session(&self, name: &str, lines: usize) -> Result<logs::Tail, String> {
        logs::read_session(&self.paths, name, lines)
    }

    /* ------------------------------------------------------------- servers */

    /// The server list of the player.
    pub fn servers(&self) -> servers::ServerStore {
        servers::ServerStore::load(&self.paths.servers_file)
    }

    /// Adds a server to the list.
    pub fn add_server(
        &mut self,
        name: &str,
        address: &str,
    ) -> Result<servers::ServerEntry, String> {
        let mut store = self.servers();
        let entry = store.add(name, address)?;
        store
            .save(&self.paths.servers_file)
            .map_err(|error| error.to_string())?;
        Ok(entry)
    }

    /// Removes a server from the list.
    pub fn remove_server(&mut self, key: &str) -> Result<servers::ServerEntry, String> {
        let mut store = self.servers();
        let entry = store.remove(key)?;
        store
            .save(&self.paths.servers_file)
            .map_err(|error| error.to_string())?;
        Ok(entry)
    }

    /// Pings a server, by name or by address.
    pub fn ping_server(&self, address: &str) -> servers::PingResult {
        servers::ping(&self.servers().address_of(address))
    }

    /// Probes the login state of a server: offline accepted, premium only or refused.
    pub fn probe_server(&self, address: &str, username: &str) -> servers::LoginProbe {
        servers::probe_login(&self.servers().address_of(address), username)
    }

    /* -------------------------------------------------------------- hosting */

    /// The servers that the launcher hosts locally.
    pub fn hosted_servers(&self) -> Vec<hosting::HostedServer> {
        hosting::list(&self.paths)
    }

    /// Creates a local server that accepts offline players.
    pub fn create_hosted_server(
        &mut self,
        name: &str,
        port: u16,
        motd: Option<&str>,
        max_players: u32,
        online_mode: bool,
    ) -> Result<hosting::HostedServer, String> {
        hosting::create(
            &self.paths,
            name,
            port,
            motd,
            max_players,
            online_mode,
            &self.settings.game_version,
            &self.settings.loader_version,
        )
    }

    /// Creates a hosted server using the newest Fabric loader for the chosen Minecraft version.
    pub fn create_hosted_server_for_version(
        &self,
        name: &str,
        port: u16,
        max_players: u32,
        game_version: &str,
    ) -> Result<hosting::HostedServer, String> {
        hosting::create_for_version(&self.paths, name, port, max_players, game_version)
    }

    /// Starts a local server in the background.
    pub fn start_hosted_server(&self, name: &str) -> Result<hosting::HostedServer, String> {
        hosting::start(&self.paths, name)
    }

    /// Stops a local server, cleanly through RCON when it answers.
    pub fn stop_hosted_server(&self, name: &str) -> Result<hosting::HostedServer, String> {
        hosting::stop(&self.paths, name)
    }

    /* ---------------------------------------------------------- performance */

    /// The optimisation pack and the state of every mod.
    pub fn optimisation_mods(&self) -> Vec<performance::OptimisationMod> {
        performance::list(&self.paths)
    }

    /// Installs the pack, or the mods named in `only`.
    pub fn install_optimisation(
        &self,
        only: &[String],
        on_progress: impl FnMut(&performance::ModProgress),
    ) -> performance::InstallReport {
        performance::install(&self.paths, &self.settings.game_version, only, on_progress)
    }

    /// The installation that was interrupted, if the last run did not finish.
    pub fn pending_installation(&self) -> Option<pending::PendingInstall> {
        pending::read(&self.paths)
    }

    /// Removes the journal of an interrupted installation and its partial downloads.
    pub fn discard_pending_installation(&self) -> Result<usize, String> {
        pending::discard(&self.paths)
    }

    /// Checks the files of an installed version against their published digest, and optionally
    /// re-downloads only the files that are missing or altered.
    pub fn verify_installation(
        &self,
        version: Option<&str>,
        repair: bool,
    ) -> Result<installer::VerifyReport, installer::InstallError> {
        let version = version
            .map(str::to_string)
            .unwrap_or_else(|| self.settings.game_version.clone());

        if repair {
            installer::repair(&self.paths, &version)
        } else {
            installer::verify(&self.paths, &version)
        }
    }

    /// Checks the jars of the optimisation pack against the manifest of their checksums.
    pub fn verify_optimisation(&self) -> performance::PackReport {
        performance::verify(&self.paths)
    }

    /// Applies a preset of `options.txt` and records it in the settings.
    pub fn tune_options(&mut self, preset: &str) -> Result<performance::OptionsReport, String> {
        let report = performance::tune_options(&self.paths, preset)?;
        self.settings.options_preset = report.preset.clone();
        self.save()?;
        Ok(report)
    }

    /// The released versions of Minecraft, newest first.
    pub fn list_versions(&self, limit: usize) -> Result<Vec<VersionEntry>, http::HttpError> {
        let manifest = fetch_manifest()?;
        Ok(released_versions(&manifest, limit))
    }

    /// The Fabric loaders of a version.
    pub fn list_loaders(&self, game_version: &str) -> Result<Vec<String>, http::HttpError> {
        fetch_fabric_loaders(game_version)
    }

    /// Adds an offline account.
    pub fn add_offline_account(&mut self, username: &str) -> Result<Account, String> {
        let account = self
            .accounts
            .add_offline(username)
            .map_err(|error| error.to_string())?;
        self.save()?;
        Ok(account)
    }

    /// Adds an account that was created somewhere else.
    pub fn add_account(&mut self, account: Account) -> Result<(), String> {
        self.accounts.upsert(account);
        self.save()
    }

    /// Removes an account.
    pub fn remove_account(&mut self, id: &str) -> Result<(), String> {
        self.accounts.remove(id);
        self.save()
    }

    /// Selects the active account.
    pub fn select_account(&mut self, id: &str) -> Result<(), String> {
        self.accounts.select(id);
        self.save()
    }

    /// Downloads the game, the libraries, the assets and the mods.
    ///
    /// The token stops the installation when it becomes true, the files that were already
    /// downloaded are kept so that the next attempt resumes instead of starting over.
    pub fn install(
        &mut self,
        game_version: Option<&str>,
        loader_version: Option<&str>,
        fabric_api_version: Option<&str>,
        cancel: &std::sync::atomic::AtomicBool,
        on_progress: impl FnMut(&InstallProgress),
    ) -> Result<(String, String, String), installer::InstallError> {
        // Installation defaults to the launcher-wide version settings.
        let profile = settings::Profile::from_settings(&self.settings);

        let game_version = game_version
            .map(|value| value.to_string())
            .unwrap_or_else(|| profile.game_version.clone());
        let loader_version = loader_version
            .map(|value| value.to_string())
            .unwrap_or_else(|| profile.loader_version.clone());
        let fabric_api_version = fabric_api_version
            .map(|value| value.to_string())
            .unwrap_or_else(|| self.settings.fabric_api_version.clone());

        let mod_jar = self.client_jar().ok_or_else(|| {
            installer::InstallError::MissingMod(
                "aether-client/build/libs/aether-client-1.0.0.jar".to_string(),
            )
        })?;

        installer::install_version(
            &game_version,
            &loader_version,
            &fabric_api_version,
            &mod_jar,
            &self.paths,
            cancel,
            on_progress,
        )?;

        self.settings.game_version = game_version.clone();
        self.settings.loader_version = loader_version.clone();
        self.settings.fabric_api_version = fabric_api_version.clone();
        self.save()
            .map_err(|error| installer::InstallError::Io(std::io::Error::other(error)))?;

        Ok((game_version, loader_version, fabric_api_version))
    }

    /// The options that are used to start the game.
    ///
    /// A Microsoft session that is about to expire is renewed first and written to the disk, so
    /// that the account does not have to be signed in again by hand every day.
    pub fn launch_options(
        &mut self,
        account_id: Option<&str>,
        game_version: Option<&str>,
    ) -> Result<LaunchOptions, String> {
        self.launch_options_for(account_id, game_version, None)
    }

    /// The options of a launch that joins a server right away.
    ///
    /// `server` may be a name of the server list, an address or `None`.
    pub fn launch_options_for(
        &mut self,
        account_id: Option<&str>,
        game_version: Option<&str>,
        server: Option<&str>,
    ) -> Result<LaunchOptions, String> {
        let account_id = account_id
            .map(|value| value.to_string())
            .or_else(|| self.accounts.selected.clone())
            .ok_or_else(|| "no account selected".to_string())?;

        let mut account = self
            .accounts
            .find(&account_id)
            .cloned()
            .ok_or_else(|| "the account does not exist".to_string())?;

        if accounts::needs_refresh(&account) {
            let refreshed = accounts::refresh_account(&account).map_err(login_error)?;
            self.accounts.replace(refreshed);
            self.save()?;

            account = self
                .accounts
                .find(&account_id)
                .cloned()
                .ok_or_else(|| "the account does not exist".to_string())?;
        }

        // Launcher-wide game settings apply to every account. Account identity remains separate.
        let profile = settings::Profile::from_settings(&self.settings);

        let game_version = game_version
            .map(|value| value.to_string())
            .unwrap_or_else(|| profile.game_version.clone());

        let java_path = if self.settings.java_path.is_empty() {
            detect_java("", self.settings.required_java_version).ok_or_else(|| {
                "no Java runtime found, set the Java path in the settings".to_string()
            })?
        } else {
            PathBuf::from(&self.settings.java_path)
        };

        let vanilla = fetch_version_json(&game_version)
            .map_err(|error| format!("could not read the version metadata: {}", error))?;
        let fabric = fetch_fabric_profile(&game_version, &profile.loader_version)
            .map_err(|error| format!("could not read the Fabric profile: {}", error))?;
        let version = merge_version_json(&vanilla, &fabric);

        let session = Session::from(&account);

        let quick_play_server = server
            .map(|key| self.servers().address_of(key))
            .filter(|address| !address.trim().is_empty());

        Ok(LaunchOptions {
            username: session.username,
            uuid: session.uuid,
            access_token: session.access_token,
            offline: session.offline,
            xuid: session.xuid,
            client_id: session.client_id,
            version,
            client_jar: self
                .paths
                .versions
                .join(&game_version)
                .join(format!("{}.jar", game_version)),
            extra_jars: vec![
                self.paths.mods.join(format!(
                    "fabric-api-{}.jar",
                    self.settings.fabric_api_version
                )),
                self.paths.mods.join("aether-client.jar"),
            ],
            natives_dir: self.paths.versions.join(&game_version).join("natives"),
            game_dir: self.paths.game.clone(),
            assets_dir: self.paths.assets.clone(),
            asset_index: vanilla.assets.unwrap_or_else(|| "29".to_string()),
            library_dir: self.paths.libraries.clone(),
            java_path,
            max_memory: profile.max_memory,
            min_memory: profile.min_memory,
            window_width: profile.window_width,
            window_height: profile.window_height,
            use_g1gc: self.settings.use_g1gc,
            jvm_preset: self.settings.jvm_preset.clone(),
            quick_play_server,
        })
    }

    /// Starts the game.
    pub fn launch(
        &mut self,
        account_id: Option<&str>,
        game_version: Option<&str>,
        server: Option<&str>,
        mut on_output: impl FnMut(&str),
    ) -> Result<(u32, bool), String> {
        let options = self.launch_options_for(account_id, game_version, server)?;
        let offline = options.offline;

        // Every run keeps its own log: a crash is explained by the file, not by a buffer of the
        // interface that the next launch wipes.
        let mut session = logs::SessionLog::start(&self.paths).ok();

        // The record of the process outlives this call: a shell that crashes must not leave Java
        // running with nothing able to name it.
        let data = self.paths.data.clone();

        let mut child = game::launch(
            &options,
            |pid: u32| game::record_pid(&data, pid),
            |line: &str| {
                if let Some(session) = session.as_mut() {
                    session.line(line);
                }
                on_output(line);
            },
        )
        .map_err(|error| format!("could not start java: {}", error))?;

        let status = match child.wait() {
            Ok(status) => status,
            Err(error) => {
                game::forget_pid(&self.paths.data);
                return Err(format!("the game stopped: {}", error));
            }
        };
        game::forget_pid(&self.paths.data);

        let code = status.code().unwrap_or(0) as u32;
        if let Some(session) = session.as_mut() {
            session.finish(code as i32);
        }

        Ok((code, offline))
    }

    /// The game that is still running, including the one of a launcher that is gone.
    pub fn running_game(&self) -> Option<u32> {
        game::running(&self.paths.data)
    }

    /// Stops the game from its record.
    ///
    /// The shell closes the child it started on its own; this is the path that still works after the
    /// shell itself was killed, or for a game started from the command line.
    pub fn stop_game(&self) -> Result<u32, String> {
        game::stop(&self.paths.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A catalogue written by a version that kept the Microsoft tokens in clear text.
    const LEGACY_CATALOGUE: &str = r#"{
        "accounts": [
            {
                "type": "microsoft",
                "id": "account-1",
                "username": "AetherTester",
                "uuid": "00000000-0000-0000-0000-000000000001",
                "accessToken": "legacy-access-token",
                "refreshToken": "legacy-refresh-token",
                "expiresAt": 1900000000
            }
        ],
        "selected": "account-1"
    }"#;

    fn scratch(name: &str) -> Paths {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let home =
            std::env::temp_dir().join(format!("aether-{}-{}-{}", name, std::process::id(), stamp));
        let game = home.join(".minecraft");
        let data = home.join("aether");

        let paths = Paths {
            assets: game.join("assets"),
            asset_indexes: game.join("assets").join("indexes"),
            asset_objects: game.join("assets").join("objects"),
            libraries: game.join("libraries"),
            versions: game.join("versions"),
            mods: game.join("mods"),
            settings_file: data.join("settings.json"),
            accounts_file: data.join("accounts.json"),
            servers_file: data.join("servers.json"),
            server_root: home.join("servers"),
            options_file: game.join("options.txt"),
            logs: data.join("logs"),
            home,
            game,
            data,
        };
        paths.ensure().unwrap();
        paths
    }

    #[cfg(windows)]
    #[test]
    fn a_legacy_catalogue_is_migrated_into_the_vault_and_scrubbed() {
        let paths = scratch("legacy-catalogue");
        std::fs::write(&paths.accounts_file, LEGACY_CATALOGUE).unwrap();

        let mut accounts: AccountStore =
            serde_json::from_str(&std::fs::read_to_string(&paths.accounts_file).unwrap()).unwrap();
        assert!(
            credentials::has_secrets(&accounts),
            "the fixture must start with clear-text tokens"
        );

        migrate_legacy_secrets(&paths.accounts_file, &mut accounts).unwrap();

        let vault = std::fs::read_to_string(credentials::file_path(&paths.accounts_file)).unwrap();
        assert!(vault.starts_with("AETHER-CREDENTIALS-V1"));
        assert!(!vault.contains("legacy-access-token"));
        assert!(!vault.contains("legacy-refresh-token"));

        let catalogue = std::fs::read_to_string(&paths.accounts_file).unwrap();
        assert!(!catalogue.contains("legacy-access-token"));
        assert!(!catalogue.contains("legacy-refresh-token"));
        assert!(!catalogue.contains("accessToken"), "{}", catalogue);
        assert!(catalogue.contains("AetherTester"));

        // The tokens are not lost: the next start reads them back from the vault.
        let mut reloaded: AccountStore = serde_json::from_str(&catalogue).unwrap();
        assert!(!credentials::has_secrets(&reloaded));
        assert!(credentials::load(&paths.accounts_file, &mut reloaded).unwrap());
        assert_eq!(reloaded.accounts[0].access_token, "legacy-access-token");
        assert_eq!(
            reloaded.accounts[0].refresh_token.as_deref(),
            Some("legacy-refresh-token")
        );

        std::fs::remove_dir_all(&paths.home).ok();
    }

    #[test]
    fn a_catalogue_loses_its_tokens_when_the_vault_cannot_be_written() {
        let paths = scratch("legacy-unprotected");
        std::fs::write(&paths.accounts_file, LEGACY_CATALOGUE).unwrap();
        let mut accounts: AccountStore = serde_json::from_str(LEGACY_CATALOGUE).unwrap();

        drop_secrets(&paths.accounts_file, &mut accounts);

        assert!(accounts.accounts[0].access_token.is_empty());
        assert!(accounts.accounts[0].refresh_token.is_none());
        assert!(!credentials::has_secrets(&accounts));

        let catalogue = std::fs::read_to_string(&paths.accounts_file).unwrap();
        assert!(!catalogue.contains("legacy-access-token"));
        assert!(!catalogue.contains("legacy-refresh-token"));
        assert!(!catalogue.contains("accessToken"), "{}", catalogue);

        std::fs::remove_dir_all(&paths.home).ok();
    }
}
