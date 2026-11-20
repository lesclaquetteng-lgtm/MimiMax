//! A local Minecraft server that accepts offline players.
//!
//! The launcher creates a Fabric server with `online-mode=false` and
//! `enforce-secure-profile=false`: an offline account can join it, and the game opens on
//! `127.0.0.1:<port>` like on any other server. This is also the honest answer to "play with a
//! cracked account on a server": a server has to disable the authentication itself, the client
//! cannot force it.
//!
//! The server runs in its own directory under `servers/`, writes its console to `console.log` and
//! can be stopped through RCON, which asks the server to save the world and shut down cleanly.

use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::settings::write_atomic;

/// The name of the metadata file of a hosted server.
pub const METADATA: &str = "aether-server.json";
/// The file that holds the process identifier.
pub const PID_FILE: &str = "server.pid";
/// The console of the server.
pub const CONSOLE_LOG: &str = "console.log";

/// A server hosted by the launcher.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedServer {
    pub name: String,
    pub directory: String,
    pub port: u16,
    pub rcon_port: u16,
    #[serde(default)]
    pub online_mode: bool,
    #[serde(default)]
    pub game_version: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub pid: Option<u32>,
}

/// The address an offline client uses to join a hosted server.
pub fn local_address(port: u16) -> String {
    format!("127.0.0.1:{}", port)
}

/// Turns a name into a folder name, without a separator or a parent directory.
pub fn folder_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric()
                || *character == ' '
                || *character == '-'
                || *character == '_'
        })
        .collect();

    let trimmed = cleaned.trim().trim_matches('.').trim();
    let mut result = trimmed.replace(' ', "-").to_ascii_lowercase();

    if result.is_empty() {
        result = "server".to_string();
    }

    result.truncate(48);
    result
}

/// The root of the hosted servers.
fn root(paths: &Paths) -> PathBuf {
    paths.server_root.clone()
}

/// The directory of a server.
pub fn directory(paths: &Paths, name: &str) -> PathBuf {
    root(paths).join(folder_name(name))
}

fn metadata_path(directory: &Path) -> PathBuf {
    directory.join(METADATA)
}

fn pid_path(directory: &Path) -> PathBuf {
    directory.join(PID_FILE)
}

/// Reads the metadata of a server.
fn read_metadata(directory: &Path) -> Option<HostedServer> {
    let content = fs::read_to_string(metadata_path(directory)).ok()?;
    let mut server: HostedServer = serde_json::from_str(&content).ok()?;
    server.pid = read_pid(directory);
    server.running = server.pid.is_some_and(process_alive);
    Some(server)
}

fn read_pid(directory: &Path) -> Option<u32> {
    fs::read_to_string(pid_path(directory))
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

/// Lists every server the launcher created.
pub fn list(paths: &Paths) -> Vec<HostedServer> {
    let entries = match fs::read_dir(root(paths)) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut servers: Vec<HostedServer> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| read_metadata(&entry.path()))
        .collect();

    servers.sort_by_key(|server| server.name.to_lowercase());
    servers
}

/// A free TCP port, starting with the requested one.
fn free_port(preferred: u16) -> u16 {
    if preferred != 0 {
        if let Ok(listener) = TcpListener::bind(("0.0.0.0", preferred)) {
            let port = listener.local_addr().map(|address| address.port()).unwrap_or(preferred);
            drop(listener);
            return port;
        }
    }

    TcpListener::bind(("127.0.0.1", 0))
        .ok()
        .and_then(|listener| listener.local_addr().ok())
        .map(|address| address.port())
        .unwrap_or(25565)
}

/// Generates a 192-bit RCON password using the operating system's CSPRNG.
fn random_password() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("could not generate a secure RCON password: {}", error))?;

    Ok(bytes.iter().map(|byte| format!("{:02x}", byte)).collect())
}

