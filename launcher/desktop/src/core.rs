//! Runs the Rust core of the launcher and turns its JSON lines into Tauri events.
//!
//! The interface never talks to Minecraft directly: it calls a command here, the command runs
//! `aether-launcher.exe` and answers with the last JSON document the core printed. The streaming
//! commands (installation, game, Microsoft login, optimisation pack) forward every typed event to
//! the window, exactly like the Electron shell did.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// The core is embedded in the shell: a portable build is therefore a single executable, and the
/// installer only has to carry one file. It is written next to the data of the launcher on the
/// first start (and replaced whenever it differs from the embedded bytes).
static EMBEDDED_CORE: &[u8] = include_bytes!("../../target/release/aether-launcher.exe");

/// The client mod is embedded as well. The core looks for `aether-client.jar` next to itself and in
/// the folder of the project, which an installed launcher on another machine has neither of. The
/// archive is written once into the data directory and handed to the core through `AETHER_MOD_JAR`,
/// so that "Install / repair" works on a machine that never saw the sources.
static EMBEDDED_MOD: &[u8] = include_bytes!("../../../aether-client/build/libs/aether-client-1.0.0.jar");

const CORE_NAME: &str = "aether-launcher.exe";
const MOD_NAME: &str = "aether-client.jar";

/// `CREATE_NO_WINDOW`: the core and `taskkill` must never flash a console over the interface.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Tells the core where the shell lives, and which client mod to install.
///
/// The embedded core runs from the application data of the launcher, so on its own it could not find
/// an installation delivered next to the portable executable. The anchor is the folder of the shell.
/// The same applies to the mod: it is extracted next to that core and given to it, because a machine
/// that only received the installer has no `aether-client.jar` anywhere else.
pub fn anchored<'a>(app: &AppHandle, command: &'a mut Command) -> &'a mut Command {
	if std::env::var("AETHER_ANCHOR").map(|value| value.is_empty()).unwrap_or(true) {
		if let Ok(executable) = std::env::current_exe() {
			if let Some(directory) = executable.parent() {
				command.env("AETHER_ANCHOR", directory);
			}
		}
	}

	if std::env::var("AETHER_MOD_JAR").map(|value| value.is_empty()).unwrap_or(true) {
		if let Ok(path) = extract_mod(app) {
			command.env("AETHER_MOD_JAR", path);
		}
	}

	command
}

/// Hides the console of a child process on Windows.
pub fn hidden(command: &mut Command) -> &mut Command {
	#[cfg(windows)]
	{
		use std::os::windows::process::CommandExt;
		command.creation_flags(CREATE_NO_WINDOW);
	}
	command
}

/// Kills a process and its children.
///
/// The core starts Java as a child: stopping the core alone would leave the game running.
pub fn kill_tree(pid: u32) {
	#[cfg(windows)]
	{
		let mut command = Command::new("taskkill");
		command
			.args(["/PID", &pid.to_string(), "/T", "/F"])
			.stdout(Stdio::null())
			.stderr(Stdio::null());
		hidden(&mut command);
		let _ = command.status();
	}

	#[cfg(not(windows))]
	{
		let _ = Command::new("kill").args(["-9", &format!("-{pid}")]).status();
		let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
	}
}

/// Finds the core: the override of the environment first, then next to the shell, then the build
/// directory during development, then the resources of the bundle, then the embedded copy.
pub fn find_core(app: &AppHandle) -> Result<PathBuf, String> {
	if let Ok(value) = std::env::var("AETHER_LAUNCHER_BIN") {
		let path = PathBuf::from(&value);
		if path.is_file() {
			return Ok(path);
		}
	}

	if let Ok(executable) = std::env::current_exe() {
		if let Some(directory) = executable.parent() {
			let candidate = directory.join(CORE_NAME);
			if candidate.is_file() {
				return Ok(candidate);
			}
		}
	}

	// A development build must run the core that was just compiled, not the copy extracted from
	// the previous one.
	#[cfg(debug_assertions)]
	{
		let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
		for profile in ["debug", "release"] {
			let candidate = manifest
				.join("..")
				.join("target")
				.join(profile)
				.join(CORE_NAME);
			if candidate.is_file() {
				return Ok(candidate);
			}
		}
	}

	if let Ok(directory) = app.path().resource_dir() {
		let candidate = directory.join(CORE_NAME);
		if candidate.is_file() {
			return Ok(candidate);
		}
	}

	extract_core(app)
}

