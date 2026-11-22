//! The settings of the launcher and the detection of a Java runtime.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::{home_directory, Paths};

/// The accent colour that is used when nothing was configured.
pub const DEFAULT_ACCENT: &str = "#E7E9EC";

/// The name of the profile that applies when nothing else does.
pub const DEFAULT_PROFILE: &str = "Default";

/// A named way to play: the version, the loader and the resources it needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub game_version: String,
    pub loader_version: String,
    pub max_memory: u32,
    pub min_memory: u32,
    pub window_width: u32,
    pub window_height: u32,
}

impl Profile {
    /// The profile made of the flat settings, the default way to play.
    pub fn from_settings(settings: &Settings) -> Profile {
        Profile::named(DEFAULT_PROFILE, settings)
    }

    /// A profile that keeps the values of the settings under another name.
    pub fn named(name: &str, settings: &Settings) -> Profile {
        Profile {
            name: name.to_string(),
            game_version: settings.game_version.clone(),
            loader_version: settings.loader_version.clone(),
            max_memory: settings.max_memory,
            min_memory: settings.min_memory,
            window_width: settings.window_width,
            window_height: settings.window_height,
        }
    }
}

/// The settings of the launcher.
///
/// A field that an older file does not carry falls back to its default instead of resetting the
/// whole file: adding a setting must never wipe the configuration of a player.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Settings {
    /// The path of the Java runtime, empty means it is detected automatically.
    pub java_path: String,
    /// The maximum size of the heap in megabytes.
    pub max_memory: u32,
    /// The initial size of the heap in megabytes.
    pub min_memory: u32,
    /// The width of the window.
    pub window_width: u32,
    /// The height of the window.
    pub window_height: u32,
    /// The Minecraft version that is played.
    pub game_version: String,
    /// The Fabric loader that is used.
    pub loader_version: String,
    /// The version of the Fabric API.
    pub fabric_api_version: String,
    /// The colourway of the interface: graphite, oxide, field or blueprint.
    pub theme: String,
    /// The accent colour selected by the player, empty/default keeps the colourway accent.
    pub accent_color: String,
    /// The spacing density of the launcher interface: roomy or compact.
    pub ui_density: String,
    /// The background linework: contour, draft or plain.
    pub surface_style: String,
    /// The navigation shape: wide or rail.
    pub navigation_style: String,
    /// Whether the output of the game is shown.
    pub show_game_output: bool,
    /// Whether the G1 garbage collector is used.
    pub use_g1gc: bool,
    /// The preset of JVM flags the game starts with: `balanced`, `performance` or `ultra`.
    pub jvm_preset: String,
    /// The preset of `options.txt` that was last applied, `vanilla` when nothing was written.
    pub options_preset: String,
    /// The major version of Java the game needs.
    pub required_java_version: u32,
    /// The named profiles, `Default` included.
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// The profile that is used when the account does not choose one.
    #[serde(default)]
    pub active_profile: String,
}

impl Default for Settings {
    fn default() -> Self {
        let mut settings = Settings {
            java_path: String::new(),
            max_memory: 4096,
            min_memory: 1024,
            window_width: 854,
            window_height: 480,
            game_version: "1.21.11".to_string(),
            loader_version: "0.20.1".to_string(),
            fabric_api_version: "0.141.6+1.21.11".to_string(),
            theme: "graphite".to_string(),
            accent_color: DEFAULT_ACCENT.to_string(),
            ui_density: "roomy".to_string(),
            surface_style: "contour".to_string(),
            navigation_style: "wide".to_string(),
            show_game_output: true,
            use_g1gc: true,
            jvm_preset: "balanced".to_string(),
            options_preset: "vanilla".to_string(),
            required_java_version: 21,
            profiles: Vec::new(),
            active_profile: String::new(),
        };

        settings.migrate();
        settings
    }
}

