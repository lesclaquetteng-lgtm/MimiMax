//! The Aether launcher.
//!
//! The launcher installs Minecraft with the Fabric loader and the Aether client and starts the game
//! with an offline account or a Microsoft account.

pub mod accounts;
pub mod game;
pub mod hosting;
pub mod http;
pub mod installer;
pub mod logs;
pub mod minecraft;
pub mod mods;
pub mod offline;
pub mod paths;
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

/// The launcher, it owns the settings, the accounts and the game.
pub struct Launcher {
    pub paths: Paths,
    pub settings: Settings,
    pub accounts: AccountStore,
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

        let accounts = match std::fs::read_to_string(&paths.accounts_file) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => AccountStore::default(),
        };

        Launcher {
            paths,
            settings,
            accounts,
        }
    }

    /// Writes the settings and the accounts to the disk.
    pub fn save(&self) {
        self.settings.save(&self.paths.settings_file).ok();
        if let Ok(content) = serde_json::to_string_pretty(&self.accounts) {
            settings::write_atomic(&self.paths.accounts_file, &content).ok();
        }
    }

    /// The profile that applies to the selected account, the one the interface shows.
    pub fn active_profile(&self) -> settings::Profile {
        let chosen = self
            .accounts
            .selected
            .as_deref()
            .and_then(|id| self.accounts.find(id))
            .and_then(|account| account.profile.clone());

        self.settings.profile_for(chosen.as_deref())
    }

    /// Adds a profile, it replaces one with the same name.
    pub fn add_profile(&mut self, profile: settings::Profile) {
        self.settings.put_profile(profile);
        self.save();
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

        self.save();
        Ok(())
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
        performance::install(
            &self.paths,
            &self.settings.game_version,
            only,
            on_progress,
        )
    }

    /// Applies a preset of `options.txt` and records it in the settings.
    pub fn tune_options(&mut self, preset: &str) -> Result<performance::OptionsReport, String> {
        let report = performance::tune_options(&self.paths, preset)?;
        self.settings.options_preset = report.preset.clone();
        self.save();
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
    pub fn add_offline_account(&mut self, username: &str) -> Result<Account, offline::OfflineError> {
        let account = self.accounts.add_offline(username)?;
        self.save();
        Ok(account)
    }

    /// Adds an account that was created somewhere else.
    pub fn add_account(&mut self, account: Account) {
        self.accounts.upsert(account);
        self.save();
    }

    /// Removes an account.
    pub fn remove_account(&mut self, id: &str) {
        self.accounts.remove(id);
        self.save();
    }

    /// Selects the active account.
    pub fn select_account(&mut self, id: &str) {
        self.accounts.select(id);
        self.save();
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
        // An installation without explicit versions uses the profile of the selected account.
        let profile = self.active_profile();

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
            installer::InstallError::MissingMod("aether-client/build/libs/aether-client-1.0.0.jar".to_string())
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
        self.save();

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
            self.save();

            account = self
                .accounts
                .find(&account_id)
                .cloned()
                .ok_or_else(|| "the account does not exist".to_string())?;
        }

        // The profile of the account decides the version, the loader and the resources it runs with.
        let profile = self.settings.profile_for(account.profile.as_deref());

        let game_version = game_version
            .map(|value| value.to_string())
            .unwrap_or_else(|| profile.game_version.clone());

        let java_path = if self.settings.java_path.is_empty() {
            detect_java("", self.settings.required_java_version)
                .ok_or_else(|| "no Java runtime found, set the Java path in the settings".to_string())?
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
                self.paths
                    .mods
                    .join(format!("fabric-api-{}.jar", self.settings.fabric_api_version)),
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

        let mut child = game::launch(&options, |line: &str| {
            if let Some(session) = session.as_mut() {
                session.line(line);
            }
            on_output(line);
        })
        .map_err(|error| format!("could not start java: {}", error))?;

        let status = child
            .wait()
            .map_err(|error| format!("the game stopped: {}", error))?;

        let code = status.code().unwrap_or(0) as u32;
        if let Some(session) = session.as_mut() {
            session.finish(code as i32);
        }

        Ok((code, offline))
    }
}
