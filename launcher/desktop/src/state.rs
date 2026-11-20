//! The processes and exclusive operations the interface can run.
//!
//! The game directory must never be modified while Minecraft is using it. These guards cover the
//! entire asynchronous Tauri command, not just the instant when the child process is spawned.

use std::process::ChildStdin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A core that was started by a streaming command.
pub struct Active {
	pub pid: u32,
	pub stdin: Option<ChildStdin>,
	pub cancelled: Arc<AtomicBool>,
	pub stopped: Arc<AtomicBool>,
}

/// Releases an exclusive operation when its Tauri command returns or is cancelled.
pub struct OperationGuard {
	busy: Arc<AtomicBool>,
}

impl Drop for OperationGuard {
	fn drop(&mut self) {
		self.busy.store(false, Ordering::Release);
	}
}

#[derive(Default)]
pub struct RunState {
	pub install: Arc<Mutex<Option<Active>>>,
	pub game: Arc<Mutex<Option<Active>>>,
	install_busy: Arc<AtomicBool>,
	game_busy: Arc<AtomicBool>,
}

impl RunState {
	/// Locks the Minecraft directory for installs, mod changes and game-option tuning.
	pub fn acquire_install(&self) -> Option<OperationGuard> {
		if self.game_busy.load(Ordering::Acquire) {
			return None;
		}
		self.install_busy
			.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
			.ok()?;

		// Close the race where a launch acquired its lock between the first check and our CAS.
		if self.game_busy.load(Ordering::Acquire) {
			self.install_busy.store(false, Ordering::Release);
			return None;
		}
		Some(OperationGuard {
			busy: Arc::clone(&self.install_busy),
		})
	}

	/// Locks Minecraft's process slot and prevents the game directory changing during play.
	pub fn acquire_game(&self) -> Option<OperationGuard> {
		if self.install_busy.load(Ordering::Acquire) {
			return None;
		}
		self.game_busy
			.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
			.ok()?;

		// Close the race where an install acquired its lock between the first check and our CAS.
		if self.install_busy.load(Ordering::Acquire) {
			self.game_busy.store(false, Ordering::Release);
			return None;
		}
		Some(OperationGuard {
			busy: Arc::clone(&self.game_busy),
		})
	}
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn install_and_launch_are_mutually_exclusive() {
		let state = RunState::default();
		let install = state.acquire_install().expect("install lock");
		assert!(state.acquire_game().is_none(), "launch must wait for the install");
		drop(install);

		let game = state.acquire_game().expect("game lock");
		assert!(state.acquire_install().is_none(), "files cannot change during play");
		drop(game);

		assert!(state.acquire_install().is_some());
	}

	#[test]
	fn only_one_install_or_game_operation_can_run_at_a_time() {
		let state = RunState::default();
		let install = state.acquire_install().expect("first install");
		assert!(state.acquire_install().is_none());
		drop(install);

		let game = state.acquire_game().expect("first game");
		assert!(state.acquire_game().is_none());
		drop(game);
	}

	#[test]
	fn guards_release_their_lock_on_drop() {
		let state = RunState::default();
		{
			let _install = state.acquire_install().expect("install lock");
			assert!(state.acquire_game().is_none());
		}
		assert!(state.acquire_game().is_some());
	}
}
