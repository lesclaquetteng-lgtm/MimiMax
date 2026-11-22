//! The server list, the status ping and the login probe.
//!
//! The launcher speaks the protocol of Minecraft itself instead of shelling out to a tool:
//!
//! * the **status ping** (`Server List Ping`) reads the version, the number of players and the MOTD
//!   that the game shows in its own list;
//! * the **login probe** opens the login state and reads the first answer of the server. An
//!   `Encryption Request` means that the server runs in online mode and only accepts premium
//!   accounts, while a `Login Success` or a `Set Compression` means that the server accepts offline
//!   players.
//!
//! The probe never authenticates and never joins: it reads the first packet and closes the socket.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The packet identifiers of the handshake and of the status state.
const PACKET_HANDSHAKE: i32 = 0x00;
const PACKET_STATUS_REQUEST: i32 = 0x00;
const PACKET_STATUS_RESPONSE: i32 = 0x00;

/// The packet identifiers of the login state.
const PACKET_LOGIN_START: i32 = 0x00;
const PACKET_ENCRYPTION_REQUEST: i32 = 0x01;
const PACKET_LOGIN_SUCCESS: i32 = 0x02;
const PACKET_SET_COMPRESSION: i32 = 0x03;

/// The first protocol version where the login start packet carries the UUID of the player.
const PROTOCOL_LOGIN_START_UUID: i32 = 764;

/// The protocol version of Minecraft 1.21.11, used when the server does not report one.
pub const FALLBACK_PROTOCOL: i32 = 774;

/// How long a connection may take to answer.
const TIMEOUT: Duration = Duration::from_secs(5);

/// A server of the list of the player.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerEntry {
    /// The name the player gave it.
    pub name: String,
    /// The address, `host` or `host:port`.
    pub address: String,
}

/// The server list, stored in `servers.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ServerStore {
    pub servers: Vec<ServerEntry>,
}

impl ServerStore {
    /// Reads the list, a missing or broken file produces an empty list.
    pub fn load(path: &Path) -> ServerStore {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => ServerStore::default(),
        }
    }

    /// Writes the list in one step.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|error| io::Error::other(error.to_string()))?;
        crate::settings::write_atomic(path, &content)
    }

    /// Adds a server, an address that is already listed is replaced.
    pub fn add(&mut self, name: &str, address: &str) -> Result<ServerEntry, String> {
        let address = address.trim().to_string();
        if address.is_empty() {
            return Err("the address is empty".to_string());
        }

        let name = match name.trim() {
            "" => address.clone(),
            other => other.to_string(),
        };

        let entry = ServerEntry { name, address };
        self.servers
            .retain(|existing| !existing.address.eq_ignore_ascii_case(&entry.address));
        self.servers.push(entry.clone());
        Ok(entry)
    }

    /// Removes a server, by address or by name.
    pub fn remove(&mut self, key: &str) -> Result<ServerEntry, String> {
        let position = self
            .servers
            .iter()
            .position(|entry| {
                entry.address.eq_ignore_ascii_case(key) || entry.name.eq_ignore_ascii_case(key)
            })
            .ok_or_else(|| format!("unknown server: {}", key))?;

        Ok(self.servers.remove(position))
    }

    /// The address of a server, by name or by address. An address that is not listed is returned
    /// unchanged, so that the interface can join a server without adding it first.
    pub fn address_of(&self, key: &str) -> String {
        self.servers
            .iter()
            .find(|entry| {
                entry.name.eq_ignore_ascii_case(key) || entry.address.eq_ignore_ascii_case(key)
            })
            .map(|entry| entry.address.clone())
            .unwrap_or_else(|| key.to_string())
    }
}

