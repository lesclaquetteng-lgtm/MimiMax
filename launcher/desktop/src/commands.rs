//! The commands the interface can call.
//!
//! Every command mirrors one handler of the Electron shell: same arguments, same answer, same
//! events. The interface of `gui/src/ui` is therefore unchanged, only the process around it is now
//! Rust.

use crate::core::{self, StreamKind};
use crate::state::{Active, RunState};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager, State, Window};

/* --------------------------------------------------------------- helpers */

fn text(value: Option<&Value>) -> Option<String> {
	value.and_then(Value::as_str).map(str::to_string)
}

/// The textual form of a JSON value, as `String(value)` in the interface.
fn cli_value(value: &Value) -> String {
	match value {
		Value::String(text) => text.clone(),
		other => other.to_string(),
	}
}

fn failure(message: &str) -> Value {
	json!({ "ok": false, "error": message })
}

/// The interface reads the account store directly, the core answers with `{ ok, accounts }`.
fn account_store(result: Value) -> Value {
	let store = match result.get("accounts") {
		Some(Value::Object(_)) => result["accounts"].clone(),
		_ => json!({ "accounts": [], "selected": null }),
	};

	let mut answer = match result.get("ok").and_then(Value::as_bool) {
		Some(true) => json!({ "ok": true }),
		_ => json!({
			"ok": false,
			"error": result
				.get("error")
				.cloned()
				.unwrap_or_else(|| json!("The accounts could not be read."))
		}),
	};

	if let (Some(target), Some(source)) = (answer.as_object_mut(), store.as_object()) {
		for (key, value) in source {
			target.insert(key.clone(), value.clone());
		}
	}

	answer
}

/* ----------------------------------------------------------------- window */

#[tauri::command(rename_all = "snake_case")]
pub fn window_action(window: Window, action: String) -> Value {
	let outcome = match action.as_str() {
		"minimize" => window.minimize(),
		"maximize" => {
			if window.is_maximized().unwrap_or(false) {
				window.unmaximize()
			} else {
				window.maximize()
			}
		}
		"close" => window.close(),
		_ => return json!({ "ok": false, "error": "Unknown window action." }),
	};

	match outcome {
		Ok(()) => json!({ "ok": true }),
		Err(error) => failure(&error.to_string()),
	}
}

/* ------------------------------------------------------------ application */

