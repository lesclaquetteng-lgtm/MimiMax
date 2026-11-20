//! The command line interface of the Aether launcher.
//!
//! Every command prints a JSON document, the graphical interface of the launcher uses the same
//! commands to talk to the core.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use aether_launcher::accounts;
use aether_launcher::game;
use aether_launcher::logs;
use aether_launcher::installer::{InstallError, InstallProgress};
use aether_launcher::minecraft;
use aether_launcher::settings::Profile;
use aether_launcher::Launcher;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    if arguments.is_empty() || arguments[0] == "help" || arguments[0] == "--help" {
        print_help();
        return;
    }

    let command = arguments[0].clone();
    let rest = &arguments[1..];

    let result = match command.as_str() {
        "info" => command_info(),
        "settings" => command_settings(rest),
        "accounts" => command_accounts(),
        "account" => command_account(rest),
        "versions" => command_versions(rest),
        "loaders" => command_loaders(rest),
        "install" => command_install(rest),
        "launch" => command_launch(rest),
        "stop" => command_stop(),
        "mods" => command_mods(rest),
        "logs" => command_logs(rest),
        "profiles" => command_profiles(rest),
        "servers" => command_servers(rest),
        "hosts" => command_hosts(rest),
        "performance" => command_performance(rest),
        "command" => command_command(rest),
        other => Err(format!("unknown command: {}", other)),
    };

    if let Err(error) = result {
        emit(&serde_json::json!({ "ok": false, "error": error }));
        std::process::exit(1);
    }
}

/// Prints a JSON document on a single line.
fn emit(value: &serde_json::Value) {
    let mut stdout = std::io::stdout();
    writeln!(stdout, "{}", value).ok();
    stdout.flush().ok();
}

/// The arguments of a command.
struct Flags {
    values: Vec<(String, String)>,
    positional: Vec<String>,
}

impl Flags {
    fn parse(arguments: &[String]) -> Flags {
        let mut values = Vec::new();
        let mut positional = Vec::new();

        let mut index = 0;
        while index < arguments.len() {
            let argument = &arguments[index];
            if let Some(name) = argument.strip_prefix("--") {
                let value = if index + 1 < arguments.len()
                    && !arguments[index + 1].starts_with("--")
                {
                    index += 1;
                    arguments[index].clone()
                } else {
                    "true".to_string()
                };
                values.push((name.to_string(), value));
            } else {
                positional.push(argument.clone());
            }
            index += 1;
        }

        Flags { values, positional }
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn number(&self, name: &str) -> Option<u32> {
        self.get(name).and_then(|value| value.parse().ok())
    }
}

fn command_info() -> Result<(), String> {
    let launcher = Launcher::open();

    let paths = serde_json::json!({
        "home": launcher.paths.home.to_string_lossy(),
        "data": launcher.paths.data.to_string_lossy(),
        "game": launcher.paths.game.to_string_lossy(),
        "assets": launcher.paths.assets.to_string_lossy(),
        "libraries": launcher.paths.libraries.to_string_lossy(),
        "versions": launcher.paths.versions.to_string_lossy(),
        "mods": launcher.paths.mods.to_string_lossy(),
        "settings": launcher.paths.settings_file.to_string_lossy(),
        "accounts": launcher.paths.accounts_file.to_string_lossy(),
        "logs": launcher.paths.logs.to_string_lossy(),
    });

    emit(&serde_json::json!({
        "ok": true,
        "name": "Aether Launcher",
        "version": env!("CARGO_PKG_VERSION"),
        "paths": paths,
        "settings": launcher.settings,
        // The profile that applies to the selected account, the one a launch would use.
        "profile": launcher.active_profile(),
    }));
    Ok(())
}

fn command_settings(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);

    // The pairs of `--key K --value V` are collected in the order they were given.
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut pending: Option<String> = None;
    for (name, value) in &flags.values {
        match name.as_str() {
            "key" => pending = Some(value.clone()),
            "value" => {
                if let Some(key) = pending.take() {
                    pairs.push((key, value.clone()));
                }
            }
            _ => {}
        }
    }