/// Splits `host`, `host:port`, `[v6]` and `[v6]:port` into an address and a port.
pub fn split_address(address: &str) -> (String, u16) {
    let trimmed = address.trim();

    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            let port = tail
                .strip_prefix(':')
                .and_then(|value| value.parse().ok())
                .unwrap_or(25565);
            return (host.to_string(), port);
        }
    }

    if trimmed.matches(':').count() > 1 && !trimmed.starts_with('[') {
        // A bare IPv6 address, a port needs the bracket form.
        return (trimmed.to_string(), 25565);
    }

    if let Some((host, port)) = trimmed.rsplit_once(':') {
        if !host.is_empty() && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(port) = port.parse() {
                return (host.to_string(), port);
            }
        }
    }

    (trimmed.to_string(), 25565)
}

/// The status of a server, the values the game shows in its list.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResult {
    pub address: String,
    pub online: bool,
    pub players_online: i64,
    pub players_max: i64,
    pub version_name: String,
    pub protocol: i32,
    pub motd: String,
    pub latency_ms: u64,
    pub error: Option<String>,
}

/// A failed probe, the reason is written for the player.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginProbe {
    /// `true` when the server answered with a packet that means "you are in".
    pub accepted: bool,
    /// `true` when the server asked for the encryption of a premium session.
    pub premium: bool,
    /// The packet the server answered with, in words.
    pub packet: String,
    /// The reason of a refusal, already readable.
    pub detail: String,
    /// The protocol version that was used for the probe.
    pub protocol: i32,
}

impl LoginProbe {
    /// A short verdict for the interface.
    pub fn verdict(&self) -> &'static str {
        if self.accepted {
            "offline"
        } else if self.premium {
            "premium"
        } else {
            "refused"
        }
    }
}

/// Reads a VarInt from a stream.
pub fn read_varint<R: Read>(reader: &mut R) -> io::Result<i32> {
    let mut value: u32 = 0;

    for index in 0..5 {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        value |= ((byte[0] & 0x7F) as u32) << (7 * index);

        if byte[0] & 0x80 == 0 {
            return Ok(value as i32);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "the VarInt is longer than five bytes",
    ))
}

/// Writes a VarInt to a buffer.
pub fn write_varint(value: i32, out: &mut Vec<u8>) {
    let mut value = value as u32;

    loop {
        if value & !0x7F == 0 {
            out.push(value as u8);
            return;
        }

        out.push(((value & 0x7F) | 0x80) as u8);
        value >>= 7;
    }
}

/// Writes a length-prefixed UTF-8 string.
fn write_string(value: &str, out: &mut Vec<u8>) {
    write_varint(value.len() as i32, out);
    out.extend_from_slice(value.as_bytes());
}

/// Reads a length-prefixed UTF-8 string from a buffer.
fn read_string(reader: &mut impl Read) -> io::Result<String> {
    let length = read_varint(reader)?;
    if !(0..=1_048_576).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the string length is out of range",
        ));
    }

    let mut buffer = vec![0u8; length as usize];
    reader.read_exact(&mut buffer)?;
    Ok(String::from_utf8_lossy(&buffer).to_string())
}

/// Frames a packet: its length, then its body.
fn frame(body: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 5);
    write_varint(body.len() as i32, &mut out);
    out.extend_from_slice(&body);
    out
}

/// Writes a packet with an identifier and no field.
fn empty_packet(identifier: i32) -> Vec<u8> {
    let mut body = Vec::new();
    write_varint(identifier, &mut body);
    frame(body)
}

/// Opens a TCP connection with a timeout.
fn connect(host: &str, port: u16) -> io::Result<TcpStream> {
    let addresses = (host, port).to_socket_addrs()?;
    let mut last: Option<io::Error> = None;

    for address in addresses {
        match TcpStream::connect_timeout(&address, TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = Some(error),
        }
    }

    Err(last.unwrap_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the host has no address")))
}

/// Reads one framed packet and returns its body.
fn read_packet(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let length = read_varint(stream)?;
    if !(0..=2 * 1_048_576).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the packet length is out of range",
        ));
    }

    let mut body = vec![0u8; length as usize];
    stream.read_exact(&mut body)?;
    Ok(body)
}

