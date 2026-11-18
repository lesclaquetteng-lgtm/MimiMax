//! The processes the interface can stop while they run.
//!
//! The installation and the game are long: the window must be able to cancel or stop them without
//! waiting for the core to finish on its own.

use std::process::ChildStdin;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// A core that was started by a streaming command.
pub struct Active {
	pub pid: u32,
	pub stdin: Option<ChildStdin>,
	pub cancelled: Arc<AtomicBool>,
	pub stopped: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct RunState {
	pub install: Arc<Mutex<Option<Active>>>,
	pub game: Arc<Mutex<Option<Active>>>,
}

/// Stops whatever is still running when the launcher closes.
///
/// Without this, closing the window would leave the core and Java behind: the launcher is the
/// owner of the session it started.
pub fn kill_active(app: &tauri::AppHandle) {
	use tauri::Manager;

	let Some(state) = app.try_state::<RunState>() else {
		return;
	};

	for slot in [&state.install, &state.game] {
		let pid = slot
			.lock()
			.ok()
			.and_then(|guard| guard.as_ref().map(|active| active.pid));

		if let Some(pid) = pid {
			crate::core::kill_tree(pid);
		}
	}
}
