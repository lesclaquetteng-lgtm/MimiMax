//! The native shell of the Aether Launcher.
//!
//! The interface is the same HTML/CSS/JavaScript as the Electron build, served by Tauri to the
//! WebView2 runtime of Windows. Everything around it is Rust: the window, the single-instance
//! lock, the process runner and the packaging.
//!
//! Why this replaced Electron: the shell went from ~150 MB installed to a single ~8 MB executable,
//! the memory footprint dropped from a whole Chromium to the system webview, and the backend is
//! the same language as the launcher core. The interface itself did not change.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod core;
mod state;

fn main() {
	tauri::Builder::default()
		// Only one launcher may write the settings and the accounts of the player, two instances
		// would overwrite each other.
		.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
			use tauri::Manager;

			if let Some(window) = app.get_webview_window("main") {
				let _ = window.unminimize();
				let _ = window.show();
				let _ = window.set_focus();
			}
		}))
		.manage(state::RunState::default())
		.invoke_handler(tauri::generate_handler![
			commands::window_action,
			commands::app_info,
			commands::settings_get,
			commands::settings_update,
			commands::accounts_get,
			commands::accounts_add_offline,
			commands::accounts_check_name,
			commands::accounts_remove,
			commands::accounts_select,
			commands::accounts_microsoft_sign_in,
			commands::versions_list,
			commands::versions_loaders,
			commands::profiles_list,
			commands::profiles_assign,
			commands::profiles_add,
			commands::profiles_update,
			commands::profiles_remove,
			commands::mods_list,
			commands::mods_search,
			commands::mods_install,
			commands::mods_toggle,
			commands::mods_open_folder,
			commands::servers_list,
			commands::servers_add,
			commands::servers_remove,
			commands::servers_ping,
			commands::servers_probe,
			commands::hosts_list,
			commands::hosts_create,
			commands::hosts_start,
			commands::hosts_stop,
			commands::performance_list,
			commands::performance_tune,
			commands::performance_jvm,
			commands::performance_install,
			commands::logs_tail,
			commands::logs_read,
			commands::install_start,
			commands::install_stop,
			commands::game_launch,
			commands::game_stop,
		])
		.setup(|app| {
			use tauri::Manager;

			// The window is created hidden, so that the first frame the player sees is the
			// interface and not a white rectangle.
			if let Some(window) = app.get_webview_window("main") {
				window.show()?;
			}

			Ok(())
		})
		.build(tauri::generate_context!())
		.expect("the Aether launcher could not start")
		.run(|app, event| match event {
			tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
				state::kill_active(app);
			}
			_ => {}
		});
}
