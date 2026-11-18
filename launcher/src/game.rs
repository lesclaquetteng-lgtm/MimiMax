//! The classpath and the arguments of the game.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;

use crate::minecraft::{
    apply_replacements, detect_os, evaluate_argument_template, resolve_libraries, FeatureContext,
    VersionJson,
};

/// The name the launcher identifies itself with.
pub const LAUNCHER_NAME: &str = "AetherLauncher";
/// The version of the launcher.
pub const LAUNCHER_VERSION: &str = "1.0.0";

/// The separator of the entries of a classpath.
pub const CLASS_PATH_SEPARATOR: &str = if cfg!(target_os = "windows") { ";" } else { ":" };

/// Everything that is needed to start the game.
pub struct LaunchOptions {
    /// The identity of the player.
    pub username: String,
    /// The identifier of the player.
    pub uuid: String,
    /// The token of the session, the placeholder `0` for an offline session.
    pub access_token: String,
    /// Whether the session is an offline one.
    pub offline: bool,
    /// The identifier of the player on Xbox live.
    pub xuid: String,
    /// The identifier of the client of a Microsoft session.
    pub client_id: String,
    /// The description of the version that is played.
    pub version: VersionJson,
    /// The jar of the game.
    pub client_jar: PathBuf,
    /// The jars that are added to the classpath, the mods and the Fabric API.
    pub extra_jars: Vec<PathBuf>,
    /// The folder with the native libraries.
    pub natives_dir: PathBuf,
    /// The folder of the game.
    pub game_dir: PathBuf,
    /// The folder of the assets.
    pub assets_dir: PathBuf,
    /// The index of the assets.
    pub asset_index: String,
    /// The folder of the libraries.
    pub library_dir: PathBuf,
    /// The runtime that starts the game.
    pub java_path: PathBuf,
    /// The maximum size of the heap in megabytes.
    pub max_memory: u32,
    /// The initial size of the heap in megabytes.
    pub min_memory: u32,
    /// The width of the window.
    pub window_width: u32,
    /// The height of the window.
    pub window_height: u32,
    /// Whether the G1 garbage collector is used.
    pub use_g1gc: bool,
    /// The preset of JVM flags: `balanced`, `performance`, `ultra` or anything else for the plain
    /// arguments of the version.
    pub jvm_preset: String,
    /// The server the game joins as soon as it starts, `--quickPlayMultiplayer`.
    pub quick_play_server: Option<String>,
}

/// Builds the classpath of the game.
///
/// The native libraries are part of the classpath as well, since Minecraft 1.19 LWJGL ships them as
/// regular libraries with a classifier and extracts them itself from the classpath.
pub fn build_classpath(options: &LaunchOptions) -> Vec<PathBuf> {
    let os = detect_os();
    let libraries = resolve_libraries(&options.version, &os);

    let mut entries: Vec<PathBuf> = vec![options.client_jar.clone()];
    entries.extend(
        libraries
            .iter()
            .map(|library| options.library_dir.join(&library.path)),
    );
    entries.extend(options.extra_jars.iter().cloned());

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        let entry = normalize(&entry);
        if seen.insert(entry.clone()) {
            result.push(entry);
        }
    }
    result
}

/// Removes the `.` and the `..` of a path without touching the disk.
fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