    if !pairs.is_empty() {
        let mut updated = launcher.settings.patched(&pairs)?;
        updated.max_memory = updated.max_memory.max(256);
        updated.min_memory = updated.min_memory.max(256);
        updated.window_width = updated.window_width.max(320);
        updated.window_height = updated.window_height.max(240);
        updated.max_memory = updated.max_memory.max(updated.min_memory);
        launcher.settings = updated;
        launcher.save();
    }

    emit(&serde_json::json!({ "ok": true, "settings": launcher.settings }));
    Ok(())
}

fn command_accounts() -> Result<(), String> {
    let launcher = Launcher::open();
    emit(&serde_json::json!({ "ok": true, "accounts": launcher.accounts }));
    Ok(())
}

fn command_account(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);

    match flags.positional.first().map(|value| value.as_str()) {
        Some("add-offline") => {
            let username = flags
                .positional
                .get(1)
                .ok_or_else(|| "usage: account add-offline <username>".to_string())?;
            let account = launcher
                .add_offline_account(username)
                .map_err(|error| error.to_string())?;
            emit(&serde_json::json!({ "ok": true, "account": account }));
        }
        Some("remove") => {
            let id = flags
                .positional
                .get(1)
                .ok_or_else(|| "usage: account remove <id>".to_string())?;
            launcher.remove_account(id);
            emit(&serde_json::json!({ "ok": true, "accounts": launcher.accounts }));
        }
        Some("select") => {
            let id = flags
                .positional
                .get(1)
                .ok_or_else(|| "usage: account select <id>".to_string())?;
            launcher.select_account(id);
            emit(&serde_json::json!({ "ok": true, "accounts": launcher.accounts }));
        }
        Some("check") => {
            let name = flags
                .positional
                .get(1)
                .ok_or_else(|| "usage: account check <username>".to_string())?;
            let status = accounts::name_status(name).map_err(|error| error.to_string())?;
            emit(&serde_json::json!({ "ok": true, "name": status.name, "premium": status.premium }));
        }
        Some("microsoft") => {
            // The code is printed first so that the interface can show it, the sign in is then
            // finished in the background.
            let code = accounts::start_device_code().map_err(|error| error.to_string())?;
            emit(&serde_json::json!({
                "type": "deviceCode",
                "deviceCode": code.device_code,
                "userCode": code.user_code,
                "verificationUri": code.verification_uri,
                "interval": code.interval,
                "expiresIn": code.expires_in,
            }));

            let token = accounts::poll_device_code(&code).map_err(|error| error.to_string())?;
            let account = accounts::complete_microsoft_login(&token)
                .map_err(|error| error.to_string())?;
            launcher.add_account(account.clone());

            // The last document of a command is its result, the interface reads it for the account.
            emit(&serde_json::json!({ "ok": true, "account": account }));
        }
        _ => return Err("usage: account <add-offline|remove|select|check|microsoft>".to_string()),
    }

    Ok(())
}

fn command_versions(arguments: &[String]) -> Result<(), String> {
    let launcher = Launcher::open();
    let flags = Flags::parse(arguments);
    let limit = flags.number("limit").unwrap_or(40) as usize;

    let versions = launcher
        .list_versions(limit)
        .map_err(|error| error.to_string())?;

    emit(&serde_json::json!({ "ok": true, "versions": versions }));
    Ok(())
}

fn command_loaders(arguments: &[String]) -> Result<(), String> {
    let launcher = Launcher::open();
    let flags = Flags::parse(arguments);
    let version = flags
        .get("version")
        .map(|value| value.to_string())
        .unwrap_or_else(|| launcher.settings.game_version.clone());

    let loaders = launcher
        .list_loaders(&version)
        .map_err(|error| error.to_string())?;

    emit(&serde_json::json!({ "ok": true, "loaders": loaders }));
    Ok(())
}

