//! The directories the launcher writes to.

use std::env;
use std::fs;
use std::path::PathBuf;

/// Every directory the launcher needs.
#[derive(Debug, Clone)]
pub struct Paths {
    /// The root of the installation of the launcher.
    pub home: PathBuf,
    /// The settings and the accounts.
    pub data: PathBuf,
    /// The game folder, it can be shared with an existing installation.
    pub game: PathBuf,
    pub assets: PathBuf,
    pub asset_indexes: PathBuf,
    pub asset_objects: PathBuf,
    pub libraries: PathBuf,
    pub versions: PathBuf,
    pub mods: PathBuf,
    pub settings_file: PathBuf,
    pub accounts_file: PathBuf,
    /// The server list of the player.
    pub servers_file: PathBuf,
    /// The root of the servers that the launcher hosts locally.
    pub server_root: PathBuf,
    /// The `options.txt` of the game.
    pub options_file: PathBuf,
    pub logs: PathBuf,
}

impl Paths {
    /// Resolves the directories from the environment.
    ///
    /// `AETHER_HOME` moves the whole installation, `AETHER_SHARED_DIR` points the game folder at
    /// an existing `.minecraft` directory.
    pub fn resolve() -> Paths {
        let home = resolve_home();

        let game = match env::var("AETHER_SHARED_DIR") {
            Ok(value) if !value.is_empty() => PathBuf::from(value),
            _ => home.join(".minecraft"),
        };

        Paths {
            data: home.join("aether"),
            assets: game.join("assets"),
            asset_indexes: game.join("assets").join("indexes"),
            asset_objects: game.join("assets").join("objects"),
            libraries: game.join("libraries"),
            versions: game.join("versions"),
            mods: game.join("mods"),
            settings_file: home.join("aether").join("settings.json"),
            accounts_file: home.join("aether").join("accounts.json"),
            servers_file: home.join("aether").join("servers.json"),
            server_root: home.join("servers"),
            options_file: game.join("options.txt"),
            logs: home.join("aether").join("logs"),
            home,
            game,
        }
    }

    /// Creates every directory that does not exist yet.
    pub fn ensure(&self) -> std::io::Result<()> {
        for directory in [
            &self.home,
            &self.data,
            &self.game,
            &self.assets,
            &self.asset_indexes,
            &self.asset_objects,
            &self.libraries,
            &self.versions,
            &self.mods,
            &self.server_root,
            &self.logs,
        ] {
            fs::create_dir_all(directory)?;
        }
        Ok(())
    }
}

/// The name of the installation the launcher uses when nothing else is configured.
pub const DEFAULT_HOME_NAME: &str = "Aether";

/// The file that remembers the installation found next to the executable.
const REMEMBERED_HOME: &str = "home.txt";

/// The folder names an installation can wear beside the executable.
///
/// The list is fixed on purpose: the launcher never walks a directory tree to find a home, so a
/// folder it was not meant to use (the leftovers of a test, another launcher's copy, a download
/// folder) can never be adopted by accident.
const NEAR_NAMES: [&str; 4] = ["aether-home", ".aether", "Aether", "tools/aether-home"];

/// Reads the environment, then leaves the choice to `select_home` and writes down the installation it
/// had to discover, so that the next starts go straight to it.
fn resolve_home() -> PathBuf {
    let selection = select_home(
        env_value("AETHER_HOME").map(PathBuf::from),
        read_pointer(&pointer_directory()),
        home_directory().join(DEFAULT_HOME_NAME),
        env_value("AETHER_SHARED_DIR").is_some(),
        || search_anchor().and_then(|directory| find_installation_near(&directory)),
    );

    if selection.origin == Origin::Discovered {
        write_pointer(&pointer_directory(), &selection.path);
    }

    selection.path
}

/// Where a home directory came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Explicit,
    Remembered,
    Default,
    Discovered,
}

/// The home directory the launcher uses, and why.
#[derive(Debug, PartialEq, Eq)]
struct Selection {
    path: PathBuf,
    origin: Origin,
}