/// The flags of a JVM preset.
///
/// The values are the ones that are tried and measured by the Minecraft community (Aikar's flags
/// for G1): a large young generation, a slow promotion of the survivors and a heap that is touched
/// once at the start. A preset that is not known has no flag, so the plain arguments of the version
/// are used.
pub fn jvm_preset_flags(preset: &str) -> Vec<&'static str> {
    match preset.to_ascii_lowercase().as_str() {
        "balanced" => vec![
            "-XX:+UseG1GC",
            "-XX:MaxGCPauseMillis=50",
            "-XX:+UnlockExperimentalVMOptions",
            "-XX:G1NewSizePercent=20",
            "-XX:G1ReservePercent=20",
            "-XX:G1HeapRegionSize=32M",
        ],
        "performance" => vec![
            "-XX:+UseG1GC",
            "-XX:+ParallelRefProcEnabled",
            "-XX:MaxGCPauseMillis=200",
            "-XX:+UnlockExperimentalVMOptions",
            "-XX:+DisableExplicitGC",
            "-XX:+AlwaysPreTouch",
            "-XX:G1NewSizePercent=30",
            "-XX:G1MaxNewSizePercent=40",
            "-XX:G1HeapRegionSize=8M",
            "-XX:G1ReservePercent=20",
            "-XX:G1HeapWastePercent=5",
            "-XX:G1MixedGCCountTarget=4",
            "-XX:InitiatingHeapOccupancyPercent=15",
            "-XX:G1MixedGCLiveThresholdPercent=90",
            "-XX:G1RSetUpdatingPauseTimePercent=5",
            "-XX:SurvivorRatio=32",
            "-XX:+PerfDisableSharedMem",
            "-XX:MaxTenuringThreshold=1",
        ],
        "ultra" => {
            let mut flags = jvm_preset_flags("performance");
            flags.extend([
                "-XX:+UseStringDeduplication",
                "-XX:+OptimizeStringConcat",
                "-XX:+UseCompressedOops",
                "-XX:AutoBoxCacheMax=20000",
                "-Dfile.encoding=UTF-8",
            ]);
            flags
        }
        _ => Vec::new(),
    }
}

/// The G1 options that the JVM only accepts after `-XX:+UnlockExperimentalVMOptions`.
const EXPERIMENTAL_PREFIXES: [&str; 5] = [
    "-XX:G1NewSizePercent",
    "-XX:G1MaxNewSizePercent",
    "-XX:G1HeapWastePercent",
    "-XX:G1MixedGCLiveThresholdPercent",
    "-XX:G1RSetUpdatingPauseTimePercent",
];

const UNLOCK_EXPERIMENTAL: &str = "-XX:+UnlockExperimentalVMOptions";

/// Puts the unlock flag in front of the first experimental option.
///
/// The JVM refuses to start when an experimental option is missing its unlock flag, or when the
/// unlock flag comes after it ("The unlock option must precede 'G1NewSizePercent'"). Moving the flag
/// at the front keeps the presets, the user arguments and the version arguments safe in one place.
fn unlock_experimental_options(arguments: &mut Vec<String>) {
    arguments.retain(|argument| argument != UNLOCK_EXPERIMENTAL);

    let first = arguments.iter().position(|argument| {
        EXPERIMENTAL_PREFIXES
            .iter()
            .any(|prefix| argument.starts_with(prefix))
    });

    if let Some(index) = first {
        arguments.insert(index, UNLOCK_EXPERIMENTAL.to_string());
    }
}