/// One value of `server.properties`, on a single line.
///
/// A newline in a value would end the property and start another one: a server name could then
/// add `enable-rcon=false`, move `level-name`, or change `server-port`. The value is flattened so
/// that what the player typed can never become a second property.
fn property_value(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character == '\n' || character == '\r' {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// The `server.properties` of an offline server.
///
/// The server listens on the loopback interface only. An offline server accepts every name without
/// a password: bound to `0.0.0.0` it would be an open server for the whole local network, and its
/// RCON, which travels in clear, would be reachable too.
fn properties(name: &str, port: u16, rcon_port: u16, password: &str, max_players: u32) -> String {
    [
        "# Written by Aether Launcher.".to_string(),
        "# online-mode=false is what lets a cracked account join this server.".to_string(),
        format!("server-port={}", port),
        "server-ip=127.0.0.1".to_string(),
        "online-mode=false".to_string(),
        "enforce-secure-profile=false".to_string(),
        format!("motd={}", property_value(name)),
        format!("max-players={}", max_players.max(1)),
        "view-distance=10".to_string(),
        "simulation-distance=8".to_string(),
        "level-name=world".to_string(),
        "level-seed=".to_string(),
        "gamemode=survival".to_string(),
        "difficulty=easy".to_string(),
        "pvp=true".to_string(),
        "spawn-protection=0".to_string(),
        "allow-flight=true".to_string(),
        "allow-nether=true".to_string(),
        "enable-command-block=false".to_string(),
        "enable-rcon=true".to_string(),
        format!("rcon.port={}", rcon_port),
        format!("rcon.password={}", password),
        "broadcast-rcon-to-ops=false".to_string(),
        "sync-chunk-writes=false".to_string(),
        "".to_string(),
    ]
    .join("\n")
}

/// Creates a server. An existing server keeps its properties and its world.
#[allow(clippy::too_many_arguments)]
pub fn create(
    paths: &Paths,
    name: &str,
    port: u16,
    motd: Option<&str>,
    max_players: u32,
    online_mode: bool,
    game_version: &str,
    loader_version: &str,
) -> Result<HostedServer, String> {
    if !crate::minecraft::is_safe_path_component(game_version)
        || !crate::minecraft::is_safe_path_component(loader_version)
    {
        return Err("invalid Minecraft or Fabric version identifier for the hosted server".to_string());
    }

    let directory = directory(paths, name);
    fs::create_dir_all(&directory).map_err(|error| format!("could not create the folder: {}", error))?;

    let metadata_file = metadata_path(&directory);
    let existing = read_metadata(&directory);

    let port = free_port(existing.as_ref().map(|server| server.port).unwrap_or(port));
    let rcon_port = free_port(existing.as_ref().map(|server| server.rcon_port).unwrap_or(0));
    let properties_file = directory.join("server.properties");
    let stored_password = server_password(&directory);
    let needs_password = stored_password.is_empty();
    let password = if needs_password {
        random_password()?
    } else {
        stored_password
    };

    if !properties_file.exists() {
        let motd = motd.unwrap_or(name);
        let content = properties(motd, port, rcon_port, &password, max_players);
        write_atomic(&properties_file, &content)
            .map_err(|error| format!("could not write server.properties: {}", error))?;
    } else {
        if online_mode {
            // A server that already exists keeps its world, but the mode is explicit in the request.
            set_property(&properties_file, "online-mode", "true")?;
        }
        // Migrate an old or incomplete configuration without replacing an existing secret.
        if needs_password {
            set_property(&properties_file, "rcon.password", &password)?;
        }
    }

    let eula = directory.join("eula.txt");
    if !eula.exists() {
        write_atomic(
            &eula,
            "# By creating this server you accept https://aka.ms/MinecraftEULA\neula=true\n",
        )
        .map_err(|error| format!("could not write eula.txt: {}", error))?;
    }

    let server = HostedServer {
        name: name.trim().to_string(),
        directory: directory.to_string_lossy().to_string(),
        port,
        rcon_port,
        online_mode,
        game_version: game_version.to_string(),
        loader_version: loader_version.to_string(),
        running: existing.as_ref().is_some_and(|server| server.running),
        pid: existing.and_then(|server| server.pid),
    };

    let content = serde_json::to_string_pretty(&server)
        .map_err(|error| error.to_string())?;
    write_atomic(&metadata_file, &content).map_err(|error| error.to_string())?;

    Ok(server)
}

/// Reads the RCON password from the properties, the metadata never stores it in clear.
fn server_password(directory: &Path) -> String {
    fs::read_to_string(directory.join("server.properties"))
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                line.strip_prefix("rcon.password=")
                    .map(|value| value.trim().to_string())
            })
        })
        .unwrap_or_default()
}