/// The named profiles: the command lists them and changes them.
///
/// `add NAME [--version] [--loader] [--memory] [--min-memory] [--width] [--height]`,
/// `use NAME`, `assign ACCOUNT [NAME]` and `remove NAME`.
fn command_profiles(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);
    let action = flags.positional.first().map(String::as_str).unwrap_or("list");

    let argument = |index: usize, usage: &str| -> Result<String, String> {
        flags
            .positional
            .get(index)
            .cloned()
            .ok_or_else(|| usage.to_string())
    };

    let number = |key: &str, default: u32| -> u32 {
        flags
            .get(key)
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };

    match action {
        "list" => {}
        "add" => {
            let name = argument(1, "profiles add needs a name")?;
            // A new profile starts as a copy of the settings, the two flags override it.
            let mut profile = Profile::named(&name, &launcher.settings);

            if let Some(value) = flags.get("version") {
                profile.game_version = value.to_string();
            }
            if let Some(value) = flags.get("loader") {
                profile.loader_version = value.to_string();
            }

            profile.max_memory = number("memory", profile.max_memory);
            profile.min_memory = number("min-memory", profile.min_memory);
            profile.window_width = number("width", profile.window_width);
            profile.window_height = number("height", profile.window_height);

            launcher.add_profile(profile);
        }
        "use" => {
            let name = argument(1, "profiles use needs a name")?;
            launcher.settings.active_profile = launcher
                .settings
                .profile(&name)
                .map(|profile| profile.name.clone())
                .ok_or_else(|| format!("unknown profile: {}", name))?;
            launcher.save();
        }
        "assign" => {
            let account = argument(1, "profiles assign needs an account")?;
            let name = flags.positional.get(2).map(String::as_str);

            if let Some(name) = name {
                launcher
                    .settings
                    .profile(name)
                    .ok_or_else(|| format!("unknown profile: {}", name))?;
            }

            launcher
                .accounts
                .set_profile(&account, name)
                .ok_or_else(|| format!("unknown account: {}", account))?;
            launcher.save();
        }
        "update" => {
            let name = argument(1, "profiles update needs a name")?;
            let profile = launcher
                .settings
                .profile(&name)
                .cloned()
                .ok_or_else(|| format!("unknown profile: {}", name))?;
            let mut profile = profile;

            if let Some(value) = flags.get("version") {
                profile.game_version = value.to_string();
            }
            if let Some(value) = flags.get("loader") {
                profile.loader_version = value.to_string();
            }

            profile.max_memory = number("memory", profile.max_memory);
            profile.min_memory = number("min-memory", profile.min_memory);
            profile.window_width = number("width", profile.window_width);
            profile.window_height = number("height", profile.window_height);

            launcher.add_profile(profile);
        }
        "remove" => {
            let name = argument(1, "profiles remove needs a name")?;
            launcher.remove_profile(&name)?;
        }
        other => return Err(format!("unknown profiles action: {}", other)),
    }

    emit(&serde_json::json!({
        "ok": true,
        "profiles": launcher.settings.profiles,
        "active": launcher.settings.active_profile,
        "activeFor": launcher.active_profile(),
        "accounts": launcher.accounts.accounts.iter().map(|account| serde_json::json!({
            "id": account.id,
            "username": account.username,
            "profile": account.profile,
        })).collect::<Vec<_>>(),
    }));
    Ok(())
}

fn command_install(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);

    // The interface asks for the stop by writing `cancel` on the standard input, the installation
    // runs in this process and has no other channel back to it.
    let cancel = Arc::new(AtomicBool::new(false));
    watch_cancel(Arc::clone(&cancel));

    let result = launcher.install(
        flags.get("version"),
        flags.get("loader"),
        flags.get("fabric-api"),
        &cancel,
        |progress: &InstallProgress| {
            emit(&serde_json::json!({
                "type": "progress",
                "task": progress.task,
                "received": progress.received,
                "total": progress.total,
                "completed": progress.completed,
                "totalTasks": progress.total_tasks,
            }));
        },
    );

    match result {
        Ok((game_version, loader_version, fabric_api_version)) => {
            emit(&serde_json::json!({
                "ok": true,
                "gameVersion": game_version,
                "loaderVersion": loader_version,
                "fabricApiVersion": fabric_api_version,
            }));
            Ok(())
        }
        Err(error) => {
            // A cancellation is not a failure of the launcher, the interface shows it as its own
            // answer instead of a red error.
            emit(&serde_json::json!({
                "ok": false,
                "cancelled": matches!(error, InstallError::Cancelled),
                "error": error.to_string(),
            }));
            Ok(())
        }
    }
}

/// Sets the flag when the interface writes `cancel` on the standard input.
fn watch_cancel(flag: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().eq_ignore_ascii_case("cancel") {
                flag.store(true, Ordering::Relaxed);
            }
        }
    });
}