impl Settings {
    /// Gives the file of an older launcher its default profile.
    ///
    /// The flat values of the settings are the default way to play: they become the `Default`
    /// profile once, so that a file written before the profiles existed keeps working.
    fn migrate(&mut self) -> bool {
        let mut changed = false;

        // The former launcher only had one dark palette. Upgrade that value once to the
        // new material-based default; custom colourways introduced by Identity Studio stay intact.
        if self.theme.eq_ignore_ascii_case("dark") {
            self.theme = "graphite".to_string();
            changed = true;
        }

        if self.active_profile.is_empty() {
            self.active_profile = DEFAULT_PROFILE.to_string();
            changed = true;
        }

        if self.profiles.is_empty() {
            self.profiles.push(Profile::from_settings(self));
            changed = true;
        }

        changed
    }

    /// The profile with that name, the case is ignored.
    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|profile| profile.name.eq_ignore_ascii_case(name))
    }

    /// The profile of an account: the one it chose, then the active one, then the default values.
    pub fn profile_for(&self, chosen: Option<&str>) -> Profile {
        chosen
            .and_then(|name| self.profile(name))
            .or_else(|| self.profile(&self.active_profile))
            .cloned()
            .unwrap_or_else(|| Profile::from_settings(self))
    }

    /// Adds a profile, a profile with the same name is replaced.
    ///
    /// The values are kept in the range the interface offers, whatever the caller sends. Writing
    /// the `Default` profile writes through to the flat settings it stands for.
    pub fn put_profile(&mut self, mut profile: Profile) {
        profile.max_memory = profile.max_memory.max(256);
        profile.min_memory = profile.min_memory.max(256);
        profile.max_memory = profile.max_memory.max(profile.min_memory);
        profile.window_width = profile.window_width.max(320);
        profile.window_height = profile.window_height.max(240);

        if profile.name.eq_ignore_ascii_case(DEFAULT_PROFILE) {
            profile.name = DEFAULT_PROFILE.to_string();
            self.game_version = profile.game_version.clone();
            self.loader_version = profile.loader_version.clone();
            self.max_memory = profile.max_memory;
            self.min_memory = profile.min_memory;
            self.window_width = profile.window_width;
            self.window_height = profile.window_height;
        }

        self.profiles
            .retain(|existing| !existing.name.eq_ignore_ascii_case(&profile.name));
        self.profiles.push(profile);
    }

    /// Removes a profile. `Default` stays, the flat settings are written through it.
    pub fn remove_profile(&mut self, name: &str) -> Result<(), String> {
        if name.eq_ignore_ascii_case(DEFAULT_PROFILE) {
            return Err("l'instance par défaut ne peut pas être supprimée".to_string());
        }

        if self.profile(name).is_none() {
            return Err(format!("unknown profile: {}", name));
        }

        self.profiles
            .retain(|profile| !profile.name.eq_ignore_ascii_case(name));

        if self.active_profile.eq_ignore_ascii_case(name) {
            self.active_profile = DEFAULT_PROFILE.to_string();
        }

        Ok(())
    }

    /// The default profile is the flat settings, they are the same values.
    fn sync_default(&mut self) {
        let default = Profile::from_settings(self);

        match self
            .profiles
            .iter_mut()
            .find(|profile| profile.name.eq_ignore_ascii_case(DEFAULT_PROFILE))
        {
            Some(profile) => *profile = default,
            None => self.profiles.insert(0, default),
        }
    }

    /// Applies the `key=value` pairs of the command line to a copy of the settings.
    ///
    /// The key is matched without regard to the case, `useG1GC` therefore addresses the same
    /// setting as `useG1gc`. A value that is valid JSON is used as it is (numbers and booleans),
    /// every other value stays a string. A key that matches no setting is an error that lists the
    /// valid ones, so a typo is never silently ignored.
    pub fn patched(&self, pairs: &[(String, String)]) -> Result<Settings, String> {
        let mut raw = serde_json::to_value(self).map_err(|error| error.to_string())?;
        let names = Self::names();
        let object = raw
            .as_object_mut()
            .ok_or_else(|| "the settings are not an object".to_string())?;

        for (key, value) in pairs {
            let name = names
                .iter()
                .find(|name| name.eq_ignore_ascii_case(key))
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "unknown setting: {}, the settings are: {}",
                        key,
                        names.join(", ")
                    )
                })?;
            object.insert(name, coerce(value));
        }

        serde_json::from_value(raw).map_err(|error| format!("invalid value: {}", error))
    }

    /// The names of the settings, as they are written in the settings file.
    fn names() -> Vec<String> {
        serde_json::to_value(Settings::default())
            .ok()
            .and_then(|value| {
                value
                    .as_object()
                    .map(|object| object.keys().cloned().collect::<Vec<String>>())
            })
            .unwrap_or_default()
    }

    /// Reads the settings, a missing or broken file produces the defaults.
    ///
    /// A file written by an older launcher has no profile: it is upgraded on the spot, the flat
    /// values becoming the `Default` profile.
    pub fn load(path: &Path) -> Settings {
        let mut settings = match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str::<Settings>(&content).unwrap_or_default(),
            Err(_) => return Settings::default(),
        };

        if settings.migrate() {
            settings.save(path).ok();
        }

        settings
    }

    /// Writes the settings.
    ///
    /// The default profile is written from the flat settings, the two can never disagree.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut copy = self.clone();
        copy.sync_default();

        let content = serde_json::to_string_pretty(&copy)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        write_atomic(path, &content)
    }
}