/// Replaces one value of `server.properties`.
fn set_property(file: &Path, key: &str, value: &str) -> Result<(), String> {
    let content = fs::read_to_string(file).map_err(|error| error.to_string())?;
    let prefix = format!("{}=", key);
    let found = content.lines().any(|line| line.starts_with(&prefix));

    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    if found {
        for line in lines.iter_mut() {
            if line.starts_with(&prefix) {
                *line = format!("{}={}", key, value);
            }
        }
    } else {
        lines.push(format!("{}={}", key, value));
    }

    let mut text = lines.join("\n");
    text.push('\n');
    write_atomic(file, &text).map_err(|error| error.to_string())
}

/// The file that records the digest of the Fabric installer that was downloaded.
const INSTALLER_DIGEST: &str = "fabric-installer.sha1";

/// Whether a file starts like a zip archive, which every jar is.
fn looks_like_jar(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .map(|()| &magic[..2] == b"PK")
        .unwrap_or(false)
}

/// The digest that was recorded when the installer was downloaded.
fn recorded_digest(file: &Path) -> Option<String> {
    fs::read_to_string(file)
        .ok()
        .map(|text| text.trim().to_ascii_lowercase())
        .filter(|text| text.len() == 40 && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// Resolves the shared Fabric server directory after validating metadata components.
fn server_artifact_directory(
    paths: &Paths,
    game_version: &str,
    loader_version: &str,
) -> Result<PathBuf, String> {
    if !crate::minecraft::is_safe_path_component(game_version)
        || !crate::minecraft::is_safe_path_component(loader_version)
    {
        return Err("the hosted server contains an unsafe Minecraft or Fabric version identifier".to_string());
    }

    Ok(root(paths).join(format!(
        "fabric-server-{}-{}",
        game_version, loader_version
    )))
}

/// The jar of the Fabric server, installed once and shared by every hosted server.
///
/// Fabric no longer serves a prebuilt server jar: the official installer downloads the vanilla
/// server, the loader libraries and the launch jar. It is run once in a shared folder, and every
/// hosted server reuses the result through `fabric-server-launcher.properties`.
fn ensure_server_jar(
    paths: &Paths,
    game_version: &str,
    loader_version: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let folder = server_artifact_directory(paths, game_version, loader_version)?;
    let launch = folder.join("fabric-server-launch.jar");
    let server = folder.join("server.jar");

    let complete = looks_like_jar(&launch)
        && server
            .metadata()
            .map(|metadata| metadata.len() > 1_000_000)
            .unwrap_or(false)
        && looks_like_jar(&server);

    if complete {
        return Ok((launch, server));
    }

    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;

    let installer = folder.join("fabric-installer.jar");
    let digest_file = folder.join(INSTALLER_DIGEST);

    // The installer is a jar that is then run by the JVM, so it has to be a jar. The archive is
    // checked after the download and its digest is recorded: the next start compares it, and a
    // file that was replaced in between is refused instead of executed.
    if installer.exists() && !looks_like_jar(&installer) {
        let _ = fs::remove_file(&installer);
    }

    if !installer.exists() {
        let url = installer_url()?;
        crate::http::download(&url, &installer, None, |_, _| {})
            .map_err(|error| format!("could not download the Fabric installer: {}", error))?;

        if !looks_like_jar(&installer) {
            let _ = fs::remove_file(&installer);
            return Err(format!(
                "the Fabric installer downloaded from {} is not a jar archive",
                url
            ));
        }

        let digest = crate::http::sha1(&installer)
            .ok_or_else(|| "could not read the digest of the Fabric installer".to_string())?;
        write_atomic(&digest_file, &format!("{}\n", digest))
            .map_err(|error| format!("could not record the digest of the installer: {}", error))?;
    } else if let Some(recorded) = recorded_digest(&digest_file) {
        let current = crate::http::sha1(&installer).unwrap_or_default();
        if current != recorded {
            let _ = fs::remove_file(&installer);
            let _ = fs::remove_file(&digest_file);
            return Err(format!(
                "the Fabric installer changed since it was downloaded (expected {}, got {}); \
                 it was removed, start the server again to download it",
                recorded, current
            ));
        }
    }

    let java = java(paths)?;
    let output = Command::new(&java)
        .arg("-jar")
        .arg(&installer)
        .arg("server")
        .arg("-dir")
        .arg(&folder)
        .arg("-mcversion")
        .arg(game_version)
        .arg("-loader")
        .arg(loader_version)
        .arg("-downloadMinecraft")
        .current_dir(&folder)
        .output()
        .map_err(|error| format!("could not run the Fabric installer: {}", error))?;

    if !output.status.success() {
        let text = String::from_utf8_lossy(&output.stderr);
        let tail: String = text.chars().rev().take(600).collect::<Vec<char>>().into_iter().rev().collect();
        return Err(format!("the Fabric installer failed: {}", tail.trim()));
    }

    if !launch.exists() || !server.exists() {
        return Err("the Fabric installer did not produce the server files".to_string());
    }

    Ok((launch, server))
}

/// The download of the newest stable Fabric installer.
#[derive(Debug, Deserialize)]
struct InstallerRelease {
    url: String,
    #[serde(default)]
    stable: bool,
}

fn installer_url() -> Result<String, String> {
    let releases: Vec<InstallerRelease> =
        crate::http::get_json("https://meta.fabricmc.net/v2/versions/installer")
            .map_err(|error| format!("could not list the Fabric installers: {}", error))?;

    releases
        .iter()
        .find(|release| release.stable)
        .or_else(|| releases.first())
        .map(|release| release.url.clone())
        .ok_or_else(|| "no Fabric installer is published".to_string())
}

/// Escapes a path for a Java properties file: the separators are slashes and a space is escaped.
fn property_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(' ', "\\ ")
}


/// Finds the Java runtime of the game, the one the launcher already detected.
fn java(paths: &Paths) -> Result<PathBuf, String> {
    let settings = crate::settings::Settings::load(&paths.settings_file);
    if !settings.java_path.is_empty() && Path::new(&settings.java_path).exists() {
        return Ok(PathBuf::from(&settings.java_path));
    }

    crate::settings::detect_java("", settings.required_java_version)
        .ok_or_else(|| "no Java runtime found".to_string())
}

/// Starts a hosted server in the background and returns at once.
pub fn start(paths: &Paths, name: &str) -> Result<HostedServer, String> {
    let directory = directory(paths, name);
    let mut server = read_metadata(&directory)
        .ok_or_else(|| format!("unknown server: {}, create it first", name))?;

    if server.running {
        return Ok(server);
    }

    let game_version = if server.game_version.is_empty() {
        GAME_VERSION.to_string()
    } else {
        server.game_version.clone()
    };
    let loader_version = if server.loader_version.is_empty() {
        "0.20.1".to_string()
    } else {
        server.loader_version.clone()
    };

    let (launch_jar, server_jar) = ensure_server_jar(paths, &game_version, &loader_version)?;
    let java = java(paths)?;

    // The shared launch jar finds the vanilla server through this file, next to the world.
    let launcher_properties = directory.join("fabric-server-launcher.properties");
    write_atomic(
        &launcher_properties,
        &format!("serverJar={}\n", property_path(&server_jar)),
    )
    .map_err(|error| format!("could not write the launcher properties: {}", error))?;

    let console = directory.join(CONSOLE_LOG);
    let output = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&console)
        .map_err(|error| format!("could not open the console log: {}", error))?;
    let errors = output
        .try_clone()
        .map_err(|error| format!("could not duplicate the console log: {}", error))?;

    let mut process = Command::new(&java);
    process
        .arg("-Xms1G")
        .arg("-Xmx2G")
        .arg("-jar")
        .arg(&launch_jar)
        .arg("nogui")
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(errors));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        process.creation_flags(CREATE_NO_WINDOW);
    }

    let child = process
        .spawn()
        .map_err(|error| format!("could not start the Java server: {}", error))?;

    let pid = child.id();
    write_atomic(&pid_path(&directory), &pid.to_string())
        .map_err(|error| format!("could not write the pid: {}", error))?;

    server.pid = Some(pid);
    server.running = true;

    Ok(server)
}