/// The priority itself, with the environment already read and the disk reached only through
/// `discover`:
///
/// 1. `AETHER_HOME`, unconditionally.
/// 2. The home remembered from a previous start, while it still is an installation.
/// 3. `~/Aether`, or `shared` when the game folder is configured elsewhere — that home owns the
///    settings, so looking for another one would lose them.
/// 4. The installation found beside the executable, the only one worth remembering.
///
/// Keeping this separate from the environment makes the order testable without touching a single
/// variable of the process.
fn select_home(
    explicit: Option<PathBuf>,
    remembered: Option<PathBuf>,
    default: PathBuf,
    shared: bool,
    discover: impl FnOnce() -> Option<PathBuf>,
) -> Selection {
    if let Some(path) = explicit {
        return Selection {
            path,
            origin: Origin::Explicit,
        };
    }

    if let Some(path) = remembered.filter(|home| is_installation(home)) {
        return Selection {
            path,
            origin: Origin::Remembered,
        };
    }

    if shared || is_installation(&default) {
        return Selection {
            path: default,
            origin: Origin::Default,
        };
    }

    match discover() {
        Some(path) => Selection {
            path,
            origin: Origin::Discovered,
        },
        None => Selection {
            path: default,
            origin: Origin::Default,
        },
    }
}

/// A variable of the environment, when it is set and not empty.
fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

/// The directory the search starts from.
///
/// A shell that embeds the core cannot let the core use its own path: the embedded copy is written to
/// the application data, far from the installation it was delivered with. It therefore passes
/// `AETHER_ANCHOR`, the folder of the shell itself. A core started on its own falls back to its own
/// folder.
fn search_anchor() -> Option<PathBuf> {
    env_value("AETHER_ANCHOR").map(PathBuf::from).or_else(|| {
        env::current_exe()
            .ok()
            .and_then(|executable| executable.parent().map(PathBuf::from))
    })
}

/// Whether a directory is the home of a usable installation.
///
/// Two marks, both required: the launcher's own `aether/settings.json`, which no other folder has,
/// and at least one downloaded version with its json. The settings file is what keeps the search away
/// from an unrelated `.minecraft` — another launcher's copy, a server test folder, a folder that only
/// looks like a home. A home left empty by a previous start has neither mark.
pub fn is_installation(home: &std::path::Path) -> bool {
    home.join("aether").join("settings.json").is_file()
        && has_downloaded_version(&home.join(".minecraft").join("versions"))
}

/// Whether a `versions` folder holds at least one version with its json.
fn has_downloaded_version(versions: &std::path::Path) -> bool {
    fs::read_dir(versions)
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| entry.path().is_dir() && contains_json(&entry.path()))
        })
        .unwrap_or(false)
}

