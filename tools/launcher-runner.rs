#![windows_subsystem = "windows"]

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

fn main() {
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(_) => PathBuf::from("."),
    };

    let root = current_exe.parent().unwrap_or(Path::new("."));

    // 1. Check for standalone packaged executable first if built
    let packaged_exe = root.join("launcher").join("release").join("win-unpacked").join("Aether Launcher.exe");
    if packaged_exe.exists() {
        if let Err(e) = Command::new(&packaged_exe).spawn() {
            show_error("MimiMax Launcher", &format!("Impossible de démarrer la version packagée:\n{}", e));
        }
        return;
    }

    // 2. Otherwise use local dev Electron
    let electron_exe = root.join("launcher").join("node_modules").join("electron").join("dist").join("electron.exe");
    let launcher_dir = root.join("launcher");

    if !electron_exe.exists() {
        show_error(
            "MimiMax Launcher",
            &format!(
                "Fichier Electron introuvable :\n{}\nVeuillez exécuter 'npm install' dans le dossier launcher.",
                electron_exe.display()
            ),
        );
        return;
    }

    let core_exe = root.join("launcher").join("target").join("release").join("aether-launcher.exe");

    let mut cmd = Command::new(&electron_exe);
    cmd.arg(&launcher_dir);
    cmd.current_dir(&launcher_dir);
    cmd.env_remove("ELECTRON_RUN_AS_NODE");

    if core_exe.exists() {
        cmd.env("AETHER_LAUNCHER_BIN", &core_exe);
    }

    match cmd.spawn() {
        Ok(_) => {}
        Err(e) => {
            show_error("MimiMax Launcher", &format!("Erreur lors du lancement de l'interface:\n{}", e));
        }
    }
}