#[tauri::command(rename_all = "snake_case")]
pub async fn app_info(app: AppHandle) -> Value {
	core::run_core(app, vec!["info".to_string()]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn settings_get(app: AppHandle) -> Value {
	core::run_core(app, vec!["settings".to_string()]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn settings_update(app: AppHandle, patch: Option<Value>) -> Value {
	let mut args = vec!["settings".to_string()];

	if let Some(entries) = patch.as_ref().and_then(Value::as_object) {
		for (key, value) in entries {
			args.push("--key".to_string());
			args.push(key.clone());
			args.push("--value".to_string());
			args.push(cli_value(value));
		}
	}

	core::run_core(app, args).await
}

/* --------------------------------------------------------------- accounts */

#[tauri::command(rename_all = "snake_case")]
pub async fn accounts_get(app: AppHandle) -> Value {
	account_store(core::run_core(app, vec!["accounts".to_string()]).await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn accounts_add_offline(app: AppHandle, username: String) -> Value {
	core::run_core(
		app,
		vec!["account".to_string(), "add-offline".to_string(), username],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn accounts_remove(app: AppHandle, id: String) -> Value {
	account_store(
		core::run_core(app, vec!["account".to_string(), "remove".to_string(), id]).await,
	)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn accounts_select(app: AppHandle, id: String) -> Value {
	account_store(
		core::run_core(app, vec!["account".to_string(), "select".to_string(), id]).await,
	)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn accounts_microsoft_sign_in(app: AppHandle) -> Value {
	blocking_stream(app, vec!["account".to_string(), "microsoft".to_string()], StreamKind::Microsoft)
		.await
}

/* --------------------------------------------------------------- versions */

#[tauri::command(rename_all = "snake_case")]
pub async fn versions_list(app: AppHandle, limit: Option<u32>) -> Value {
	core::run_core(
		app,
		vec![
			"versions".to_string(),
			"--limit".to_string(),
			limit.unwrap_or(40).to_string(),
		],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn versions_loaders(app: AppHandle, game_version: String) -> Value {
	core::run_core(
		app,
		vec![
			"loaders".to_string(),
			"--version".to_string(),
			game_version,
		],
	)
	.await
}

/* --------------------------------------------------------------- profiles */

#[tauri::command(rename_all = "snake_case")]
pub async fn profiles_list(app: AppHandle) -> Value {
	core::run_core(app, vec!["profiles".to_string(), "list".to_string()]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn profiles_assign(app: AppHandle, account_id: String, name: String) -> Value {
	core::run_core(
		app,
		vec![
			"profiles".to_string(),
			"assign".to_string(),
			account_id,
			name,
		],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn profiles_add(app: AppHandle, name: String, source: Value) -> Value {
	let mut args = vec!["profiles".to_string(), "add".to_string(), name];

	for (key, flag) in [
		("version", "--version"),
		("loader", "--loader"),
		("memory", "--memory"),
		("minMemory", "--min-memory"),
		("width", "--width"),
		("height", "--height"),
	] {
		if let Some(value) = source.get(key) {
			args.push(flag.to_string());
			args.push(cli_value(value));
		}
	}

	core::run_core(app, args).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn profiles_update(app: AppHandle, name: String, patch: Value) -> Value {
	let mut args = vec!["profiles".to_string(), "update".to_string(), name];

	for (key, flag) in [
		("version", "--version"),
		("loader", "--loader"),
		("memory", "--memory"),
		("minMemory", "--min-memory"),
		("width", "--width"),
		("height", "--height"),
	] {
		if let Some(value) = patch.get(key) {
			args.push(flag.to_string());
			args.push(cli_value(value));
		}
	}

	core::run_core(app, args).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn profiles_remove(app: AppHandle, name: String) -> Value {
	core::run_core(
		app,
		vec!["profiles".to_string(), "remove".to_string(), name],
	)
	.await
}

/* ------------------------------------------------------------------- mods */

#[tauri::command(rename_all = "snake_case")]
pub async fn mods_list(app: AppHandle) -> Value {
	core::run_core(app, vec!["mods".to_string(), "list".to_string()]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn mods_toggle(app: AppHandle, name: String, enabled: bool) -> Value {
	core::run_core(
		app,
		vec![
			"mods".to_string(),
			"toggle".to_string(),
			name,
			"--enabled".to_string(),
			enabled.to_string(),
		],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn mods_open_folder(app: AppHandle) -> Value {
	let info = core::run_core(app.clone(), vec!["info".to_string()]).await;
	let folder = info
		.get("paths")
		.and_then(|paths| paths.get("mods"))
		.and_then(Value::as_str);

	let Some(folder) = folder else {
		return failure("The mods folder is unknown.");
	};

	if let Err(error) = std::fs::create_dir_all(folder) {
		return failure(&error.to_string());
	}

	match open_folder(Path::new(folder)) {
		Ok(()) => json!({ "ok": true, "folder": folder }),
		Err(error) => json!({ "ok": false, "error": error, "folder": folder }),
	}
}

/// Opens a directory in the file explorer, without a console window.
fn open_folder(path: &Path) -> Result<(), String> {
	#[cfg(windows)]
	{
		use std::os::windows::process::CommandExt;
		use std::process::{Command, Stdio};

		// `explorer` returns a non-zero code even when it succeeds, only the spawn matters.
		Command::new("explorer")
			.arg(path)
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.creation_flags(0x0800_0000)
			.spawn()
			.map(|_| ())
			.map_err(|error| error.to_string())
	}

	#[cfg(not(windows))]
	{
		use std::process::{Command, Stdio};

		let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
		Command::new(opener)
			.arg(path)
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.map(|_| ())
			.map_err(|error| error.to_string())
	}
}

/* ---------------------------------------------------------------- servers */

#[tauri::command(rename_all = "snake_case")]
pub async fn servers_list(app: AppHandle) -> Value {
	core::run_core(app, vec!["servers".to_string(), "list".to_string()]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn servers_add(app: AppHandle, name: String, address: String) -> Value {
	core::run_core(
		app,
		vec![
			"servers".to_string(),
			"add".to_string(),
			address.clone(),
			"--name".to_string(),
			if name.is_empty() { address } else { name },
		],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn servers_remove(app: AppHandle, key: String) -> Value {
	core::run_core(
		app,
		vec!["servers".to_string(), "remove".to_string(), key],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn servers_ping(app: AppHandle, address: String) -> Value {
	core::run_core(app, vec!["servers".to_string(), "ping".to_string(), address]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn servers_probe(app: AppHandle, address: String) -> Value {
	core::run_core(
		app,
		vec!["servers".to_string(), "probe".to_string(), address],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn hosts_list(app: AppHandle) -> Value {
	core::run_core(app, vec!["hosts".to_string(), "list".to_string()]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn hosts_create(app: AppHandle, options: Option<Value>) -> Value {
	let options = options.unwrap_or_else(|| json!({}));
	let mut args = vec!["hosts".to_string(), "create".to_string()];

	args.push(text(options.get("name")).unwrap_or_default());
	args.push("--port".to_string());
	args.push(
		options
			.get("port")
			.and_then(Value::as_u64)
			.unwrap_or(25565)
			.to_string(),
	);
	args.push("--max-players".to_string());
	args.push(
		options
			.get("maxPlayers")
			.and_then(Value::as_u64)
			.unwrap_or(20)
			.to_string(),
	);

	if let Some(motd) = text(options.get("motd")) {
		if !motd.is_empty() {
			args.push("--motd".to_string());
			args.push(motd);
		}
	}

	if options.get("onlineMode").and_then(Value::as_bool) == Some(true) {
		args.push("--online-mode".to_string());
		args.push("true".to_string());
	}

	core::run_core(app, args).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn hosts_start(app: AppHandle, name: String) -> Value {
	core::run_core(app, vec!["hosts".to_string(), "start".to_string(), name]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn hosts_stop(app: AppHandle, name: String) -> Value {
	core::run_core(app, vec!["hosts".to_string(), "stop".to_string(), name]).await
}

/* ------------------------------------------------------------ performance */

#[tauri::command(rename_all = "snake_case")]
pub async fn performance_list(app: AppHandle) -> Value {
	core::run_core(app, vec!["performance".to_string(), "list".to_string()]).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn performance_tune(app: AppHandle, preset: String) -> Value {
	core::run_core(
		app,
		vec![
			"performance".to_string(),
			"tune".to_string(),
			"--preset".to_string(),
			if preset.is_empty() { "max".to_string() } else { preset },
		],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn performance_jvm(app: AppHandle, preset: String) -> Value {
	core::run_core(
		app,
		vec![
			"performance".to_string(),
			"jvm".to_string(),
			if preset.is_empty() { "balanced".to_string() } else { preset },
		],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn performance_install(app: AppHandle, slugs: Option<Value>) -> Value {
	let mut args = vec!["performance".to_string(), "install".to_string()];

	if let Some(Value::Array(entries)) = slugs {
		let names: Vec<String> = entries
			.iter()
			.filter_map(Value::as_str)
			.map(str::to_string)
			.collect();
		if !names.is_empty() {
			args.push("--slugs".to_string());
			args.push(names.join(","));
		}
	}

	blocking_stream(app, args, StreamKind::Optimisation).await
}

/* ------------------------------------------------------------------- logs */

#[tauri::command(rename_all = "snake_case")]
pub async fn logs_tail(app: AppHandle, lines: Option<u32>) -> Value {
	core::run_core(
		app,
		vec![
			"logs".to_string(),
			"--lines".to_string(),
			lines.unwrap_or(200).to_string(),
		],
	)
	.await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn logs_read(app: AppHandle, name: String, lines: Option<u32>) -> Value {
	core::run_core(
		app,
		vec![
			"logs".to_string(),
			"read".to_string(),
			name,
			"--lines".to_string(),
			lines.unwrap_or(400).to_string(),
		],
	)
	.await
}

/* ------------------------------------------------------- install and game */

/// A streaming command without a process to stop (Microsoft login, optimisation pack).
async fn blocking_stream(app: AppHandle, args: Vec<String>, kind: StreamKind) -> Value {
	core::blocking(move || core::run_core_streaming(&app, &args, kind, |_, _| {}))
		.await
		.unwrap_or_else(|| failure("The launcher core stopped unexpectedly."))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn install_start(app: AppHandle, profile: Option<Value>) -> Value {
	let mut args = vec!["install".to_string()];
	if let Some(version) = profile
		.as_ref()
		.and_then(|profile| profile.get("gameVersion"))
		.and_then(Value::as_str)
	{
		args.push("--version".to_string());
		args.push(version.to_string());
	}
	if let Some(loader) = profile
		.as_ref()
		.and_then(|profile| profile.get("loaderVersion"))
		.and_then(Value::as_str)
	{
		args.push("--loader".to_string());
		args.push(loader.to_string());
	}

	let cancelled = Arc::new(AtomicBool::new(false));
	let stopped = Arc::new(AtomicBool::new(false));
	let slot = app.state::<RunState>().install.clone();
	let flag = cancelled.clone();
	let application = app.clone();

	let result = core::blocking(move || {
		core::run_core_streaming(&application, &args, StreamKind::Install, move |pid, stdin| {
			if let Ok(mut guard) = slot.lock() {
				*guard = Some(Active {
					pid,
					stdin,
					cancelled: flag,
					stopped,
				});
			}
		})
	})
	.await
	.unwrap_or_else(|| failure("The launcher core stopped unexpectedly."));

	let slot = app.state::<RunState>().install.clone();
	if let Ok(mut guard) = slot.lock() {
		*guard = None;
	}

	if cancelled.load(Ordering::SeqCst) {
		json!({
			"ok": false,
			"cancelled": true,
			"error": "The installation was cancelled."
		})
	} else {
		result
	}
}

#[tauri::command(rename_all = "snake_case")]
pub fn install_stop(state: State<'_, RunState>) -> Value {
	let slot = state.install.clone();

	let pid = {
		let mut guard = match slot.lock() {
			Ok(guard) => guard,
			Err(_) => return failure("The state of the installation is unavailable."),
		};

		let Some(active) = guard.as_mut() else {
			return failure("No installation is running.");
		};

		active.cancelled.store(true, Ordering::SeqCst);
		if let Some(stdin) = active.stdin.as_mut() {
			let _ = core::write_line(stdin, "cancel");
		}

		active.pid
	};

	// A core that ignores the request is stopped the hard way; the files it downloaded stay, the
	// next installation resumes them.
	let watcher = slot.clone();
	std::thread::spawn(move || {
		std::thread::sleep(Duration::from_secs(10));
		let still_running = watcher
			.lock()
			.ok()
			.and_then(|guard| guard.as_ref().map(|active| active.pid))
			== Some(pid);

		if still_running {
			core::kill_tree(pid);
		}
	});

	json!({ "ok": true })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn game_launch(app: AppHandle, options: Option<Value>) -> Value {
	let mut args = vec!["launch".to_string()];

	if let Some(account) = options
		.as_ref()
		.and_then(|options| options.get("accountId"))
		.and_then(Value::as_str)
	{
		args.push("--account".to_string());
		args.push(account.to_string());
	}
	if let Some(version) = options
		.as_ref()
		.and_then(|options| options.get("gameVersion"))
		.and_then(Value::as_str)
	{
		args.push("--version".to_string());
		args.push(version.to_string());
	}
	if let Some(server) = options
		.as_ref()
		.and_then(|options| options.get("server"))
		.and_then(Value::as_str)
	{
		args.push("--server".to_string());
		args.push(server.to_string());
	}

	let stopped = Arc::new(AtomicBool::new(false));
	let slot = app.state::<RunState>().game.clone();
	let flag = stopped.clone();
	let application = app.clone();

	let result = core::blocking(move || {
		core::run_core_streaming(&application, &args, StreamKind::Game, move |pid, stdin| {
			if let Ok(mut guard) = slot.lock() {
				*guard = Some(Active {
					pid,
					stdin,
					cancelled: Arc::new(AtomicBool::new(false)),
					stopped: flag,
				});
			}
		})
	})
	.await
	.unwrap_or_else(|| failure("The launcher core stopped unexpectedly."));

	let slot = app.state::<RunState>().game.clone();
	if let Ok(mut guard) = slot.lock() {
		*guard = None;
	}

	if stopped.load(Ordering::SeqCst) {
		json!({ "ok": true, "stopped": true })
	} else {
		result
	}
}

// An asynchronous command cannot borrow the state across its await, so the slot is read and released
// in a block of its own; the answer is a `Result` because the command is asynchronous.
#[tauri::command(rename_all = "snake_case")]
pub async fn game_stop(app: AppHandle) -> Result<Value, String> {
	let pid = {
		let state = app.state::<RunState>();
		let mut guard = state
			.game
			.lock()
			.map_err(|_| "The state of the game is unavailable.".to_string())?;

		match guard.as_mut() {
			Some(active) => {
				active.stopped.store(true, Ordering::SeqCst);
				Some(active.pid)
			}
			None => None,
		}
	};

	if let Some(pid) = pid {
		core::kill_tree(pid);
		return Ok(json!({ "ok": true }));
	}

	// The shell has no child of its own: the game may have been started by a previous run that was
	// killed, and only the record of the core still names it. Without this fallback that game would
	// survive every restart of the launcher.
	Ok(core::run_core(app, vec!["stop".to_string()]).await)
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn the_account_store_keeps_the_lists_of_the_core() {
		let answer = account_store(json!({
			"ok": true,
			"accounts": { "accounts": [{ "username": "Notch" }], "selected": "id" }
		}));

		assert_eq!(answer["ok"], json!(true));
		assert_eq!(answer["accounts"][0]["username"], json!("Notch"));
		assert_eq!(answer["selected"], json!("id"));
	}

	#[test]
	fn a_failed_account_store_carries_the_error_of_the_core() {
		let answer = account_store(json!({ "ok": false, "error": "broken" }));

		assert_eq!(answer["ok"], json!(false));
		assert_eq!(answer["error"], json!("broken"));
		assert_eq!(answer["accounts"], json!([]));
		assert_eq!(answer["selected"], json!(null));
	}

	#[test]
	fn the_settings_are_written_like_the_interface_wrote_them() {
		assert_eq!(cli_value(&json!(false)), "false");
		assert_eq!(cli_value(&json!(4096)), "4096");
		assert_eq!(cli_value(&json!("a b")), "a b");
	}
}