/// The handshake that starts the status or the login state.
fn handshake(address: &str, port: u16, protocol: i32, next_state: i32) -> Vec<u8> {
    let mut body = Vec::new();
    write_varint(PACKET_HANDSHAKE, &mut body);
    write_varint(protocol, &mut body);
    write_string(address, &mut body);
    body.extend_from_slice(&port.to_be_bytes());
    write_varint(next_state, &mut body);
    frame(body)
}

/// The login start packet, with the UUID that recent protocols expect.
fn login_start(username: &str, protocol: i32) -> Vec<u8> {
    let mut body = Vec::new();
    write_varint(PACKET_LOGIN_START, &mut body);
    write_string(username, &mut body);

    if protocol >= PROTOCOL_LOGIN_START_UUID {
        // The UUID is sent as sixteen bytes. The identifier only has to look like one, the server
        // decides the real identity of an offline player, so a constant is enough for a probe.
        let uuid = uuid_bytes("5627dd98-e6be-3c21-b8a8-e92344183641");
        body.extend_from_slice(&uuid);
    }

    frame(body)
}

/// Reads the sixteen bytes of a UUID.
fn uuid_bytes(uuid: &str) -> [u8; 16] {
    let hex: String = uuid.chars().filter(|c| *c != '-').collect();
    let mut bytes = [0u8; 16];

    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        if start + 2 <= hex.len() {
            *byte = u8::from_str_radix(&hex[start..start + 2], 16).unwrap_or(0);
        }
    }

    bytes
}

/// Pings a server and reads its status.
pub fn ping(address: &str) -> PingResult {
    let (host, port) = split_address(address);
    let started = Instant::now();

    let mut result = PingResult {
        address: address.to_string(),
        protocol: FALLBACK_PROTOCOL,
        ..PingResult::default()
    };

    let outcome = (|| -> io::Result<PingResult> {
        let mut stream = connect(&host, port)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;

        let mut request = handshake(&host, port, FALLBACK_PROTOCOL, 1);
        request.extend_from_slice(&empty_packet(PACKET_STATUS_REQUEST));
        stream.write_all(&request)?;

        let body = read_packet(&mut stream)?;
        let latency = started.elapsed().as_millis() as u64;

        let mut reader = std::io::Cursor::new(body);
        let identifier = read_varint(&mut reader)?;
        if identifier != PACKET_STATUS_RESPONSE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected status packet 0x{:02X}", identifier),
            ));
        }

        let json = read_string(&mut reader)?;
        let value: serde_json::Value = serde_json::from_str(&json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;

        Ok(parse_status(&value, latency))
    })();

    match outcome {
        Ok(ping) => {
            result.online = true;
            result.players_online = ping.players_online;
            result.players_max = ping.players_max;
            result.version_name = ping.version_name;
            result.protocol = ping.protocol;
            result.motd = ping.motd;
            result.latency_ms = ping.latency_ms;
        }
        Err(error) => {
            result.error = Some(error.to_string());
            result.latency_ms = started.elapsed().as_millis() as u64;
        }
    }

    result
}