/// Asks the server to stop through RCON, then kills it if it does not.
pub fn stop(paths: &Paths, name: &str) -> Result<HostedServer, String> {
    let directory = directory(paths, name);
    let mut server = read_metadata(&directory)
        .ok_or_else(|| format!("unknown server: {}", name))?;

    let Some(pid) = server.pid else {
        server.running = false;
        let _ = fs::remove_file(pid_path(&directory));
        return Ok(server);
    };

    if !process_alive(pid) {
        server.running = false;
        server.pid = None;
        let _ = fs::remove_file(pid_path(&directory));
        return Ok(server);
    }

    // The clean path: RCON tells the server to save and to shut down. A server that is still
    // starting does not listen yet, so the command is retried for a few seconds.
    let password = server_password(&directory);
    for _ in 0..10 {
        if rcon_command(server.rcon_port, &password, "stop").is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(400));
    }

    if !wait_for_exit(pid, Duration::from_secs(20)) {
        kill_tree(pid);
        let _ = wait_for_exit(pid, Duration::from_secs(5));
    }

    let _ = fs::remove_file(pid_path(&directory));
    server.running = false;
    server.pid = None;

    Ok(server)
}

fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
    let started = Instant::now();

    while started.elapsed() < timeout {
        if !process_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    false
}