fn command_launch(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);

    let account = flags.get("account").map(|value| value.to_string());
    let version = flags.get("version").map(|value| value.to_string());
    let server = flags.get("server").map(|value| value.to_string());

    let result = launcher.launch(
        account.as_deref(),
        version.as_deref(),
        server.as_deref(),
        |line: &str| {
            emit(&serde_json::json!({ "type": "output", "line": line }));
        },
    );

    match result {
        Ok((code, offline)) => {
            emit(&serde_json::json!({ "ok": true, "exitCode": code, "offline": offline }));
            if code != 0 {
                std::process::exit(code as i32);
            }
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// Stops the game from the record it left, even when the launcher that started it is gone.
fn command_stop() -> Result<(), String> {
    let launcher = Launcher::open();

    // No game to stop is a state of the launcher, not a failure of the command: the answer says so
    // and the exit code stays zero, so a script can ask without handling an error.
    let answer = match launcher.stop_game() {
        Ok(pid) => serde_json::json!({ "ok": true, "pid": pid }),
        Err(error) => serde_json::json!({ "ok": false, "error": error }),
    };

    emit(&answer);
    Ok(())
}

fn command_mods(arguments: &[String]) -> Result<(), String> {
    let launcher = Launcher::open();
    let flags = Flags::parse(arguments);

    match flags.positional.first().map(|value| value.as_str()) {
        Some("list") | None => {
            emit(&serde_json::json!({
                "ok": true,
                "folder": launcher.paths.mods.to_string_lossy(),
                "mods": launcher.mods(),
            }));
            Ok(())
        }
        Some("toggle") => {
            let name = flags.positional.get(1).ok_or_else(|| {
                "usage: mods toggle <file> [--enabled true|false]".to_string()
            })?;
            let enabled = flags
                .get("enabled")
                .map(|value| value == "true" || value == "1")
                .unwrap_or(true);

            let entry = launcher.set_mod_enabled(name, enabled)?;
            emit(&serde_json::json!({
                "ok": true,
                "mod": entry,
                "folder": launcher.paths.mods.to_string_lossy(),
                "mods": launcher.mods(),
            }));
            Ok(())
        }
        Some(other) => Err(format!("usage: mods <list|toggle> (got {})", other)),
    }
}

fn command_logs(arguments: &[String]) -> Result<(), String> {
    let launcher = Launcher::open();
    let flags = Flags::parse(arguments);
    let lines = flags.number("lines").unwrap_or(200) as usize;

    match flags.positional.first().map(|value| value.as_str()) {
        Some("read") => {
            let name = flags
                .positional
                .get(1)
                .ok_or_else(|| "usage: logs read <session>".to_string())?;

            let session = launcher.session(name, lines)?;
            emit(&serde_json::json!({ "ok": true, "name": name, "session": session }));
            Ok(())
        }
        _ => {
            let install = launcher
                .install_log(lines)
                .map_err(|error| format!("the log could not be read: {}", error))?;

            emit(&serde_json::json!({
                "ok": true,
                "install": install,
                "sessions": launcher.sessions(logs::KEEP_SESSIONS),
                "keep": logs::KEEP_SESSIONS,
            }));
            Ok(())
        }
    }
}

fn command_command(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);

    let account = flags.get("account").map(|value| value.to_string());
    let version = flags.get("version").map(|value| value.to_string());
    let server = flags.get("server").map(|value| value.to_string());

    let options = launcher.launch_options_for(account.as_deref(), version.as_deref(), server.as_deref())?;
    let built = game::build_command(&options);

    emit(&serde_json::json!({
        "ok": true,
        "program": built.program.to_string_lossy(),
        "arguments": built.arguments,
        "classpath": built.classpath.iter().map(|entry| entry.to_string_lossy().to_string()).collect::<Vec<String>>(),
    }));
    Ok(())
}

fn command_servers(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);
    let action = flags.positional.first().map(String::as_str).unwrap_or("list");

    let address = || -> Result<String, String> {
        flags
            .positional
            .get(1)
            .cloned()
            .or_else(|| flags.get("address").map(|value| value.to_string()))
            .ok_or_else(|| "usage: servers <ping|probe|add|remove> <address>".to_string())
    };

    match action {
        "list" => {
            let store = launcher.servers();
            emit(&serde_json::json!({ "ok": true, "servers": store.servers }));
        }
        "add" => {
            let address = address()?;
            let name = flags
                .get("name")
                .unwrap_or(address.as_str())
                .to_string();
            let entry = launcher.add_server(&name, &address)?;
            emit(&serde_json::json!({
                "ok": true,
                "server": entry,
                "servers": launcher.servers().servers,
            }));
        }
        "remove" => {
            let key = address()?;
            let entry = launcher.remove_server(&key)?;
            emit(&serde_json::json!({
                "ok": true,
                "server": entry,
                "servers": launcher.servers().servers,
            }));
        }
        "ping" => {
            let ping = launcher.ping_server(&address()?);
            emit(&serde_json::json!({ "ok": true, "ping": ping }));
        }
        "probe" => {
            let address = address()?;
            let username = flags
                .get("username")
                .map(|value| value.to_string())
                .or_else(|| {
                    launcher
                        .accounts
                        .selected
                        .as_deref()
                        .and_then(|id| launcher.accounts.find(id))
                        .map(|account| account.username.clone())
                })
                .unwrap_or_else(|| "AetherProbe".to_string());

            let probe = launcher.probe_server(&address, &username);
            emit(&serde_json::json!({
                "ok": true,
                "probe": probe,
                "verdict": probe.verdict(),
            }));
        }
        other => return Err(format!("usage: servers <list|add|remove|ping|probe> (got {})", other)),
    }

    Ok(())
}

