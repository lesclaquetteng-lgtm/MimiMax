#![windows_subsystem = "windows"]

//! The small executable at the root of the project (`MimiMax.exe`, `Aether-Launcher.exe`).
//!
//! It only finds the shell and starts it. The shell is the native Tauri one (`launcher/desktop`):
//! the Electron build it used to start was removed, so a runner that still pointed at
//! `launcher/node_modules/electron` would open nothing at all on a machine that never ran
//! `npm install`.

use std::env;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(
        hwnd: *mut std::ffi::c_void,
        lp_text: *const u16,
        lp_caption: *const u16,
        u_type: u32,
    ) -> i32;
}

fn show_error(title: &str, message: &str) {
    let title_wide: Vec<u16> = OsStr::new(title).encode_wide().chain(std::iter::once(0)).collect();
    let message_wide: Vec<u16> = OsStr::new(message).encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message_wide.as_ptr(),
            title_wide.as_ptr(),
            0x00000010, // MB_ICONERROR
        );
    }
}

/// The shell, in the order an installation can carry it:
///
/// 1. beside the runner, the layout of a portable or installed build;
/// 2. in the build directory of the source tree;
/// 3. the artefact that `dist/` ships.
fn find_shell(root: &Path) -> Option<PathBuf> {
    for candidate in [
        root.join("aether-desktop.exe"),
        root.join("launcher").join("desktop").join("target").join("release").join("aether-desktop.exe"),
        root.join("dist").join("Aether-Launcher-1.0.0-portable.exe"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn main() {
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(_) => PathBuf::from("."),
    };

    let root = current_exe.parent().unwrap_or(Path::new("."));

    let Some(shell) = find_shell(root) else {
        show_error(
            "MimiMax Launcher",
            "La coquille du lanceur est introuvable.\n\nCompilez-la avec :\n  cargo build --release --manifest-path launcher/desktop/Cargo.toml",
        );
        return;
    };

    match Command::new(&shell).current_dir(root).spawn() {
        Ok(_) => {}
        Err(error) => {
            show_error(
                "MimiMax Launcher",
                &format!("Erreur lors du lancement de l'interface :\n{}", error),
            );
        }
    }
}