/// Whether a process is still running.
pub fn process_alive(pid: u32) -> bool {
    // PID 0 is not a process we manage, and Unix process IDs are signed.
    if pid == 0 {
        return false;
    }

    #[cfg(windows)]
    {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .output();

        match output {
            Ok(output) => {
                let text = String::from_utf8_lossy(&output.stdout);
                text.split_whitespace().any(|field| field == pid.to_string())
            }
            Err(_) => false,
        }
    }

    #[cfg(not(windows))]
    {
        // Passing a u32 above i32::MAX to kill(1) can be parsed as a negative
        // process-group ID. Reject it instead of probing an unintended group.
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };

        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

/// Kills a process and its children.
///
/// Shared with the game: a hosted server and a game are both trees that outlive the process the
/// launcher holds.
pub(crate) fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// The version of the game that a hosted server uses by default.
const GAME_VERSION: &str = crate::performance::GAME_VERSION;

/* -------------------------------------------------------------------------- RCON */

/// Builds one RCON packet.
pub fn rcon_packet(identifier: i32, kind: i32, body: &str) -> Vec<u8> {
    let length = 4 + 4 + body.len() + 2;
    let mut packet = Vec::with_capacity(length + 4);

    packet.extend_from_slice(&(length as i32).to_le_bytes());
    packet.extend_from_slice(&identifier.to_le_bytes());
    packet.extend_from_slice(&kind.to_le_bytes());
    packet.extend_from_slice(body.as_bytes());
    packet.push(0);
    packet.push(0);

    packet
}

/// Reads one RCON packet.
fn read_rcon(stream: &mut TcpStream) -> io::Result<(i32, i32, String)> {
    let mut length = [0u8; 4];
    stream.read_exact(&mut length)?;
    let length = i32::from_le_bytes(length);

    if !(10..=4_096).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the RCON packet length is out of range",
        ));
    }

    let mut body = vec![0u8; length as usize];
    stream.read_exact(&mut body)?;

    let identifier = i32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let kind = i32::from_le_bytes([body[4], body[5], body[6], body[7]]);
    let text = String::from_utf8_lossy(&body[8..body.len() - 2]).to_string();

    Ok((identifier, kind, text))
}