fn command_hosts(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);
    let action = flags.positional.first().map(String::as_str).unwrap_or("list");

    let name = || -> Result<String, String> {
        flags
            .positional
            .get(1)
            .cloned()
            .ok_or_else(|| "usage: hosts <create|start|stop> <name>".to_string())
    };

    match action {
        "list" => {
            emit(&serde_json::json!({ "ok": true, "servers": launcher.hosted_servers() }));
        }
        "create" => {
            let name = name()?;
            let port = flags.number("port").unwrap_or(25565) as u16;
            let max_players = flags.number("max-players").unwrap_or(20);
            let online_mode = flags
                .get("online-mode")
                .map(|value| value == "true" || value == "1")
                .unwrap_or(false);
            let motd = flags.get("motd").map(|value| value.to_string());

            let server = launcher.create_hosted_server(
                &name,
                port,
                motd.as_deref(),
                max_players,
                online_mode,
            )?;

            let address = aether_launcher::hosting::local_address(server.port);
            emit(&serde_json::json!({
                "ok": true,
                "server": server,
                "address": address,
                "servers": launcher.hosted_servers(),
            }));
        }
        "start" => {
            let server = launcher.start_hosted_server(&name()?)?;
            let address = aether_launcher::hosting::local_address(server.port);
            emit(&serde_json::json!({
                "ok": true,
                "server": server,
                "address": address,
            }));
        }
        "stop" => {
            let server = launcher.stop_hosted_server(&name()?)?;
            emit(&serde_json::json!({
                "ok": true,
                "server": server,
                "servers": launcher.hosted_servers(),
            }));
        }
        other => return Err(format!("usage: hosts <list|create|start|stop> (got {})", other)),
    }

    Ok(())
}