/// Writes the embedded core into the data directory of the launcher.
fn extract_core(app: &AppHandle) -> Result<PathBuf, String> {
	let directory = app
		.path()
		.app_local_data_dir()
		.map_err(|error| error.to_string())?
		.join("bin");
	std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;

	let path = directory.join(CORE_NAME);
	let current = std::fs::read(&path).unwrap_or_default();
	if current != EMBEDDED_CORE {
		let temporary = directory.join(format!("{CORE_NAME}.new"));
		std::fs::write(&temporary, EMBEDDED_CORE).map_err(|error| error.to_string())?;

		if std::fs::rename(&temporary, &path).is_err() {
			// An older core is still running and Windows locks the file. It works, so it is better
			// to start with it than to refuse to open the launcher.
			let _ = std::fs::remove_file(&temporary);
			if !path.is_file() {
				return Err("the launcher core could not be written to the disk.".to_string());
			}
		}
	}

	Ok(path)
}

/// Writes the embedded client mod into the data directory of the launcher.
fn extract_mod(app: &AppHandle) -> Result<PathBuf, String> {
	let directory = app
		.path()
		.app_local_data_dir()
		.map_err(|error| error.to_string())?
		.join("mods");
	write_embedded_mod(&directory, EMBEDDED_MOD)
}

/// Writes `bytes` to `directory/MOD_NAME`, replacing an older copy.
///
/// Kept separate from the app handle so that the replacement can be tested without a window.
fn write_embedded_mod(directory: &std::path::Path, bytes: &[u8]) -> Result<PathBuf, String> {
	std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;

	let path = directory.join(MOD_NAME);
	let current = std::fs::read(&path).unwrap_or_default();
	if current == bytes {
		return Ok(path);
	}

	let temporary = directory.join(format!("{MOD_NAME}.new"));
	std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;

	// Unlike the core, the archive is not a running executable: nothing locks it, and the rename
	// replaces the previous version in one step. The rewrite below only covers a file system that
	// refuses the rename, for instance when the archive is open in a file manager.
	if std::fs::rename(&temporary, &path).is_err() {
		let _ = std::fs::remove_file(&temporary);
		let _ = std::fs::remove_file(&path);
		std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
	}

	Ok(path)
}

/// The time the core has to answer before it is considered stuck.
///
/// The commands that read the network are given more room, the local ones only need a moment.
fn timeout_for(args: &[String]) -> u64 {
	match args.first().map(String::as_str) {
		Some("versions") | Some("loaders") => 45,
		Some("mods") if args.get(1).map(String::as_str) == Some("search") => 45,
		// A mod may pull several required dependencies, each with a verified resumable download.
		Some("mods") if args.get(1).map(String::as_str) == Some("install") => 900,
		Some("servers") => 30,
		// The first start of a hosted server installs Fabric and the vanilla server.
		Some("hosts") if args.get(1).map(String::as_str) == Some("start") => 900,
		Some("performance") if args.get(1).map(String::as_str) == Some("install") => 900,
		_ => 20,
	}
}

fn read_stream<R>(reader: Option<R>) -> std::thread::JoinHandle<String>
where
	R: Read + Send + 'static,
{
	std::thread::spawn(move || {
		let mut buffer = String::new();
		if let Some(mut reader) = reader {
			let _ = reader.read_to_string(&mut buffer);
		}
		buffer
	})
}

/// The last JSON document of an output: the core prints one document per line.
fn last_document(output: &str) -> Option<Value> {
	let mut document = None;
	for line in output.lines() {
		let line = line.trim();
		if line.is_empty() {
			continue;
		}
		if let Ok(value) = serde_json::from_str::<Value>(line) {
			if value.is_object() {
				document = Some(value);
			}
		}
	}
	document
}

