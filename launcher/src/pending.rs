//! The journal of an installation that started but did not finish.
//!
//! Every download is verified by its SHA-1 before it is moved into place, so an interrupted run
//! never leaves a corrupt file: the next run skips every file that is already correct and fetches
//! the rest. The journal is what makes that resumption visible: it names the version being built,
//! the step that was running when the process stopped and how far it got, so the launcher can say
//! at its next start that an installation is waiting instead of pretending nothing happened.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::settings::write_atomic;

/// The name of the journal, at the root of the data directory.
pub const PENDING: &str = "install.pending.json";

/// How deep the cleanup of the partial files walks, a guard against a junction loop.
const MAX_PART_DEPTH: usize = 8;

/// The state of an installation that has not finished.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingInstall {
    /// `game` for a version of the game, `mods` for the optimisation pack.
    pub kind: String,
    /// The version of the game the installation builds.
    pub version: String,
    #[serde(default)]
    pub loader: String,
    #[serde(default)]
    pub fabric_api: String,
    /// The projects the dependency graph resolved, for a pack installation.
    #[serde(default)]
    pub slugs: Vec<String>,
    /// The step that was running when the journal was last written.
    pub step: String,
    /// The number of finished steps.
    pub completed: usize,
    /// The number of steps of the run.
    pub total: usize,
    /// Seconds since the Unix epoch, the interface says how old the run is.
    #[serde(default)]
    pub started_at: u64,
}

impl PendingInstall {
    /// A journal for a run that is about to start.
    pub fn new(kind: &str, version: &str) -> PendingInstall {
        PendingInstall {
            kind: kind.to_string(),
            version: version.to_string(),
            loader: String::new(),
            fabric_api: String::new(),
            slugs: Vec::new(),
            step: "starting".to_string(),
            completed: 0,
            total: 0,
            started_at: now(),
        }
    }
}

/// The current time, in seconds since the Unix epoch.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// The journal of the launcher.
pub fn path(paths: &Paths) -> PathBuf {
    paths.data.join(PENDING)
}

/// Writes the journal, replacing the previous one.
///
/// The file is written atomically: a process that dies while the journal is being written leaves
/// either the previous journal or the new one, never a truncated document that the next start
/// cannot read.
pub fn begin(paths: &Paths, pending: &PendingInstall) -> Result<(), String> {
    let content = serde_json::to_string_pretty(pending).map_err(|error| error.to_string())?;
    write_atomic(&path(paths), &content)
        .map_err(|error| format!("could not write {}: {}", PENDING, error))
}

/// Records the step that is running, without ever failing the installation itself.
///
/// A journal that cannot be written only costs the resumption hint, it must not turn a download
/// that is going well into an error.
pub fn record(paths: &Paths, step: &str, completed: usize) {
    let Some(mut pending) = read(paths) else {
        return;
    };

    pending.step = step.to_string();
    pending.completed = completed;
    let _ = begin(paths, &pending);
}

/// Removes the journal, the run is over.
pub fn close(paths: &Paths) {
    let _ = fs::remove_file(path(paths));
}

/// Reads the journal, `None` when there is none or when it cannot be understood.
///
/// A corrupt journal is not an installation: it is a leftover, and the next run overwrites it.
pub fn read(paths: &Paths) -> Option<PendingInstall> {
    let content = fs::read_to_string(path(paths)).ok()?;
    let pending: PendingInstall = serde_json::from_str(&content).ok()?;

    if pending.kind.trim().is_empty() {
        return None;
    }

    Some(pending)
}

/// Removes the journal and every `.part` file an interrupted download left behind.
///
/// The `.part` files are the only state a download keeps beside its destination; deleting them
/// frees the space of a run the player does not want to resume.
pub fn discard(paths: &Paths) -> Result<usize, String> {
    let mut removed = 0;

    match fs::remove_file(path(paths)) {
        Ok(()) => removed += 1,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("could not remove {}: {}", PENDING, error)),
    }

    for root in [
        &paths.mods,
        &paths.versions,
        &paths.libraries,
        &paths.assets,
    ] {
        removed += remove_partials(root, 0);
    }

    Ok(removed)
}