fn command_performance(arguments: &[String]) -> Result<(), String> {
    let mut launcher = Launcher::open();
    let flags = Flags::parse(arguments);
    let action = flags.positional.first().map(String::as_str).unwrap_or("list");

    match action {
        "list" | "status" => {
            let mods = launcher.optimisation_mods();
            emit(&serde_json::json!({
                "ok": true,
                "mods": mods,
                "state": performance_state(&launcher),
            }));
        }
        "install" => {
            // The slugs are either `--slugs a,b` or the positional arguments after `install`.
            let mut only: Vec<String> = flags
                .get("slugs")
                .map(|value| {
                    value
                        .split(',')
                        .map(|slug| slug.trim().to_string())
                        .filter(|slug| !slug.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            only.extend(flags.positional.iter().skip(1).cloned());

            let report = launcher.install_optimisation(&only, |progress| {
                let mut value =
                    serde_json::to_value(progress).unwrap_or_else(|_| serde_json::json!({}));
                if let Some(object) = value.as_object_mut() {
                    object.insert(
                        "type".to_string(),
                        serde_json::Value::String("optimisation".to_string()),
                    );
                }
                emit(&value);
            });

            emit(&serde_json::json!({
                "ok": report.failed.is_empty(),
                "report": report,
                "mods": launcher.optimisation_mods(),
                "state": performance_state(&launcher),
            }));
        }
        "tune" => {
            let preset = flags.get("preset").unwrap_or("max");
            let report = launcher.tune_options(preset)?;
            emit(&serde_json::json!({
                "ok": true,
                "report": report,
                "state": performance_state(&launcher),
            }));
        }
        "jvm" => {
            let preset = flags
                .positional
                .get(1)
                .cloned()
                .or_else(|| flags.get("preset").map(|value| value.to_string()))
                .ok_or_else(|| {
                    "usage: performance jvm <balanced|performance|ultra>".to_string()
                })?;

            if game::jvm_preset_flags(&preset).is_empty()
                && !preset.eq_ignore_ascii_case("custom")
            {
                return Err(format!("unknown JVM preset: {}", preset));
            }

            launcher.settings.jvm_preset = preset;
            launcher.save();
            emit(&serde_json::json!({
                "ok": true,
                "state": performance_state(&launcher),
            }));
        }
        other => {
            return Err(format!(
                "usage: performance <list|install|tune|jvm> (got {})",
                other
            ))
        }
    }

    Ok(())
}

/// The state the performance page shows.
fn performance_state(launcher: &Launcher) -> serde_json::Value {
    serde_json::json!({
        "gameVersion": launcher.settings.game_version,
        "optionsPreset": launcher.settings.options_preset,
        "jvmPreset": launcher.settings.jvm_preset,
        "optionsPath": launcher.paths.options_file.to_string_lossy(),
        "modsFolder": launcher.paths.mods.to_string_lossy(),
    })
}

fn print_help() {
    let text = [
        "Aether Launcher",
        "",
        "Usage: aether-launcher <command> [options]",
        "",
        "Commands:",
        "  info                          print the paths and the settings",
        "  settings [--key K --value V]  read or change a setting",
        "  accounts                      list the accounts",
        "  account add-offline <name>    create an offline account",
        "  account remove <id>           remove an account",
        "  account select <id>           select the active account",
        "  account check <name>          ask Mojang whether a name is a premium one",
        "  account microsoft             start the sign in with Microsoft",
        "  versions [--limit N]          list the Minecraft versions",
        "  loaders [--version X]         list the Fabric loaders of a version",
        "  install [--version X]         download the game, the libraries and the assets",
        "  launch [--account ID]         start Minecraft",
        "  stop                          stop the game, even after a crashed launcher",
        "  mods list                     list the jars of the mods folder",
        "  mods toggle <file>            enable or disable a mod (--enabled false to disable)",
        "  logs [--lines N]              print the installation log and the last sessions",
        "  profiles [list|add|update|use|assign|remove]  the named profiles of the player",
        "  servers [list|add|remove|ping|probe]  the server list and the offline probe",
        "  hosts [list|create|start|stop]        a local server that accepts offline players",
        "  performance [list|install|tune|jvm]   the optimisation pack and the game options",
        "  logs read <session>           print the last lines of one session of the game",
        "  command [--account ID] [--server ADDRESS]  print the exact launch command",
        "",
        "Options:",
        "  --version <id>     the Minecraft version (default 1.21.11)",
        "  --loader <id>      the Fabric loader version (default 0.20.1)",
        "  --account <id>     the account, the selected one by default",
        "  --limit <n>        the number of versions",
        "  --key K --value V  change a setting, the value may be JSON (numbers, booleans)",
    ]
    .join("\n");

    println!("{}", text);
}

/// Reads the lines of the standard input, used by the interactive sign in.
#[allow(dead_code)]
fn read_lines() -> Vec<String> {
    let stdin = std::io::stdin();
    stdin
        .lock()
        .lines()
        .map_while(Result::ok)
        .collect()
}

/// The version of Minecraft that is used by default.
#[allow(dead_code)]
pub fn default_version() -> String {
    minecraft::VERSION_MANIFEST_URL.to_string()
}