/// Builds the JVM arguments.
pub fn build_jvm_arguments(options: &LaunchOptions, classpath: &[PathBuf]) -> Vec<String> {
    let os = detect_os();
    let features = FeatureContext::default();
    let template = evaluate_argument_template(&options.version.arguments.jvm, &os, &features);

    let classpath = classpath
        .iter()
        .map(|entry| entry.to_string_lossy().to_string())
        .collect::<Vec<String>>()
        .join(CLASS_PATH_SEPARATOR);

    let mut replacements: HashMap<String, String> = HashMap::new();
    replacements.insert(
        "natives_directory".to_string(),
        options.natives_dir.to_string_lossy().to_string(),
    );
    replacements.insert("launcher_name".to_string(), LAUNCHER_NAME.to_string());
    replacements.insert("launcher_version".to_string(), LAUNCHER_VERSION.to_string());
    replacements.insert("classpath".to_string(), classpath);
    replacements.insert(
        "library_directory".to_string(),
        options.library_dir.to_string_lossy().to_string(),
    );
    replacements.insert("version_name".to_string(), options.version.id.clone());

    let mut arguments = Vec::new();
    for entry in template {
        // The Fabric profile ships a malformed copy of this property, a clean one is added below.
        if entry.contains("FabricMcEmu") {
            continue;
        }
        arguments.push(apply_replacements(&entry, &replacements));
    }

    if options.max_memory > 0 {
        arguments.push(format!("-Xmx{}M", options.max_memory));
    }
    if options.min_memory > 0 {
        arguments.push(format!("-Xms{}M", options.min_memory));
    }

    let preset_flags = jvm_preset_flags(&options.jvm_preset);
    if preset_flags.is_empty() {
        // The plain setting of the previous versions is still honoured.
        if options.use_g1gc {
            arguments.push("-XX:+UseG1GC".to_string());
        }
    } else {
        arguments.extend(preset_flags.iter().map(|flag| flag.to_string()));
    }

    unlock_experimental_options(&mut arguments);

    // Fabric needs to know which Minecraft version it is running against.
    arguments.push(format!(
        "-Dfabric.gameVersion={}",
        options
            .version
            .inherits_from
            .clone()
            .unwrap_or_else(|| options.version.id.clone())
    ));
    arguments.push("-DFabricMcEmu=net.minecraft.client.main.Main".to_string());

    arguments
}

/// Builds the game arguments.
pub fn build_game_arguments(options: &LaunchOptions) -> Vec<String> {
    let os = detect_os();

    let mut features = HashMap::new();
    features.insert("is_demo_user".to_string(), false);
    features.insert("has_custom_resolution".to_string(), true);
    features.insert("has_quick_plays_support".to_string(), false);
    features.insert("is_quick_play_singleplayer".to_string(), false);
    features.insert("is_quick_play_multiplayer".to_string(), false);
    features.insert("is_quick_play_realms".to_string(), false);

    let context = FeatureContext { features };
    let template = evaluate_argument_template(&options.version.arguments.game, &os, &context);

    let mut replacements: HashMap<String, String> = HashMap::new();
    replacements.insert("auth_player_name".to_string(), options.username.clone());
    replacements.insert("version_name".to_string(), options.version.id.clone());
    replacements.insert(
        "version_type".to_string(),
        options.version.kind.clone().unwrap_or_else(|| "release".to_string()),
    );
    replacements.insert("game_directory".to_string(), options.game_dir.to_string_lossy().to_string());
    replacements.insert("assets_root".to_string(), options.assets_dir.to_string_lossy().to_string());
    replacements.insert("assets_index_name".to_string(), options.asset_index.clone());
    replacements.insert("auth_uuid".to_string(), options.uuid.clone());
    replacements.insert(
        "auth_access_token".to_string(),
        if options.offline {
            "0".to_string()
        } else {
            options.access_token.clone()
        },
    );
    replacements.insert("auth_xuid".to_string(), options.xuid.clone());
    replacements.insert("clientid".to_string(), options.client_id.clone());
    replacements.insert("resolution_width".to_string(), options.window_width.to_string());
    replacements.insert("resolution_height".to_string(), options.window_height.to_string());

    let mut arguments: Vec<String> = template
        .iter()
        .map(|entry| apply_replacements(entry, &replacements))
        .collect();

    // Quick play is an argument of the game itself, it is not part of the template.
    if let Some(server) = options.quick_play_server.as_deref() {
        if !server.trim().is_empty() {
            arguments.push("--quickPlayMultiplayer".to_string());
            arguments.push(server.trim().to_string());
        }
    }

    arguments
}

/// The complete command line of the game.
pub struct CommandLine {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub classpath: Vec<PathBuf>,
}

/// Builds the complete command line.
pub fn build_command(options: &LaunchOptions) -> CommandLine {
    let classpath = build_classpath(options);
    let mut arguments = build_jvm_arguments(options, &classpath);
    arguments.push(options.version.main_class.clone());
    arguments.extend(build_game_arguments(options));

    CommandLine {
        program: options.java_path.clone(),
        arguments,
        classpath,
    }
}