/// Reads the fields of a status document.
fn parse_status(value: &serde_json::Value, latency_ms: u64) -> PingResult {
    let version = value.get("version");
    let players = value.get("players");

    PingResult {
        address: String::new(),
        online: true,
        players_online: players
            .and_then(|value| value.get("online"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        players_max: players
            .and_then(|value| value.get("max"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        version_name: version
            .and_then(|value| value.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        protocol: version
            .and_then(|value| value.get("protocol"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(FALLBACK_PROTOCOL as i64) as i32,
        motd: readable_motd(value.get("description")),
        latency_ms,
        error: None,
    }
}

/// Flattens a chat component into one line of text.
pub fn flatten_component(value: &serde_json::Value) -> String {
    let mut text = String::new();
    collect_component(value, &mut text, 0);
    text
}

fn collect_component(value: &serde_json::Value, out: &mut String, depth: usize) {
    if depth > 8 {
        return;
    }

    match value {
        serde_json::Value::String(text) => out.push_str(text),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_component(item, out, depth + 1);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(text)) = map.get("text") {
                out.push_str(text);
            }
            if let Some(extra) = map.get("extra") {
                collect_component(extra, out, depth + 1);
            }
            if let Some(serde_json::Value::Array(arguments)) = map.get("with") {
                collect_component(&serde_json::Value::Array(arguments.clone()), out, depth + 1);
            }
        }
        _ => {}
    }
}

/// Removes the `§` formatting codes and turns the MOTD into one readable line.
pub fn strip_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars();

    while let Some(character) = characters.next() {
        if character == '§' {
            // The code that follows is consumed, whichever it is.
            characters.next();
            continue;
        }

        out.push(match character {
            '\n' | '\r' => ' ',
            other => other,
        });
    }

    out.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// The description of a status document, ready to be displayed.
fn readable_motd(description: Option<&serde_json::Value>) -> String {
    match description {
        Some(value) => strip_formatting(&flatten_component(value)),
        None => String::new(),
    }
}

/// Probes the login state of a server to know whether it accepts offline accounts.
///
/// The protocol version of the server is read first, so the probe speaks the same version as the
/// server instead of guessing. An `Encryption Request` means online mode: only a premium session
/// can answer it, an offline account cannot.
pub fn probe_login(address: &str, username: &str) -> LoginProbe {
    let status = ping(address);
    let protocol = if status.online {
        status.protocol
    } else {
        FALLBACK_PROTOCOL
    };

    let (host, port) = split_address(address);

    let outcome = (|| -> io::Result<LoginProbe> {
        let mut stream = connect(&host, port)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;

        let mut request = handshake(&host, port, protocol, 2);
        request.extend_from_slice(&login_start(username, protocol));
        stream.write_all(&request)?;

        let body = read_packet(&mut stream)?;
        let mut reader = std::io::Cursor::new(body);
        let identifier = read_varint(&mut reader)?;

        let (accepted, premium, packet) = match identifier {
            PACKET_ENCRYPTION_REQUEST => (false, true, "encryption request"),
            PACKET_LOGIN_SUCCESS => (true, false, "login success"),
            PACKET_SET_COMPRESSION => (true, false, "set compression"),
            PACKET_LOGIN_START => (false, false, "login start"),
            id => (
                false,
                false,
                if id == 0x00 { "disconnect" } else { "unknown" },
            ),
        };

        let detail = if identifier == 0x00 {
            let reason = read_string(&mut reader).unwrap_or_default();
            let component: serde_json::Value =
                serde_json::from_str(&reason).unwrap_or(serde_json::Value::String(reason));
            let text = strip_formatting(&flatten_component(&component));

            let lowered = text.to_ascii_lowercase();
            let premium = lowered.contains("online mode")
                || lowered.contains("authenticat")
                || lowered.contains("premium")
                || lowered.contains("mojang");

            return Ok(LoginProbe {
                accepted: false,
                premium,
                packet: "disconnect".to_string(),
                detail: if text.is_empty() {
                    "the server refused the login".to_string()
                } else {
                    text
                },
                protocol,
            });
        } else {
            if accepted {
                "the server accepts offline players".to_string()
            } else if premium {
                "the server runs in online mode, a Microsoft account is required".to_string()
            } else {
                format!("the server answered with {}", packet)
            }
        };

        Ok(LoginProbe {
            accepted,
            premium,
            packet: packet.to_string(),
            detail,
            protocol,
        })
    })();

    outcome.unwrap_or_else(|error| LoginProbe {
        accepted: false,
        premium: false,
        packet: "unreachable".to_string(),
        detail: error.to_string(),
        protocol,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        for value in [
            0,
            1,
            127,
            128,
            255,
            300,
            2_097_151,
            i32::MAX,
            -1,
            -2_147_483_648,
        ] {
            let mut buffer = Vec::new();
            write_varint(value, &mut buffer);

            let mut reader = std::io::Cursor::new(buffer.clone());
            assert_eq!(read_varint(&mut reader).unwrap(), value, "for {}", value);
        }
    }

    #[test]
    fn a_varint_is_never_longer_than_five_bytes() {
        let mut buffer = Vec::new();
        write_varint(i32::MAX, &mut buffer);
        assert_eq!(buffer.len(), 5);
    }

    #[test]
    fn the_address_of_a_server_is_split() {
        assert_eq!(
            split_address("mc.example.com"),
            ("mc.example.com".to_string(), 25565)
        );
        assert_eq!(
            split_address("mc.example.com:25566"),
            ("mc.example.com".to_string(), 25566)
        );
        assert_eq!(split_address(" [::1]:25567 "), ("::1".to_string(), 25567));
        assert_eq!(split_address("::1"), ("::1".to_string(), 25565));
        assert_eq!(split_address(""), (String::new(), 25565));
    }

    #[test]
    fn a_motd_is_flattened_and_stripped() {
        let motd = serde_json::json!({
            "text": "§6Aether ",
            "extra": [{ "text": "§7Network\n" }, { "text": "§fWelcome" }]
        });

        assert_eq!(
            strip_formatting(&flatten_component(&motd)),
            "Aether Network Welcome"
        );
    }

    #[test]
    fn a_plain_string_motd_is_read() {
        assert_eq!(
            strip_formatting(&flatten_component(&serde_json::json!("§aHello"))),
            "Hello"
        );
    }

    #[test]
    fn a_status_document_is_read() {
        let document = serde_json::json!({
            "version": { "name": "1.21.11", "protocol": 774 },
            "players": { "online": 12, "max": 100 },
            "description": { "text": "A test server" }
        });

        let status = parse_status(&document, 42);
        assert_eq!(status.version_name, "1.21.11");
        assert_eq!(status.protocol, 774);
        assert_eq!(status.players_online, 12);
        assert_eq!(status.players_max, 100);
        assert_eq!(status.motd, "A test server");
        assert_eq!(status.latency_ms, 42);
        assert!(status.online);
    }

    #[test]
    fn the_login_start_carries_the_uuid_of_recent_protocols() {
        let old = login_start("Steve", 763);
        let recent = login_start("Steve", 774);

        assert!(recent.len() > old.len() + 8, "the UUID is missing");
        // The body of the frame is the handshake, the identifier is the first byte of the body.
        let mut reader = std::io::Cursor::new(&recent);
        let _length = read_varint(&mut reader).unwrap();
        assert_eq!(read_varint(&mut reader).unwrap(), PACKET_LOGIN_START);
        assert_eq!(read_string(&mut reader).unwrap(), "Steve");
    }

    #[test]
    fn a_server_is_added_removed_and_found() {
        let mut store = ServerStore::default();

        store.add("Main", "mc.example.com:25565").unwrap();
        store.add("Second", "second.example.com").unwrap();
        assert_eq!(store.servers.len(), 2);
        assert_eq!(store.address_of("main"), "mc.example.com:25565");
        assert_eq!(store.address_of("SECOND.EXAMPLE.COM"), "second.example.com");
        assert_eq!(
            store.address_of("unknown.example.com"),
            "unknown.example.com"
        );

        // The same address replaces the entry instead of being listed twice.
        store.add("Renamed", "MC.EXAMPLE.COM:25565").unwrap();
        assert_eq!(store.servers.len(), 2);
        assert_eq!(store.address_of("renamed"), "MC.EXAMPLE.COM:25565");

        assert_eq!(
            store.remove("mc.example.com:25565").unwrap().name,
            "Renamed"
        );
        assert!(store.remove("Renamed").is_err());
    }

    #[test]
    fn a_server_list_survives_a_round_trip() {
        let directory = std::env::temp_dir().join(format!("aether-servers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("servers.json");

        let mut store = ServerStore::default();
        store.add("Aether", "localhost:25565").unwrap();
        store.save(&file).unwrap();

        let loaded = ServerStore::load(&file);
        assert_eq!(loaded.servers, store.servers);

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_missing_list_is_empty() {
        let store = ServerStore::load(Path::new("C:/does/not/exist/servers.json"));
        assert!(store.servers.is_empty());
    }
}
