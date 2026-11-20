//! The processes the interface can stop while they run.
//!
//! The installation and the game are long: the window must be able to cancel or stop them without
//! waiting for the core to finish on its own. Operation guards also prevent two commands from
//! writing to the same Minecraft directories at the same time.

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

/// Keeps one install-like operation or one game launch exclusive until its future completes.
///
/// The guard releases the slot on drop, including when a Tauri command returns early or its
/// future is cancelled by the runtime.
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
	/// Acquires the shared game-directory lock used by Minecraft and mod-pack installations.
	pub fn acquire_install(&self) -> Option<OperationGuard> {
		// Do not touch libraries, mods or options.txt while Minecraft is using the game folder.
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

	/// Acquires the single-game-process lock, only when the game directory is idle.
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
	fn install_lock_is_exclusive_and_released_on_drop() {
		let state = RunState::default();
		let first = state.acquire_install().expect("first install acquires the lock");
		assert!(state.acquire_install().is_none(), "second install is rejected");
		drop(first);
		assert!(state.acquire_install().is_some(), "lock can be acquired after completion");
	}

	#[test]
	fn game_and_install_locks_are_independent() {
		let state = RunState::default();
		let install = state.acquire_install().expect("install lock");
		let game = state.acquire_game().expect("game lock");
		assert!(state.acquire_install().is_none());
		assert!(state.acquire_game().is_none());
		drop((install, game));
		assert!(state.acquire_install().is_some());
		assert!(state.acquire_game().is_some());
	}

	#[test]
	fn dropping_a_guard_releases_the_lock_even_after_early_exit() {
		let state = RunState::default();
		{
			let _guard = state.acquire_game().expect("game lock");
			assert!(state.acquire_game().is_none());
		}
		assert!(state.acquire_game().is_some());
	}

	#[test]
	fn installing_mods_or_game_files_blocks_a_game_launch() {
		let state = RunState::default();
		let _install = state.acquire_install().expect("install lock");
		assert!(state.acquire_game().is_none());
	}

	#[test]
	fn a_running_game_blocks_mod_installation_and_tuning() {
		let state = RunState::default();
		let _game = state.acquire_game().expect("game lock");
		assert!(state.acquire_install().is_none());
	}
}