fn contains_json(directory: &std::path::Path) -> bool {
    fs::read_dir(directory)
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|extension| extension.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Looks for an installation beside the executable: in its own folder, in its parent, and in the
/// few folders a distribution actually uses.
///
/// A portable build is unpacked beside its installation (`<exe>/aether-home/`) or inside a source
/// tree (`dist/` beside `tools/aether-home/`). Every candidate is a fixed path, so the launcher only
/// ever asks whether *these* folders are homes — it never browses the disk, and an unrelated folder
/// cannot be picked up.
pub fn find_installation_near(directory: &std::path::Path) -> Option<PathBuf> {
    for root in [Some(directory), directory.parent()].into_iter().flatten() {
        if is_installation(root) {
            return Some(root.to_path_buf());
        }

        for name in NEAR_NAMES {
            let candidate = root.join(name);
            if is_installation(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

/// Writes the installation remembered by the next starts.
fn write_pointer(directory: &std::path::Path, home: &std::path::Path) {
    let _ = fs::create_dir_all(directory);
    let _ = fs::write(
        directory.join(REMEMBERED_HOME),
        home.to_string_lossy().as_bytes(),
    );
}

/// The folder that remembers the installation between the runs.
///
/// The pointer cannot live in the installation itself — resolving the home is what needs it — so it
/// sits in the user configuration, outside any home and outside the executable folder, which may be
/// read-only once installed.
fn pointer_directory() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            if !appdata.is_empty() {
                return PathBuf::from(appdata).join(DEFAULT_HOME_NAME);
            }
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(config) = env::var("XDG_CONFIG_HOME") {
            if !config.is_empty() {
                return PathBuf::from(config).join("aether");
            }
        }
        if let Ok(home) = env::var("HOME") {
            if !home.is_empty() {
                return PathBuf::from(home).join(".config").join("aether");
            }
        }
    }

    home_directory().join(DEFAULT_HOME_NAME)
}

fn read_pointer(directory: &std::path::Path) -> Option<PathBuf> {
    let text = fs::read_to_string(directory.join(REMEMBERED_HOME)).ok()?;
    let value = text.trim();
    if value.is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

/// The home directory of the current user.
pub fn home_directory() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(profile) = env::var("USERPROFILE") {
            if !profile.is_empty() {
                return PathBuf::from(profile);
            }
        }
        let drive = env::var("HOMEDRIVE").unwrap_or_default();
        let path = env::var("HOMEPATH").unwrap_or_default();
        if !drive.is_empty() && !path.is_empty() {
            return PathBuf::from(format!("{}{}", drive, path));
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(home) = env::var("HOME") {
            if !home.is_empty() {
                return PathBuf::from(home);
            }
        }
    }

    PathBuf::from(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("aether-paths-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn install_version(home: &std::path::Path, id: &str) {
        let version = home.join(".minecraft").join("versions").join(id);
        fs::create_dir_all(&version).unwrap();
        fs::write(version.join(format!("{}.json", id)), "{}").unwrap();
        set_as_aether_home(home);
    }

    /// Only the launcher writes this file, so it marks a folder as an Aether home.
    fn set_as_aether_home(home: &std::path::Path) {
        let data = home.join("aether");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("settings.json"), "{}").unwrap();
    }

    #[test]
    fn an_empty_home_is_not_an_installation() {
        let home = scratch("empty-home");
        for directory in ["versions", "assets", "libraries", "mods"] {
            fs::create_dir_all(home.join(".minecraft").join(directory)).unwrap();
        }

        assert!(!is_installation(&home));
    }

    #[test]
    fn a_downloaded_version_makes_an_installation() {
        let home = scratch("installed-home");
        install_version(&home, "1.21.11");

        assert!(is_installation(&home));
    }

    #[test]
    fn a_settings_file_without_a_version_is_not_an_installation() {
        let home = scratch("settings-only");
        set_as_aether_home(&home);
        fs::create_dir_all(home.join(".minecraft").join("versions")).unwrap();

        assert!(!is_installation(&home));
    }

    #[test]
    fn another_launcher_game_folder_is_not_an_installation() {
        let folder = scratch("foreign-game-folder");
        let version = folder.join(".minecraft").join("versions").join("1.21.11");
        fs::create_dir_all(&version).unwrap();
        fs::write(version.join("1.21.11.json"), "{}").unwrap();

        assert!(
            !is_installation(&folder),
            "a game folder that the launcher never wrote was taken for its home"
        );
    }

    #[test]
    fn a_downloaded_asset_index_alone_is_not_an_installation() {
        let home = scratch("assets-only");
        set_as_aether_home(&home);
        let indexes = home.join(".minecraft").join("assets").join("indexes");
        fs::create_dir_all(&indexes).unwrap();
        fs::write(indexes.join("29.json"), "{}").unwrap();

        assert!(!is_installation(&home));
    }

    #[test]
    fn the_remembered_home_is_written_and_read_back() {
        let directory = scratch("pointer");
        let home = directory.join("aether-home");

        assert_eq!(read_pointer(&directory), None);
        write_pointer(&directory, &home);
        assert_eq!(read_pointer(&directory), Some(home));
    }

    #[test]
    fn an_empty_pointer_file_is_ignored() {
        let directory = scratch("empty-pointer");
        fs::create_dir_all(directory.join("aether")).unwrap();
        fs::write(directory.join("aether").join(REMEMBERED_HOME), "  \n").unwrap();

        assert_eq!(read_pointer(&directory.join("aether")), None);
    }

    #[test]
    fn the_installation_beside_the_executable_is_found_from_a_source_tree() {
        let root = scratch("source-tree");
        let executable = root.join("dist");
        fs::create_dir_all(&executable).unwrap();
        let homes = root.join("tools").join("aether-home");
        install_version(&homes, "1.21.11");

        assert_eq!(
            find_installation_near(&executable),
            Some(homes),
            "the installation beside dist was not found"
        );
    }

    #[test]
    fn a_home_right_beside_the_executable_wins_over_a_deeper_one() {
        let root = scratch("beside-exe");
        let executable = root.join("dist");
        fs::create_dir_all(&executable).unwrap();
        let beside = executable.join("aether-home");
        install_version(&beside, "1.21.11");
        let deeper = root.join("tools").join("aether-home");
        install_version(&deeper, "1.21.11");

        assert_eq!(find_installation_near(&executable), Some(beside));
    }

    #[test]
    fn an_unrelated_neighbour_is_never_adopted() {
        let root = scratch("no-neighbour");
        let executable = root.join("dist");
        fs::create_dir_all(&executable).unwrap();
        // Other homes that exist in the neighbourhood, under names the launcher never looks for.
        install_version(&root.join("target").join("fake"), "1.21.11");
        install_version(&root.join("leftovers").join("installed-home"), "1.21.11");

        assert_eq!(find_installation_near(&executable), None);
    }

    #[test]
    fn an_empty_neighbourhood_finds_nothing() {
        let root = scratch("nothing-near");
        let executable = root.join("dist");
        fs::create_dir_all(&executable).unwrap();
        fs::create_dir_all(root.join("tools").join("aether-home").join(".minecraft")).unwrap();

        assert_eq!(find_installation_near(&executable), None);
    }

    /// A home that the launcher made: settings and a downloaded version.
    fn usable_home(name: &str) -> PathBuf {
        let home = scratch(name);
        install_version(&home, "1.21.11");
        home
    }

    #[test]
    fn the_explicit_home_wins_over_everything() {
        let explicit = scratch("priority-explicit");

        let selection = select_home(
            Some(explicit.clone()),
            Some(usable_home("priority-remembered")),
            usable_home("priority-default"),
            false,
            || Some(usable_home("priority-discovered")),
        );

        assert_eq!(selection.path, explicit);
        assert_eq!(selection.origin, Origin::Explicit);
    }

    #[test]
    fn the_remembered_home_comes_before_the_default_one() {
        let remembered = usable_home("priority-remembered-2");

        let selection = select_home(
            None,
            Some(remembered.clone()),
            usable_home("priority-default-2"),
            false,
            || Some(usable_home("priority-discovered-2")),
        );

        assert_eq!(selection.path, remembered);
        assert_eq!(selection.origin, Origin::Remembered);
    }

    #[test]
    fn a_remembered_home_that_disappeared_is_skipped() {
        let gone = scratch("priority-gone");
        let default = usable_home("priority-default-3");

        let selection = select_home(None, Some(gone), default.clone(), false, || {
            Some(usable_home("priority-discovered-3"))
        });

        assert_eq!(selection.path, default);
        assert_eq!(selection.origin, Origin::Default);
    }

    #[test]
    fn a_usable_default_home_comes_before_the_discovery() {
        let default = usable_home("priority-default-4");

        let selection = select_home(None, None, default.clone(), false, || {
            Some(usable_home("priority-discovered-4"))
        });

        assert_eq!(selection.path, default);
        assert_eq!(selection.origin, Origin::Default);
    }

    #[test]
    fn the_discovery_is_used_when_the_default_home_is_empty() {
        let default = scratch("priority-empty-default");
        let discovered = usable_home("priority-discovered-5");

        let selection = select_home(None, None, default, false, || Some(discovered.clone()));

        assert_eq!(selection.path, discovered);
        assert_eq!(
            selection.origin,
            Origin::Discovered,
            "only a discovered home is worth remembering"
        );
    }

    #[test]
    fn a_shared_game_folder_disables_the_discovery() {
        let default = scratch("priority-shared-default");

        let selection = select_home(None, None, default.clone(), true, || {
            panic!("the discovery must not run when the game folder is configured")
        });

        assert_eq!(selection.path, default);
        assert_eq!(selection.origin, Origin::Default);
    }

    #[test]
    fn nothing_found_leaves_the_default_home() {
        let default = scratch("priority-nothing");

        let selection = select_home(None, None, default.clone(), false, || None);

        assert_eq!(selection.path, default);
        assert_eq!(selection.origin, Origin::Default);
    }
}
