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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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

	write_embedded_file(&directory, CORE_NAME, EMBEDDED_CORE)
		.map_err(|error| format!("the launcher core could not be written to the disk: {error}"))
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

/// Creates collision-resistant temporary siblings for embedded resources.
static NEXT_EMBEDDED_TEMP: AtomicU64 = AtomicU64::new(0);

/// Replaces a file atomically, including when its destination already exists on Windows.
fn replace_embedded_file(temporary: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
	#[cfg(windows)]
	{
		use std::os::windows::ffi::OsStrExt;

		let source: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
		let target: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();

		#[link(name = "Kernel32")]
		extern "system" {
			#[link_name = "MoveFileExW"]
			fn move_file_ex_w(existing_name: *const u16, new_name: *const u16, flags: u32) -> i32;
		}

		// MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
		let replaced = unsafe { move_file_ex_w(source.as_ptr(), target.as_ptr(), 0x1 | 0x8) };
		if replaced == 0 {
			return Err(std::io::Error::last_os_error());
		}
		Ok(())
	}

	#[cfg(not(windows))]
	{
		std::fs::rename(temporary, destination)
	}
}

/// Writes an embedded executable or JAR without exposing a partially-written destination.
/// If Windows keeps an older executable locked, its existing complete copy remains usable.
fn write_embedded_file(
	directory: &std::path::Path,
	filename: &str,
	bytes: &[u8],
) -> std::io::Result<PathBuf> {
	use std::fs::OpenOptions;

	std::fs::create_dir_all(directory)?;
	let destination = directory.join(filename);
	if std::fs::read(&destination).ok().as_deref() == Some(bytes) {
		return Ok(destination);
	}

	let mut temporary = None;
	let mut file = None;
	for _ in 0..32 {
		let sequence = NEXT_EMBEDDED_TEMP.fetch_add(1, Ordering::Relaxed);
		let candidate = directory.join(format!(
			".{filename}.{}.{}.tmp",
			std::process::id(),
			sequence
		));
		match OpenOptions::new().write(true).create_new(true).open(&candidate) {
			Ok(created) => {
				temporary = Some(candidate);
				file = Some(created);
				break;
			}
			Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
			Err(error) => return Err(error),
		}
	}

	let temporary = temporary.ok_or_else(|| {
		std::io::Error::new(
			std::io::ErrorKind::AlreadyExists,
			"could not allocate a unique embedded-resource temporary file",
		)
	})?;

	let write_result = (|| {
		let mut file = file.expect("a temporary embedded-resource file was created");
		file.write_all(bytes)?;
		file.sync_all()
	})();

	if let Err(error) = write_result {
		let _ = std::fs::remove_file(&temporary);
		return Err(error);
	}

	if let Err(error) = replace_embedded_file(&temporary, &destination) {
		let _ = std::fs::remove_file(&temporary);
		// The previous executable may be mapped by the current process or another launcher.
		// Keep a complete old copy rather than deleting it or writing a truncated replacement.
		if destination.is_file() {
			return Ok(destination);
		}
		return Err(error);
	}

	Ok(destination)
}

/// Writes the embedded client mod into the data directory of the launcher.
/// Kept separate from the app handle so the replacement can be tested without a window.
fn write_embedded_mod(directory: &std::path::Path, bytes: &[u8]) -> Result<PathBuf, String> {
	write_embedded_file(directory, MOD_NAME, bytes).map_err(|error| error.to_string())
}

/// The time the core has to answer before it is considered stuck.
///
/// The commands that read the network are given more room, the local ones only need a moment.
fn timeout_for(args: &[String]) -> u64 {
	match args.first().map(String::as_str) {
		Some("versions") | Some("loaders") => 45,
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
}

fn emit_event(app: &AppHandle, kind: StreamKind, value: &Value) {
	let event = match kind {
		StreamKind::Install => "install:progress",
		StreamKind::Game => "game:output",
		StreamKind::Microsoft => "microsoft:deviceCode",
		StreamKind::Optimisation => "optimisation:progress",
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
		let path = write_embedded_mod(&directory, b"first").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"first");

		// Same bytes: the archive is left alone and no temporary file is written.
		let path = write_embedded_mod(&directory, b"first").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"first");
		assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);

		// New bytes: the archive is replaced atomically, with no temporary file left behind.
		let path = write_embedded_mod(&directory, b"second").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"second");
		assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);

		let _ = std::fs::remove_dir_all(&directory);
	}

	#[test]
	fn concurrent_embedded_mod_updates_do_not_share_temporary_files() {
		let directory = std::env::temp_dir().join(format!("aether-mod-concurrent-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&directory);
		std::fs::create_dir_all(&directory).unwrap();

		let workers: Vec<_> = (0..8)
			.map(|index| {
				let directory = directory.clone();
				std::thread::spawn(move || {
					let bytes = vec![index as u8; 4096 + index];
					write_embedded_mod(&directory, &bytes).unwrap();
					bytes
				})
			})
			.collect();

		let candidates: Vec<Vec<u8>> = workers.into_iter().map(|worker| worker.join().unwrap()).collect();
		let actual = std::fs::read(directory.join(MOD_NAME)).unwrap();
		assert!(candidates.iter().any(|candidate| *candidate == actual));
		assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);

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
		assert_eq!(timeout_for(&args(&["performance", "install"])), 900);
		assert_eq!(timeout_for(&args(&["performance", "list"])), 20);
		assert_eq!(timeout_for(&args(&["info"])), 20);
	}
}