/// A process-local sequence makes temporary names distinct even when different files share
/// the same stem (for example, accounts.json and accounts.credentials).
static NEXT_TEMP_FILE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn temporary_path(path: &Path, sequence: u64) -> PathBuf {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("aether-data");
    path.with_file_name(format!(
        ".{}.{}.{}.tmp",
        filename,
        std::process::id(),
        sequence
    ))
}

/// Writes a file in one step.
///
/// The content is written to a unique sibling file and moved into place. Different files with the
/// same stem never share a temporary path, and concurrent writes cannot truncate each other's data.
pub fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut temporary = None;
    let mut file = None;
    for _ in 0..32 {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let candidate = temporary_path(path, sequence);
        match OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(created) => {
                temporary = Some(candidate);
                file = Some(created);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    let temporary = temporary.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique temporary file",
        )
    })?;

    let write_result = (|| {
        let mut file = file.expect("a temporary file was created");
        file.write_all(content.as_bytes())?;
        file.sync_all()
    })();

    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    Ok(())
}

/// Reads a value of the command line, a value that is valid JSON keeps its type.
fn coerce(value: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(parsed) if !parsed.is_string() => parsed,
        _ => serde_json::Value::String(value.to_string()),
    }
}

/// Reads the major version of a Java runtime.
///
/// Java writes the version to the standard error stream.
pub fn read_java_major_version(executable: &Path) -> Option<u32> {
    let output = Command::new(executable).arg("-version").output().ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    let start = text.find("version \"")? + "version \"".len();
    let version: String = text[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
    version.parse().ok()
}

/// The directories that are searched for a Java runtime.
fn java_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("C:\\Program Files\\Java"),
        PathBuf::from("C:\\Program Files\\Eclipse Adoptium"),
        PathBuf::from("C:\\Program Files\\Microsoft"),
        PathBuf::from("C:\\Program Files\\Amazon Corretto"),
        PathBuf::from("C:\\Program Files\\Zulu"),
        PathBuf::from("C:\\Program Files (x86)\\Java"),
        PathBuf::from("/usr/lib/jvm"),
        PathBuf::from("/Library/Java/JavaVirtualMachines"),
        PathBuf::from("/opt/java"),
        home_directory().join(".gradle").join("jdks"),
    ];

    // The official launcher ships its own runtime.
    if let Ok(appdata) = std::env::var("APPDATA") {
        let runtime = PathBuf::from(appdata)
            .join(".minecraft")
            .join("runtime")
            .join("java-runtime-delta");
        roots.insert(0, runtime);
    }

    roots
}

/// Locates a usable Java runtime.
///
/// The launcher prefers an explicit setting, then a runtime with exactly the required version, then
/// the oldest runtime that is new enough. A runtime that is too old is never used, the game would
/// refuse to start with it.
pub fn detect_java(preferred: &str, required_major: u32) -> Option<PathBuf> {
    if !preferred.is_empty() {
        let path = PathBuf::from(preferred);
        if path.exists() {
            if let Some(major) = read_java_major_version(&path) {
                if major >= required_major {
                    return Some(path);
                }
            }
        }
    }

    let executable_name = if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    };

    let mut usable: Vec<(PathBuf, u32)> = Vec::new();

    for root in java_roots() {
        let entries = match std::fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let candidate = entry.path().join("bin").join(executable_name);
            if !candidate.exists() {
                continue;
            }
            if let Some(major) = read_java_major_version(&candidate) {
                if major >= required_major && !usable.iter().any(|(_, seen)| *seen == major) {
                    usable.push((candidate, major));
                }
            }
        }
    }

    usable.sort_by_key(|(_, major)| *major);

    if let Some((path, _major)) = usable.iter().find(|(_, major)| *major == required_major) {
        return Some(path.clone());
    }

    usable.first().map(|(path, _)| path.clone())
}