/// Runs the core and waits for its answer, with the same timeout as the Electron shell.
pub fn run_core_blocking(app: &AppHandle, args: &[String]) -> Value {
	let binary = match find_core(app) {
		Ok(path) => path,
		Err(error) => return json!({ "ok": false, "error": error }),
	};

	let mut command = Command::new(&binary);
	command
		.args(args)
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	anchored(app, &mut command);
	hidden(&mut command);

	let mut child = match command.spawn() {
		Ok(child) => child,
		Err(error) => return json!({ "ok": false, "error": error.to_string() }),
	};

	let seconds = timeout_for(args);
	let done = Arc::new(AtomicBool::new(false));
	let timed_out = Arc::new(AtomicBool::new(false));
	let watchdog = {
		let done = done.clone();
		let timed_out = timed_out.clone();
		let pid = child.id();
		std::thread::spawn(move || {
			let deadline = Instant::now() + Duration::from_secs(seconds);
			while Instant::now() < deadline {
				if done.load(Ordering::SeqCst) {
					return;
				}
				std::thread::sleep(Duration::from_millis(150));
			}
			timed_out.store(true, Ordering::SeqCst);
			kill_tree(pid);
		})
	};

	let stdout = read_stream(child.stdout.take());
	let stderr = read_stream(child.stderr.take());
	let _ = child.wait();
	done.store(true, Ordering::SeqCst);
	let _ = watchdog.join();

	if timed_out.load(Ordering::SeqCst) {
		return json!({
			"ok": false,
			"error": format!("The launcher core did not answer within {seconds} s.")
		});
	}

	let output = format!(
		"{}\n{}",
		stdout.join().unwrap_or_default(),
		stderr.join().unwrap_or_default()
	);

	last_document(&output).unwrap_or_else(|| {
		json!({
			"ok": false,
			"error": if output.trim().is_empty() {
				"The launcher core did not answer.".to_string()
			} else {
				output.trim().to_string()
			}
		})
	})
}

/// Runs the core on a thread of its own so that the interface stays responsive.
pub async fn run_core(app: AppHandle, args: Vec<String>) -> Value {
	blocking(move || run_core_blocking(&app, &args))
		.await
		.unwrap_or_else(|| {
			json!({ "ok": false, "error": "The launcher core stopped unexpectedly." })
		})
}

/// Awaits a blocking task without tying down a worker of the async runtime.
pub async fn blocking<T, F>(task: F) -> Option<T>
where
	T: Send + 'static,
	F: FnOnce() -> T + Send + 'static,
{
	let (sender, receiver) = tokio::sync::oneshot::channel();
	std::thread::spawn(move || {
		let _ = sender.send(task());
	});
	receiver.await.ok()
}

/// The streaming commands, each one with the event it forwards.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
	Install,
	Game,
	Microsoft,
	Optimisation,
	Modrinth,
}

fn emit_event(app: &AppHandle, kind: StreamKind, value: &Value) {
	let event = match kind {
		StreamKind::Install => "install:progress",
		StreamKind::Game => "game:output",
		StreamKind::Microsoft => "microsoft:deviceCode",
		StreamKind::Optimisation => "optimisation:progress",
		StreamKind::Modrinth => "mods:progress",
	};

	// The interface of the game only needs the text of the line, the other panels want the whole
	// event (percentage, detail, code, ...).
	let payload = match kind {
		StreamKind::Game => Value::String(
			value
				.get("line")
				.and_then(Value::as_str)
				.unwrap_or_default()
				.to_string(),
		),
		_ => value.clone(),
	};

	let _ = app.emit(event, payload);
}

fn read_events<R>(
	reader: R,
	app: AppHandle,
	kind: StreamKind,
	document: Arc<Mutex<Option<Value>>>,
) -> std::thread::JoinHandle<()>
where
	R: BufRead + Send + 'static,
{
	std::thread::spawn(move || {
		for line in reader.lines() {
			let Ok(line) = line else { break };
			let trimmed = line.trim();
			if trimmed.is_empty() {
				continue;
			}

			match serde_json::from_str::<Value>(trimmed) {
				// A line with a type is an event, every other document is the answer.
				Ok(value) if value.is_object() && value.get("type").is_some() => {
					emit_event(&app, kind, &value);
				}
				Ok(value) if value.is_object() => {
					*document.lock().unwrap() = Some(value);
				}
				_ => {
					// The core also forwards the raw output of Java, which is not JSON.
					if kind == StreamKind::Game {
						let _ = app.emit("game:output", line);
					}
				}
			}
		}
	})
}