/// Starts the game.
///
/// The standard output and the standard error are read at the same time, every line is forwarded as
/// soon as it arrives. Reading them one after the other would reorder the log and can block the
/// game when one of the two pipes fills up.
pub fn launch(
    options: &LaunchOptions,
    mut on_output: impl FnMut(&str),
) -> Result<Child, std::io::Error> {
    let command = build_command(options);

    let mut process = Command::new(&command.program);
    process
        .args(&command.arguments)
        .current_dir(&options.game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        process.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = process.spawn()?;

    // Both streams are forwarded through a channel, so the caller keeps a single output callback.
    let (sender, receiver) = mpsc::channel::<String>();
    let mut readers = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        readers.push(forward_lines(stdout, sender.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(forward_lines(stderr, sender.clone()));
    }
    drop(sender);

    for line in receiver {
        on_output(&line);
    }

    for reader in readers {
        let _ = reader.join();
    }

    Ok(child)
}

/// Spawns a thread that forwards every line of a stream of the process.
fn forward_lines<R: std::io::Read + Send + 'static>(
    stream: R,
    sender: mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};

        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else {
                break;
            };
            if sender.send(line).is_err() {
                // The caller stopped reading, there is no point in forwarding more lines.
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::{Argument, Arguments, Downloads, VersionJson};

    fn options() -> LaunchOptions {
        LaunchOptions {
            username: "Notch".to_string(),
            uuid: "b50ad385-829d-3141-a216-7e7d7539ba7f".to_string(),
            access_token: "0".to_string(),
            offline: true,
            xuid: String::new(),
            client_id: String::new(),
            version: VersionJson {
                id: "1.21.11".to_string(),
                main_class: "net.minecraft.client.main.Main".to_string(),
                inherits_from: Some("1.21.11".to_string()),
                kind: Some("release".to_string()),
                downloads: Downloads::default(),
                arguments: Arguments {
                    game: vec![
                        Argument::Value("--username".to_string()),
                        Argument::Value("${auth_player_name}".to_string()),
                        Argument::Value("--accessToken".to_string()),
                        Argument::Value("${auth_access_token}".to_string()),
                    ],
                    jvm: vec![
                        Argument::Value("-cp".to_string()),
                        Argument::Value("${classpath}".to_string()),
                    ],
                },
                ..Default::default()
            },
            client_jar: PathBuf::from("C:/mc/versions/1.21.11/1.21.11.jar"),
            extra_jars: vec![PathBuf::from("C:/mc/mods/aether-client.jar")],
            natives_dir: PathBuf::from("C:/mc/versions/1.21.11/natives"),
            game_dir: PathBuf::from("C:/mc"),
            assets_dir: PathBuf::from("C:/mc/assets"),
            asset_index: "29".to_string(),
            library_dir: PathBuf::from("C:/mc/libraries"),
            java_path: PathBuf::from("C:/java/bin/java.exe"),
            max_memory: 4096,
            min_memory: 1024,
            window_width: 854,
            window_height: 480,
            use_g1gc: true,
            jvm_preset: "balanced".to_string(),
            quick_play_server: None,
        }
    }

    #[test]
    fn the_classpath_contains_the_game_and_the_mods() {
        let classpath = build_classpath(&options());
        assert!(classpath
            .iter()
            .any(|entry| entry.to_string_lossy().contains("1.21.11.jar")));
        assert!(classpath
            .iter()
            .any(|entry| entry.to_string_lossy().contains("aether-client.jar")));
    }

    #[test]
    fn the_classpath_is_passed_exactly_once() {
        let options = options();
        let classpath = build_classpath(&options);
        let arguments = build_jvm_arguments(&options, &classpath);
        let joined = classpath
            .iter()
            .map(|entry| entry.to_string_lossy().to_string())
            .collect::<Vec<String>>()
            .join(CLASS_PATH_SEPARATOR);

        let occurrences = arguments.iter().filter(|entry| *entry == &joined).count();
        assert_eq!(occurrences, 1, "the classpath was passed more than once");
    }

    #[test]
    fn the_arguments_carry_the_identity_of_the_player() {
        let arguments = build_game_arguments(&options());
        assert_eq!(arguments[arguments.iter().position(|e| e == "--username").unwrap() + 1], "Notch");
        assert_eq!(
            arguments[arguments.iter().position(|e| e == "--accessToken").unwrap() + 1],
            "0"
        );
    }

    #[test]
    fn the_memory_settings_are_applied() {
        let options = options();
        let classpath = build_classpath(&options);
        let arguments = build_jvm_arguments(&options, &classpath);
        assert!(arguments.contains(&"-Xmx4096M".to_string()));
        assert!(arguments.contains(&"-Xms1024M".to_string()));
        assert!(arguments.contains(&"-XX:+UseG1GC".to_string()));
        assert!(arguments.contains(&"-Dfabric.gameVersion=1.21.11".to_string()));
    }

    #[test]
    fn a_preset_replaces_the_gc_flag() {
        let options = options();
        let classpath = build_classpath(&options);
        let arguments = build_jvm_arguments(&options, &classpath);

        assert_eq!(
            arguments.iter().filter(|entry| *entry == "-XX:+UseG1GC").count(),
            1,
            "the collector is configured twice"
        );
        assert!(arguments.iter().any(|entry| entry.starts_with("-XX:G1NewSizePercent=")));
    }

    #[test]
    fn an_experimental_option_is_always_unlocked_first() {
        for preset in ["balanced", "performance", "ultra"] {
            let mut options = options();
            options.jvm_preset = preset.to_string();
            let classpath = build_classpath(&options);
            let arguments = build_jvm_arguments(&options, &classpath);

            let unlock = arguments
                .iter()
                .position(|entry| entry == UNLOCK_EXPERIMENTAL)
                .unwrap_or_else(|| panic!("{} never unlocks the experimental options", preset));
            let first_experimental = arguments
                .iter()
                .position(|entry| EXPERIMENTAL_PREFIXES.iter().any(|prefix| entry.starts_with(prefix)))
                .unwrap_or_else(|| panic!("{} uses no experimental option", preset));

            assert!(
                unlock < first_experimental,
                "{} sets an experimental option before unlocking it",
                preset
            );
            assert_eq!(
                arguments.iter().filter(|entry| *entry == UNLOCK_EXPERIMENTAL).count(),
                1,
                "{} unlocks the experimental options twice",
                preset
            );
        }
    }

    #[test]
    fn the_unlock_flag_is_moved_in_front_of_a_late_one() {
        let mut arguments = vec![
            "-XX:G1NewSizePercent=20".to_string(),
            UNLOCK_EXPERIMENTAL.to_string(),
        ];

        unlock_experimental_options(&mut arguments);

        assert_eq!(arguments[0], UNLOCK_EXPERIMENTAL);
        assert_eq!(arguments[1], "-XX:G1NewSizePercent=20");
    }

    #[test]
    fn an_unknown_preset_keeps_the_plain_gc_flag() {
        let mut options = options();
        options.jvm_preset = "custom".to_string();
        let classpath = build_classpath(&options);
        let arguments = build_jvm_arguments(&options, &classpath);

        assert!(arguments.contains(&"-XX:+UseG1GC".to_string()));
    }

    #[test]
    fn a_server_is_passed_as_quick_play() {
        let mut options = options();
        options.quick_play_server = Some("127.0.0.1:25565".to_string());

        let arguments = build_game_arguments(&options);
        let position = arguments
            .iter()
            .position(|entry| entry == "--quickPlayMultiplayer")
            .expect("the quick play argument is missing");
        assert_eq!(arguments[position + 1], "127.0.0.1:25565");
    }

    #[test]
    fn no_server_means_no_quick_play_argument() {
        let arguments = build_game_arguments(&options());
        assert!(!arguments.contains(&"--quickPlayMultiplayer".to_string()));
    }
}