/// Sends a command to an RCON server and returns its answer.
pub fn rcon_command(port: u16, password: &str, command: &str) -> Result<String, String> {
    if port == 0 {
        return Err("the server has no RCON port".to_string());
    }

    let address = format!("127.0.0.1:{}", port);
    let mut stream = TcpStream::connect_timeout(
        &address.parse().map_err(|error| format!("invalid address: {}", error))?,
        Duration::from_secs(3),
    )
    .map_err(|error| format!("RCON is not reachable: {}", error))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;

    stream
        .write_all(&rcon_packet(1, 3, password))
        .map_err(|error| error.to_string())?;

    let (identifier, _, _) = read_rcon(&mut stream).map_err(|error| error.to_string())?;
    if identifier == -1 {
        return Err("the server refused the RCON password".to_string());
    }

    stream
        .write_all(&rcon_packet(2, 2, command))
        .map_err(|error| error.to_string())?;

    let (_, _, text) = read_rcon(&mut stream).map_err(|error| error.to_string())?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn test_paths(root: &Path) -> Paths {
        Paths {
            home: root.to_path_buf(),
            data: root.join("aether"),
            game: root.join(".minecraft"),
            assets: root.join(".minecraft/assets"),
            asset_indexes: root.join(".minecraft/assets/indexes"),
            asset_objects: root.join(".minecraft/assets/objects"),
            libraries: root.join(".minecraft/libraries"),
            versions: root.join(".minecraft/versions"),
            mods: root.join(".minecraft/mods"),
            settings_file: root.join("aether/settings.json"),
            accounts_file: root.join("aether/accounts.json"),
            servers_file: root.join("aether/servers.json"),
            server_root: root.join("servers"),
            options_file: root.join(".minecraft/options.txt"),
            logs: root.join("aether/logs"),
        }
    }

    #[test]
    fn a_name_becomes_a_safe_folder() {
        assert_eq!(folder_name("My Server"), "my-server");
        assert_eq!(folder_name("../escape"), "escape");
        assert_eq!(folder_name("C:\\Windows"), "cwindows");
        assert_eq!(folder_name("  "), "server");
        assert_eq!(folder_name("Aether"), "aether");
    }

    #[test]
    fn server_artifact_paths_reject_untrusted_version_components() {
        let root = std::env::temp_dir().join(format!("aether-server-version-path-{}", std::process::id()));
        let paths = test_paths(&root);

        let valid = server_artifact_directory(&paths, "1.21.11", "0.20.1").unwrap();
        assert!(valid.ends_with("fabric-server-1.21.11-0.20.1"));
        assert!(server_artifact_directory(&paths, "../outside", "0.20.1").is_err());
        assert!(server_artifact_directory(&paths, "1.21.11", "../../outside").is_err());
        assert!(server_artifact_directory(&paths, "C:\\Windows", "0.20.1").is_err());
    }

    #[test]
    fn a_path_of_a_property_uses_slashes_and_escapes_spaces() {
        let path = Path::new("C:\\Users\\A B\\server.jar");
        assert_eq!(property_path(path), "C:/Users/A\\ B/server.jar");
    }

    #[test]
    fn a_properties_file_disables_the_authentication() {
        let content = properties("Test", 25565, 25575, "secret", 20);

        assert!(content.contains("online-mode=false"));
        assert!(content.contains("enforce-secure-profile=false"));
        assert!(content.contains("server-port=25565"));
        assert!(content.contains("rcon.port=25575"));
        assert!(content.contains("rcon.password=secret"));
        assert!(content.contains("max-players=20"));
    }

    #[test]
    fn a_server_is_created_with_an_offline_property_and_a_pid_file() {
        let directory = std::env::temp_dir().join(format!("aether-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let paths = test_paths(&directory);

        let server = create(
            &paths,
            "Test Server",
            0,
            Some("Aether Local"),
            10,
            false,
            "1.21.11",
            "0.20.1",
        )
        .expect("the server was created");

        assert!(server.port > 0);
        assert!(server.rcon_port > 0);
        assert!(!server.running);

        let folder = PathBuf::from(&server.directory);
        assert!(folder.join("server.properties").exists());
        assert!(folder.join("eula.txt").exists());
        assert!(folder.join(METADATA).exists());

        let properties = fs::read_to_string(folder.join("server.properties")).unwrap();
        assert!(properties.contains("online-mode=false"));

        let list = list(&paths);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Test Server");
        assert!(!list[0].running);

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_property_is_replaced_without_touching_the_rest() {
        let directory = std::env::temp_dir().join(format!("aether-props-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("server.properties");

        fs::write(&file, "server-port=25565\nonline-mode=false\nmotd=Test\n").unwrap();
        set_property(&file, "online-mode", "true").unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("online-mode=true"));
        assert!(content.contains("motd=Test"));
        assert!(content.contains("server-port=25565"));

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn an_rcon_packet_has_the_length_it_announces() {
        let packet = rcon_packet(7, 3, "hello");
        let length = i32::from_le_bytes([packet[0], packet[1], packet[2], packet[3]]);

        assert_eq!(length as usize, packet.len() - 4);
        assert_eq!(&packet[4..8], &7i32.to_le_bytes());
        assert_eq!(&packet[8..12], &3i32.to_le_bytes());
        assert_eq!(&packet[12..17], b"hello");
        assert_eq!(&packet[17..19], &[0, 0]);
    }

    #[test]
    fn the_rcon_client_authenticates_and_sends_the_command() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().unwrap().port();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("a connection");

            // The authentication packet.
            let mut length = [0u8; 4];
            stream.read_exact(&mut length).unwrap();
            let length = i32::from_le_bytes(length) as usize;
            let mut body = vec![0u8; length];
            stream.read_exact(&mut body).unwrap();
            assert_eq!(&body[8..body.len() - 2], b"secret");
            stream.write_all(&rcon_packet(1, 2, "")).unwrap();

            // The command packet, read before it is answered so that the socket stays open.
            let mut length = [0u8; 4];
            stream.read_exact(&mut length).unwrap();
            let length = i32::from_le_bytes(length) as usize;
            let mut body = vec![0u8; length];
            stream.read_exact(&mut body).unwrap();
            assert_eq!(&body[8..body.len() - 2], b"stop");
            assert_eq!(&body[4..8], &2i32.to_le_bytes());
            stream.write_all(&rcon_packet(2, 0, "Stopping the server")).unwrap();
        });

        let answer = rcon_command(port, "secret", "stop").expect("the command was sent");
        assert_eq!(answer, "Stopping the server");

        handle.join().unwrap();
    }

    #[test]
    fn a_wrong_password_is_reported() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().unwrap().port();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("a connection");
            let mut length = [0u8; 4];
            stream.read_exact(&mut length).unwrap();
            let length = i32::from_le_bytes(length) as usize;
            let mut body = vec![0u8; length];
            stream.read_exact(&mut body).unwrap();
            stream.write_all(&rcon_packet(-1, 2, "")).unwrap();
        });

        let error = rcon_command(port, "wrong", "stop").expect_err("the password was refused");
        assert!(error.contains("refused"));

        handle.join().unwrap();
    }

    #[test]
    fn the_password_is_random_and_long_enough() {
        let first = random_password().expect("the OS random source is available");
        let second = random_password().expect("the OS random source is available");

        assert_eq!(first.len(), 48);
        assert_ne!(first, second);
        assert!(first.chars().all(|character| character.is_ascii_hexdigit()));
    }

    #[test]
    fn the_server_listens_on_the_loopback_only() {
        let content = properties("Test", 25565, 25575, "secret", 10);

        assert!(content.contains("\nserver-ip=127.0.0.1\n"));
        assert!(!content.contains("server-ip=\n"), "the server would listen on 0.0.0.0");
    }

    #[test]
    fn a_newline_in_the_motd_cannot_add_a_property() {
        let content = properties(
            "hello\nenable-rcon=false\r\nlevel-name=../../Windows",
            25565,
            25575,
            "secret",
            10,
        );

        assert_eq!(
            content.lines().filter(|line| line.starts_with("motd=")).count(),
            1
        );
        assert_eq!(
            content
                .lines()
                .filter(|line| line.starts_with("enable-rcon="))
                .count(),
            1,
            "the name added a second enable-rcon property"
        );
        assert!(content.contains("enable-rcon=true"));
        assert!(content.contains("level-name=world"));
        // The text is still there, but only as a value of the motd: it starts no property.
        assert!(content.contains("motd=hello enable-rcon=false  level-name=../../Windows\n"));
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_not_a_jar() {
        let directory = std::env::temp_dir().join(format!("aether-jar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let file = directory.join("server.jar");
        fs::write(&file, b"<html>404 not found</html>").unwrap();
        assert!(!looks_like_jar(&file));

        fs::write(&file, b"PK\x03\x04the rest of a zip").unwrap();
        assert!(looks_like_jar(&file));

        assert!(!looks_like_jar(&directory.join("missing.jar")));

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_recorded_digest_is_only_kept_when_it_is_a_sha1() {
        let directory = std::env::temp_dir().join(format!("aether-digest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let file = directory.join(INSTALLER_DIGEST);
        fs::write(&file, "  DA39A3EE5E6B4B0D3255BFEF95601890AFD80709\n").unwrap();
        assert_eq!(
            recorded_digest(&file).as_deref(),
            Some("da39a3ee5e6b4b0d3255bfef95601890afd80709")
        );

        fs::write(&file, "<html>404</html>").unwrap();
        assert!(recorded_digest(&file).is_none());

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_process_that_does_not_exist_is_not_alive() {
        assert!(!process_alive(0));
        assert!(!process_alive(u32::MAX));
    }
}
