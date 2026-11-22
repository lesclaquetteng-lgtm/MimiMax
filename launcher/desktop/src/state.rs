//! The processes the interface can stop while they run.
//!
//! The installation and the game are long: the window must be able to cancel or stop them without
//! waiting for the core to finish on its own.

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

#[derive(Default)]
pub struct RunState {
    pub install: Arc<Mutex<Option<Active>>>,
    pub game: Arc<Mutex<Option<Active>>>,
    pub install_busy: Arc<AtomicBool>,
    pub game_busy: Arc<AtomicBool>,
}

/// A single active long-running operation; dropping it always releases the slot.
pub struct BusyOperation {
    flag: Arc<AtomicBool>,
}

impl BusyOperation {
    /// Acquires the operation slot, returning no guard when another call already owns it.
    pub fn try_acquire(flag: Arc<AtomicBool>) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self { flag })
    }
}

impl Drop for BusyOperation {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
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
    fn only_one_call_can_own_a_long_running_operation() {
        let flag = Arc::new(AtomicBool::new(false));
        let first = BusyOperation::try_acquire(flag.clone()).expect("first call owns the slot");
        assert!(BusyOperation::try_acquire(flag.clone()).is_none());

        drop(first);
        assert!(BusyOperation::try_acquire(flag.clone()).is_some());
    }
}
