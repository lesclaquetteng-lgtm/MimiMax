//! The mods of the game.
//!
//! A mod is a jar of the mods folder. Disabling one does not delete it: the file is renamed with a
//! `.disabled` suffix, which the loader ignores, so the jar stays on the disk and can be enabled
//! again without downloading it twice.

use std::fs;
use std::path::{Path, PathBuf};

/// The suffix that marks a mod as disabled.
pub const DISABLED_SUFFIX: &str = ".disabled";

/// A file of the mods folder.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    /// The name of the file on the disk, the identifier used to enable or disable the mod.
    pub name: String,
    /// The name the game sees, without the `.disabled` suffix.
    pub mod_id: String,
    /// The full path of the file.
    pub path: String,
    /// The size of the file, in bytes.
    pub size: u64,
    /// `true` when the loader reads the file.
    pub enabled: bool,
}

impl ModEntry {
    fn from_path(path: PathBuf) -> Option<ModEntry> {
        let name = path.file_name()?.to_str()?.to_string();
        if !is_mod(&name) {
            return None;
        }

        let metadata = fs::metadata(&path).ok()?;
        if !metadata.is_file() {
            return None;
        }

        let enabled = !name.ends_with(DISABLED_SUFFIX);
        let mod_id = name
            .strip_suffix(DISABLED_SUFFIX)
            .unwrap_or(&name)
            .to_string();

        Some(ModEntry {
            name,
            mod_id,
            path: path.to_string_lossy().to_string(),
            size: metadata.len(),
            enabled,
        })
    }
}

/// Only the jars of the folder are mods, the other files are left alone.
fn is_mod(name: &str) -> bool {
    name.ends_with(".jar") || name.ends_with(".jar.disabled")
}

/// The mods of the folder, enabled ones first, then by name.
pub fn list(mods_dir: &Path) -> Vec<ModEntry> {
    let Ok(entries) = fs::read_dir(mods_dir) else {
        return Vec::new();
    };

    let mut mods: Vec<ModEntry> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| ModEntry::from_path(entry.path()))
        .collect();

    mods.sort_by(|left, right| {
        right
            .enabled
            .cmp(&left.enabled)
            .then_with(|| left.mod_id.to_lowercase().cmp(&right.mod_id.to_lowercase()))
    });

    mods
}

/// Refuses anything that is not a plain file name of the mods folder.
fn checked_name(name: &str) -> Result<&str, String> {
    let path = Path::new(name);
    if name.is_empty() || path.file_name().and_then(|value| value.to_str()) != Some(name) {
        return Err(format!("invalid mod name: {}", name));
    }
    Ok(name)
}

/// Enables or disables a mod by renaming its file.
pub fn set_enabled(mods_dir: &Path, name: &str, enabled: bool) -> Result<ModEntry, String> {
    let name = checked_name(name)?;
    let current = mods_dir.join(name);

    if !current.is_file() {
        return Err(format!("the mod does not exist: {}", name));
    }

    // The file name is the state: the suffix is added to disable the mod and removed to enable it.
    let base = name.strip_suffix(DISABLED_SUFFIX).unwrap_or(name);
    let target = if enabled {
        mods_dir.join(base)
    } else {
        mods_dir.join(format!("{}{}", base, DISABLED_SUFFIX))
    };

    if target != current {
        if target.exists() {
            return Err(format!(
                "the file {} already exists, remove it first",
                target
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_default()
            ));
        }

        fs::rename(&current, &target)
            .map_err(|error| format!("the mod could not be renamed: {}", error))?;
    }

    ModEntry::from_path(target)
        .ok_or_else(|| "the mod could not be read after the change".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty mods folder that belongs to one test.
    fn temporary(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("aether-mods-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn the_folder_lists_the_enabled_and_the_disabled_jars() {
        let root = temporary("list");
        fs::write(root.join("sodium.jar"), b"jar").unwrap();
        fs::write(root.join("lithium.jar.disabled"), b"jar jar").unwrap();
        fs::write(root.join("notes.txt"), b"not a mod").unwrap();
        fs::create_dir_all(root.join("config")).unwrap();

        let mods = list(&root);
        assert_eq!(mods.len(), 2, "{:?}", mods);

        // The enabled mods come first, the others follow by name.
        assert_eq!(mods[0].name, "sodium.jar");
        assert!(mods[0].enabled);
        assert_eq!(mods[0].size, 3);

        assert_eq!(mods[1].name, "lithium.jar.disabled");
        assert_eq!(mods[1].mod_id, "lithium.jar");
        assert!(!mods[1].enabled);
        assert_eq!(mods[1].size, 7);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_mod_is_disabled_and_enabled_again_by_its_file_name() {
        let root = temporary("toggle");
        fs::write(root.join("sodium.jar"), b"jar").unwrap();

        let disabled = set_enabled(&root, "sodium.jar", false).unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.name, "sodium.jar.disabled");
        assert_eq!(disabled.mod_id, "sodium.jar");
        assert!(root.join("sodium.jar.disabled").exists());
        assert!(!root.join("sodium.jar").exists());

        let enabled = set_enabled(&root, "sodium.jar.disabled", true).unwrap();
        assert!(enabled.enabled);
        assert_eq!(enabled.name, "sodium.jar");
        assert!(root.join("sodium.jar").exists());
        assert!(!root.join("sodium.jar.disabled").exists());

        // Asking for the state the file already has does nothing.
        let again = set_enabled(&root, "sodium.jar", true).unwrap();
        assert!(again.enabled);
        assert!(root.join("sodium.jar").exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_name_that_leaves_the_folder_is_refused() {
        let root = temporary("escape");
        fs::write(root.join("sodium.jar"), b"jar").unwrap();

        for name in ["../sodium.jar", "sub/sodium.jar", "..", ""] {
            let error = set_enabled(&root, name, false).expect_err(name);
            assert!(error.contains("invalid mod name"), "{}: {}", name, error);
        }

        // The file of the parent folder was not touched.
        assert!(root.join("sodium.jar").exists());

        fs::remove_dir_all(&root).ok();
    }
}