/// Runs the core and forwards every line it prints.
///
/// `on_spawn` receives the process identifier and the handle of its standard input, so that the
/// installation can be cancelled and the game can be stopped while they run.
pub fn run_core_streaming(
	app: &AppHandle,
	args: &[String],
	kind: StreamKind,
	on_spawn: impl FnOnce(u32, Option<ChildStdin>),
) -> Value {
	let binary = match find_core(app) {
		Ok(path) => path,
		Err(error) => return json!({ "ok": false, "error": error }),
	};

	let mut command = Command::new(&binary);
	command
		.args(args)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	anchored(app, &mut command);
	hidden(&mut command);

	let mut child = match command.spawn() {
		Ok(child) => child,
		Err(error) => return json!({ "ok": false, "error": error.to_string() }),
	};

	let pid = child.id();
	let stdin = child.stdin.take();
	on_spawn(pid, stdin);

	let document = Arc::new(Mutex::new(None));
	let stdout = match child.stdout.take() {
		Some(stdout) => read_events(BufReader::new(stdout), app.clone(), kind, document.clone()),
		None => std::thread::spawn(|| {}),
	};
	let stderr = match child.stderr.take() {
		Some(stderr) => read_events(BufReader::new(stderr), app.clone(), kind, document.clone()),
		None => std::thread::spawn(|| {}),
	};

	let status = child.wait();
	let _ = stdout.join();
	let _ = stderr.join();

	let answer = document.lock().unwrap().take();
	match answer {
		Some(value) => value,
		None => match status {
			Ok(status) if status.success() => json!({ "ok": true }),
			Ok(status) => json!({
				"ok": false,
				"crashed": true,
				"exitCode": status.code(),
				"error": format!(
					"The launcher core stopped with the code {}.",
					status.code().map(|code| code.to_string()).unwrap_or_else(|| "?".to_string())
				)
			}),
			Err(error) => json!({ "ok": false, "error": error.to_string() }),
		},
	}
}

/// Writes a line on the standard input of a running core.
pub fn write_line(stdin: &mut ChildStdin, line: &str) -> std::io::Result<()> {
	stdin.write_all(format!("{line}\n").as_bytes())?;
	stdin.flush()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn args(values: &[&str]) -> Vec<String> {
		values.iter().map(|value| value.to_string()).collect()
	}

	#[test]
	fn the_last_document_of_the_output_wins() {
		let output = "{\"ok\":false,\"stage\":\"metadata\"}\n\
			{\"ok\":true,\"stage\":\"done\"}\n";
		assert_eq!(last_document(output).unwrap()["stage"], serde_json::json!("done"));
	}

	#[test]
	fn a_raw_line_is_not_a_document() {
		// The core forwards the output of Java on the same stream as its own documents.
		let output = "[23:36:46] [Render thread/INFO]: Connecting to 127.0.0.1\n";
		assert!(last_document(output).is_none());
	}

	#[test]
	fn the_embedded_mod_is_a_jar() {
		// A truncated or empty inclusion would only fail on the target machine, at install time.
		assert!(EMBEDDED_MOD.len() > 100_000);
		assert_eq!(&EMBEDDED_MOD[..4], b"PK\x03\x04");
	}

	#[test]
	fn the_mod_is_written_and_replaced_atomically() {
		let directory =
			std::env::temp_dir().join(format!("aether-mod-test-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&directory);
		let temporary = directory.join(format!("{MOD_NAME}.new"));

		let path = write_embedded_mod(&directory, b"first").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"first");

		// Same bytes: the archive is left alone and no temporary file is written.
		let path = write_embedded_mod(&directory, b"first").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"first");
		assert!(!temporary.exists());

		// New bytes: the archive is replaced, and nothing is left behind.
		let path = write_embedded_mod(&directory, b"second").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"second");
		assert!(!temporary.exists());

		let _ = std::fs::remove_dir_all(&directory);
	}

	#[test]
	fn the_timeouts_follow_the_network() {
		assert_eq!(timeout_for(&args(&["versions"])), 45);
		assert_eq!(timeout_for(&args(&["loaders", "--version", "1.21.11"])), 45);
		assert_eq!(timeout_for(&args(&["servers", "probe", "mc.hypixel.net"])), 30);
		// Only the first start of a server downloads and installs, the list is local.
		assert_eq!(timeout_for(&args(&["hosts", "start", "Local Test"])), 900);
		assert_eq!(timeout_for(&args(&["hosts", "list"])), 20);
		assert_eq!(timeout_for(&args(&["mods", "install", "sodium", "--game-version", "1.21.11"])), 900);
		assert_eq!(timeout_for(&args(&["performance", "install"])), 900);
		assert_eq!(timeout_for(&args(&["performance", "list"])), 20);
		assert_eq!(timeout_for(&args(&["info"])), 20);
	}
}