/// The path of the jar of the client.
///
/// It is searched next to the launcher and in the folder of the project, so that the launcher works
/// both when it is started from the sources and when it is installed.
pub fn find_client_jar(paths: &Paths) -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("AETHER_MOD_JAR") {
        let path = PathBuf::from(custom);
        if path.exists() {
            return Some(path);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("aether-client.jar"));
            // target/release/../../.. is the root of the project.
            candidates.push(directory.join("../../../aether-client/build/libs/aether-client-1.0.0.jar"));
            candidates.push(directory.join("../../aether-client/build/libs/aether-client-1.0.0.jar"));
        }
    }

    if let Ok(current) = std::env::current_dir() {
        candidates.push(current.join("../aether-client/build/libs/aether-client-1.0.0.jar"));
        candidates.push(current.join("aether-client/build/libs/aether-client-1.0.0.jar"));
    }

    if let Some(root) = paths.home.parent() {
        candidates.push(root.join("aether-client/build/libs/aether-client-1.0.0.jar"));
    }

    candidates
        .into_iter()
        .find(|path| path.exists() && path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_sane() {
        let settings = Settings::default();
        assert_eq!(settings.game_version, "1.21.11");
        assert_eq!(settings.loader_version, "0.20.1");
        assert_eq!(settings.required_java_version, 21);
        assert_eq!(settings.max_memory, 4096);
        assert_eq!(settings.theme, "graphite");
        assert_eq!(settings.ui_density, "roomy");
        assert_eq!(settings.surface_style, "contour");
        assert_eq!(settings.navigation_style, "wide");
    }

    #[test]
    fn a_legacy_dark_theme_is_migrated_to_graphite_and_saved() {
        let directory = std::env::temp_dir().join(format!("aether-theme-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("settings.json");

        std::fs::write(
            &file,
            r##"{"theme":"dark","accentColor":"#3B82F6","maxMemory":8192}"##,
        )
        .unwrap();

        let settings = Settings::load(&file);
        assert_eq!(settings.theme, "graphite");
        assert_eq!(settings.max_memory, 8192);
        assert_eq!(settings.accent_color, "#3B82F6");

        let persisted: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(persisted["theme"], "graphite");
        assert_eq!(persisted["accentColor"], "#3B82F6");

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_patch_coerces_the_values_and_ignores_the_case_of_the_key() {
        let settings = Settings::default();

        let patched = settings
            .patched(&[
                ("maxMemory".to_string(), "8192".to_string()),
                ("useG1GC".to_string(), "false".to_string()),
                ("javaPath".to_string(), "C:/java/bin/java.exe".to_string()),
            ])
            .expect("the patch was applied");

        assert_eq!(patched.max_memory, 8192);
        assert!(!patched.use_g1gc);
        assert_eq!(patched.java_path, "C:/java/bin/java.exe");
        // The other settings are untouched.
        assert_eq!(patched.min_memory, settings.min_memory);
        assert_eq!(patched.game_version, settings.game_version);
    }

    #[test]
    fn appearance_settings_are_patched_and_survive_serialization() {
        let patched = Settings::default()
            .patched(&[
                ("theme".to_string(), "oxide".to_string()),
                ("accentColor".to_string(), "#D58A54".to_string()),
                ("uiDensity".to_string(), "compact".to_string()),
                ("surfaceStyle".to_string(), "draft".to_string()),
                ("navigationStyle".to_string(), "rail".to_string()),
            ])
            .expect("the appearance patch is valid");

        let value = serde_json::to_value(&patched).unwrap();
        let restored: Settings = serde_json::from_value(value).unwrap();

        assert_eq!(restored.theme, "oxide");
        assert_eq!(restored.accent_color, "#D58A54");
        assert_eq!(restored.ui_density, "compact");
        assert_eq!(restored.surface_style, "draft");
        assert_eq!(restored.navigation_style, "rail");
    }

    #[test]
    fn an_unknown_setting_is_reported_with_the_valid_ones() {
        let error = Settings::default()
            .patched(&[("maxMemori".to_string(), "8192".to_string())])
            .expect_err("the typo was rejected");

        assert!(error.contains("unknown setting: maxMemori"));
        assert!(error.contains("maxMemory"));
    }

    #[test]
    fn a_patch_that_does_not_fit_the_settings_is_rejected() {
        let error = Settings::default()
            .patched(&[("maxMemory".to_string(), "a lot".to_string())])
            .expect_err("the value was rejected");

        assert!(error.contains("invalid value"));
    }

    #[test]
    fn a_file_is_replaced_in_one_step() {
        let directory = std::env::temp_dir().join(format!("aether-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("accounts.json");

        write_atomic(&file, "{\"accounts\":[]}").unwrap();
        write_atomic(&file, "{\"accounts\":[1]}").unwrap();

        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "{\"accounts\":[1]}"
        );
        assert!(
            !directory.join("accounts.tmp").exists(),
            "a temporary file was left behind"
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn atomic_temporary_paths_are_unique_for_files_with_the_same_stem() {
        let root = std::env::temp_dir().join(format!("aether-atomic-paths-{}", std::process::id()));
        let accounts = root.join("accounts.json");
        let credentials = root.join("accounts.credentials");

        assert_ne!(
            temporary_path(&accounts, 7),
            temporary_path(&credentials, 7),
            "different files must not share a temporary path"
        );
        assert_ne!(
            temporary_path(&accounts, 7),
            temporary_path(&accounts, 8),
            "separate writes must not share a temporary path"
        );
    }

    #[test]
    fn atomic_writes_to_the_account_catalogue_and_vault_do_not_collide() {
        let directory = std::env::temp_dir().join(format!("aether-atomic-pair-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let accounts = directory.join("accounts.json");
        let credentials = directory.join("accounts.credentials");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));

        let first = {
            let barrier = barrier.clone();
            let accounts = accounts.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..32 {
                    write_atomic(&accounts, "{\"accounts\":[]}").unwrap();
                }
            })
        };
        let second = {
            let barrier = barrier.clone();
            let credentials = credentials.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..32 {
                    write_atomic(&credentials, "AETHER-CREDENTIALS-V1\nprotected").unwrap();
                }
            })
        };

        first.join().unwrap();
        second.join().unwrap();

        assert_eq!(std::fs::read_to_string(&accounts).unwrap(), "{\"accounts\":[]}");
        assert_eq!(
            std::fs::read_to_string(&credentials).unwrap(),
            "AETHER-CREDENTIALS-V1\nprotected"
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_profile_replaces_the_one_with_the_same_name() {
        let mut settings = Settings::default();
        let base = Profile::from_settings(&settings);

        settings.put_profile(Profile {
            name: "Cinematic".to_string(),
            max_memory: 8192,
            ..base.clone()
        });
        assert_eq!(settings.profiles.len(), 2);

        // The same name, another case: the profile is replaced, not added twice.
        settings.put_profile(Profile {
            name: "cinematic".to_string(),
            max_memory: 2048,
            ..base
        });
        assert_eq!(settings.profiles.len(), 2);
        assert_eq!(settings.profile("CINEMATIC").unwrap().max_memory, 2048);
    }

    #[test]
    fn a_profile_keeps_its_values_in_the_range_of_the_interface() {
        let mut settings = Settings::default();

        settings.put_profile(Profile {
            name: "Tiny".to_string(),
            max_memory: 64,
            min_memory: 512,
            window_width: 100,
            window_height: 100,
            ..Profile::from_settings(&settings)
        });

        let tiny = settings.profile("Tiny").expect("the profile was added");
        assert_eq!(tiny.max_memory, 512, "the maximum fell under the minimum");
        assert_eq!(tiny.min_memory, 512);
        assert_eq!(tiny.window_width, 320);
        assert_eq!(tiny.window_height, 240);
    }

    #[test]
    fn updating_the_default_profile_updates_the_settings() {
        let mut settings = Settings::default();
        let mut profile = settings.profile(DEFAULT_PROFILE).unwrap().clone();
        profile.max_memory = 8192;
        profile.game_version = "1.20.1".to_string();

        settings.put_profile(profile);

        assert_eq!(settings.max_memory, 8192, "the flat settings did not follow");
        assert_eq!(settings.game_version, "1.20.1");
        assert_eq!(settings.profile(DEFAULT_PROFILE).unwrap().max_memory, 8192);
    }

    #[test]
    fn the_default_profile_cannot_be_removed() {
        let mut settings = Settings::default();
        settings.put_profile(Profile {
            name: "Cinematic".to_string(),
            ..Profile::from_settings(&settings)
        });
        settings.active_profile = "Cinematic".to_string();

        settings.remove_profile("cinematic").expect("the profile was removed");
        assert_eq!(settings.profiles.len(), 1);
        assert_eq!(settings.active_profile, DEFAULT_PROFILE, "the active profile was removed");

        assert!(settings.remove_profile(DEFAULT_PROFILE).is_err());
        assert!(settings.remove_profile("nowhere").is_err());
        assert_eq!(settings.profiles.len(), 1);
    }

    #[test]
    fn a_settings_file_without_a_profile_gets_the_default_one() {
        let directory = std::env::temp_dir().join(format!("aether-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("settings.json");

        // The file of the previous version: the flat values, no profile at all.
        std::fs::write(
            &file,
            r#"{"maxMemory":8192,"gameVersion":"1.20.1","loaderVersion":"0.15.11"}"#,
        )
        .unwrap();

        let settings = Settings::load(&file);
        assert_eq!(settings.profiles.len(), 1);
        assert_eq!(settings.profiles[0].name, DEFAULT_PROFILE);
        assert_eq!(settings.profiles[0].max_memory, 8192);
        assert_eq!(settings.profiles[0].game_version, "1.20.1");

        // The upgrade is written down, a second read finds it without migrating again.
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(written["activeProfile"], DEFAULT_PROFILE);
        assert_eq!(written["profiles"][0]["name"], DEFAULT_PROFILE);

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn the_default_profile_follows_the_flat_settings() {
        let directory = std::env::temp_dir().join(format!("aether-default-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("settings.json");

        Settings {
            max_memory: 8192,
            ..Settings::default()
        }
        .save(&file)
        .unwrap();

        let loaded = Settings::load(&file);
        let default = loaded.profile(DEFAULT_PROFILE).expect("the default profile");
        assert_eq!(default.max_memory, 8192, "the two values disagree");

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn the_profile_of_an_account_wins_over_the_active_one() {
        let mut settings = Settings::default();
        settings.profiles.push(Profile {
            name: "Hardcore".to_string(),
            game_version: "1.20.1".to_string(),
            loader_version: "0.15.11".to_string(),
            max_memory: 8192,
            min_memory: 2048,
            window_width: 1920,
            window_height: 1080,
        });

        assert_eq!(settings.profile_for(None).name, DEFAULT_PROFILE);
        assert_eq!(settings.profile_for(Some("hardcore")).max_memory, 8192);
        assert_eq!(settings.profile_for(Some("nope")).name, DEFAULT_PROFILE);

        settings.active_profile = "Hardcore".to_string();
        assert_eq!(settings.profile_for(None).name, "Hardcore");
        // The account still decides when it names a profile itself.
        assert_eq!(settings.profile_for(Some("Default")).name, DEFAULT_PROFILE);
    }

    #[test]
    fn the_settings_survive_a_round_trip() {
        let directory = std::env::temp_dir().join(format!("aether-settings-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("settings.json");

        let settings = Settings {
            max_memory: 8192,
            accent_color: "#2ECC8F".to_string(),
            ..Settings::default()
        };
        settings.save(&file).unwrap();

        let loaded = Settings::load(&file);
        assert_eq!(loaded.max_memory, 8192);
        assert_eq!(loaded.accent_color, "#2ECC8F");
        assert_eq!(loaded.game_version, "1.21.11");

        std::fs::remove_dir_all(&directory).ok();
    }
}