/// Counts the `.part` files removed below one directory.
fn remove_partials(directory: &Path, depth: usize) -> usize {
    if depth > MAX_PART_DEPTH {
        return 0;
    }

    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };

    let mut removed = 0;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };

        // A symbolic link is never followed: the cleanup stays inside the managed folders.
        if kind.is_symlink() {
            continue;
        }

        let path = entry.path();
        if kind.is_dir() {
            removed += remove_partials(&path, depth + 1);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("part"))
            && fs::remove_file(&path).is_ok()
        {
            removed += 1;
        }
    }

    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(root: &Path) -> Paths {
        Paths {
            home: root.to_path_buf(),
            data: root.join("aether"),
            game: root.join(".minecraft"),
            assets: root.join(".minecraft/assets"),
            asset_indexes: root.join(".minecraft/assets/indexes"),
            asset_objects: root.join(".minecraft/assets/objects"),
            libraries: root.join(".minecraft/libraries"),
            versions: root.join(".minecraft/versions"),
            mods: root.join(".minecraft/mods"),
            settings_file: root.join("aether/settings.json"),
            accounts_file: root.join("aether/accounts.json"),
            servers_file: root.join("aether/servers.json"),
            server_root: root.join("servers"),
            options_file: root.join(".minecraft/options.txt"),
            logs: root.join("aether/logs"),
        }
    }

    fn scratch(name: &str) -> (PathBuf, Paths) {
        let root = std::env::temp_dir().join(format!("aether-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = test_paths(&root);
        fs::create_dir_all(&paths.data).unwrap();
        (root, paths)
    }

    #[test]
    fn a_journal_lives_until_the_run_closes_it() {
        let (root, paths) = scratch("pending-life");

        let mut journal = PendingInstall::new("game", "1.21.11");
        journal.loader = "0.20.1".to_string();
        journal.total = 5;
        begin(&paths, &journal).unwrap();

        let read_back = read(&paths).expect("the journal was written");
        assert_eq!(read_back.kind, "game");
        assert_eq!(read_back.version, "1.21.11");
        assert_eq!(read_back.loader, "0.20.1");
        assert_eq!(read_back.step, "starting");
        assert!(read_back.started_at > 0);

        record(&paths, "Downloading assets", 3);
        let updated = read(&paths).unwrap();
        assert_eq!(updated.step, "Downloading assets");
        assert_eq!(updated.completed, 3);
        assert_eq!(updated.version, "1.21.11", "the version was lost");

        close(&paths);
        assert!(read(&paths).is_none(), "a finished run left a journal");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_corrupt_journal_is_a_leftover_not_an_installation() {
        let (root, paths) = scratch("pending-corrupt");
        fs::write(path(&paths), "{ not json").unwrap();
        assert!(read(&paths).is_none());

        fs::write(
            path(&paths),
            r#"{"kind":"","version":"1.21.11","step":"x","completed":0,"total":0}"#,
        )
        .unwrap();
        assert!(read(&paths).is_none(), "a journal without a kind is kept");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn discarding_a_run_removes_the_journal_and_every_partial_file() {
        let (root, paths) = scratch("pending-discard");
        fs::create_dir_all(&paths.mods).unwrap();
        fs::create_dir_all(paths.versions.join("1.21.11")).unwrap();
        fs::create_dir_all(&paths.assets).unwrap();
        fs::create_dir_all(&paths.libraries).unwrap();

        begin(&paths, &PendingInstall::new("mods", "1.21.11")).unwrap();
        fs::write(paths.mods.join("sodium.jar.part"), b"half").unwrap();
        fs::write(paths.mods.join("sodium.jar"), b"whole").unwrap();
        fs::write(paths.versions.join("1.21.11/1.21.11.jar.part"), b"half").unwrap();
        fs::write(paths.assets.join("index.part"), b"half").unwrap();
        fs::write(paths.libraries.join("keep.jar"), b"whole").unwrap();

        let removed = discard(&paths).expect("the run was discarded");

        assert_eq!(removed, 4, "the journal and the three partial files");
        assert!(read(&paths).is_none());
        assert!(
            paths.mods.join("sodium.jar").exists(),
            "a complete jar was deleted"
        );
        assert!(paths.libraries.join("keep.jar").exists());
        assert!(!paths.mods.join("sodium.jar.part").exists());
        assert!(!paths.versions.join("1.21.11/1.21.11.jar.part").exists());
        assert!(!paths.assets.join("index.part").exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn discarding_without_a_run_is_not_an_error() {
        let (root, paths) = scratch("pending-empty");
        assert_eq!(discard(&paths).unwrap(), 0);
        fs::remove_dir_all(&root).ok();
    }
}
