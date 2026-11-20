//! The optimisation pack and the tuning of the game options.
//!
//! Two levers are independent and complementary:
//!
//! * **the mods** are resolved from Modrinth for the exact version of the game, downloaded from the
//!   official CDN and verified with the SHA-1 that the API publishes;
//! * **`options.txt`** is rewritten with a preset that removes the expensive effects. The original
//!   file is copied once to `options.txt.aether-backup`, so that the player can go back.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::http::{self, Client};
use crate::paths::Paths;
use crate::settings::write_atomic;

/// The version of the game the pack is resolved for.
pub const GAME_VERSION: &str = "1.21.11";

/// The name of the backup of the options.
pub const OPTIONS_BACKUP: &str = "options.txt.aether-backup";

/// The name of the record of the mods that were installed by the launcher.
pub const MANIFEST: &str = "performance.json";

/// One mod of the optimisation pack.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimisationMod {
    /// The slug of the project on Modrinth.
    pub slug: String,
    /// The name the player reads.
    pub name: String,
    /// What the mod does, in one line.
    pub description: String,
    /// Whether the pack is incomplete without it.
    pub required: bool,
    /// `true` when the jar is in the mods folder.
    pub installed: bool,
    /// The version that is installed.
    pub version: Option<String>,
    /// The name of the jar on the disk.
    pub filename: Option<String>,
}

/// The description of a mod of the pack, independent of the disk.
struct Spec {
    slug: &'static str,
    name: &'static str,
    description: &'static str,
    required: bool,
}

/// The pack. Every mod was checked to have a Fabric build for 1.21.11.
const PACK: &[Spec] = &[
    Spec {
        slug: "sodium",
        name: "Sodium",
        description: "Rewrites the chunk renderer, the largest frame time win on Fabric.",
        required: true,
    },
    Spec {
        slug: "lithium",
        name: "Lithium",
        description: "Optimises the game logic and the ticking without changing the behaviour.",
        required: true,
    },
    Spec {
        slug: "ferrite-core",
        name: "FerriteCore",
        description: "Cuts the memory footprint of models, block states and textures.",
        required: true,
    },
    Spec {
        slug: "immediatelyfast",
        name: "ImmediatelyFast",
        description: "Batches the immediate-mode drawing of the interface and of the entities.",
        required: true,
    },
    Spec {
        slug: "entityculling",
        name: "EntityCulling",
        description: "Skips the entities that are hidden behind blocks, using asynchronous ray casts.",
        required: true,
    },
    Spec {
        slug: "moreculling",
        name: "MoreCulling",
        description: "Culls the faces of the leaves, of the glass and of the other thin blocks.",
        required: true,
    },
    // MoreCulling and Dynamic FPS declare Cloth Config as a hard dependency: without it the game
    // refuses to start, so the library is part of the pack.
    Spec {
        slug: "cloth-config",
        name: "Cloth Config API",
        description: "The configuration library that MoreCulling and Dynamic FPS are built on.",
        required: true,
    },
    Spec {
        slug: "krypton",
        name: "Krypton",
        description: "Moves the network stack off the render thread and lowers its latency.",
        required: true,
    },
    Spec {
        slug: "badoptimizations",
        name: "BadOptimizations",
        description: "Micro-optimisations of the lighting, of the sky and of the debug screens.",
        required: false,
    },
    Spec {
        slug: "dynamic-fps",
        name: "Dynamic FPS",
        description: "Lowers the frame rate when the window is in the background.",
        required: false,
    },
    Spec {
        slug: "clumps",
        name: "Clumps",
        description: "Groups the experience orbs, which removes hundreds of entities on a farm.",
        required: false,
    },
];

/// The answer of Modrinth for one version.
#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    version_number: String,
    #[serde(default)]
    files: Vec<ModrinthFile>,
}

/// One file of a version.
#[derive(Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: ModrinthHashes,
}

/// The checksums of a file.
#[derive(Debug, Default, Deserialize)]
struct ModrinthHashes {
    #[serde(default)]
    sha1: String,
}

/// The mods that the launcher installed, so that the list knows the version even after a restart.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct Manifest {
    mods: Vec<InstalledMod>,
}

/// One entry of the manifest.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledMod {
    slug: String,
    version: String,
    filename: String,
    #[serde(default)]
    sha1: String,
}

/// What the installation of the pack did.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<FailedMod>,
}

/// A mod that could not be installed.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedMod {
    pub slug: String,
    pub error: String,
}

/// The progress of one mod, streamed to the interface.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProgress {
    pub slug: String,
    pub name: String,
    pub state: String,
    pub detail: String,
    pub received: u64,
    pub total: u64,
}

/// The name of the jar on the disk, without a directory.
///
/// The name comes from the Modrinth API and is used to build a path in the mods folder. A name such
/// as `../../aether/settings.json` would write outside of it, and the file would then be loaded by
/// the game, so only a plain `.jar` name is accepted.
fn jar_name(filename: &str) -> Result<String, String> {
    let name = Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{:?} is not a file name", filename))?;

    if name != filename {
        return Err(format!("{:?} contains a directory", filename));
    }

    if !name.to_ascii_lowercase().ends_with(".jar") || name.starts_with('.') {
        return Err(format!("{:?} is not a mod jar", filename));
    }

    Ok(name.to_string())
}

/// The list of the pack with the state of every mod.
pub fn list(paths: &Paths) -> Vec<OptimisationMod> {
    let manifest = load_manifest(paths);

    PACK.iter()
        .map(|spec| {
            let installed = manifest
                .mods
                .iter()
                .find(|entry| {
                    jar_name(&entry.filename)
                        .map(|name| paths.mods.join(name).exists())
                        .unwrap_or(false)
                });

            OptimisationMod {
                slug: spec.slug.to_string(),
                name: spec.name.to_string(),
                description: spec.description.to_string(),
                required: spec.required,
                installed: installed.is_some(),
                version: installed.map(|entry| entry.version.clone()),
                filename: installed.map(|entry| entry.filename.clone()),
            }
        })
        .collect()
}

/// Installs the pack.
///
/// `only` limits the installation to some slugs; an empty slice installs the whole pack. A mod that
/// fails does not stop the others: the report names it and the rest of the pack is installed.
pub fn install(
    paths: &Paths,
    game_version: &str,
    only: &[String],
    mut on_progress: impl FnMut(&ModProgress),
) -> InstallReport {
    let client = Client::new();
    let mut manifest = load_manifest(paths);
    let mut report = InstallReport::default();

    for spec in PACK {
        if !only.is_empty() && !only.iter().any(|slug| slug == spec.slug) {
            continue;
        }

        let announce = |state: &str, detail: &str, received: u64, total: u64| ModProgress {
            slug: spec.slug.to_string(),
            name: spec.name.to_string(),
            state: state.to_string(),
            detail: detail.to_string(),
            received,
            total,
        };

        on_progress(&announce("resolving", "reading the version on Modrinth", 0, 0));

        let version = match resolve(&client, spec.slug, game_version) {
            Ok(version) => version,
            Err(error) => {
                let detail = error.to_string();
                on_progress(&announce("failed", &detail, 0, 0));
                report.failed.push(FailedMod {
                    slug: spec.slug.to_string(),
                    error: detail,
                });
                continue;
            }
        };

        let Some(file) = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first())
        else {
            let detail = "the version has no file".to_string();
            on_progress(&announce("failed", &detail, 0, 0));
            report.failed.push(FailedMod {
                slug: spec.slug.to_string(),
                error: detail,
            });
            continue;
        };

        let filename = match jar_name(&file.filename) {
            Ok(name) => name,
            Err(detail) => {
                on_progress(&announce("failed", &detail, 0, 0));
                report.failed.push(FailedMod {
                    slug: spec.slug.to_string(),
                    error: detail,
                });
                continue;
            }
        };

        // Modrinth publishes a SHA-1 for every file. Without it the jar cannot be verified, and an
        // unverified jar is code the game would load, so the mod is reported instead.
        let checksum = match file.hashes.sha1.trim() {
            "" => {
                let detail = "Modrinth did not publish a SHA-1 for this file".to_string();
                on_progress(&announce("failed", &detail, 0, 0));
                report.failed.push(FailedMod {
                    slug: spec.slug.to_string(),
                    error: detail,
                });
                continue;
            }
            sha1 => sha1.to_ascii_lowercase(),
        };

        let destination = paths.mods.join(&filename);
        if destination.exists() && http::sha1(&destination).as_deref() == Some(checksum.as_str()) {
            remember(
                &mut manifest,
                spec.slug,
                &version.version_number,
                &filename,
                &checksum,
            );
            on_progress(&announce("skipped", "already installed", 0, 0));
            report.skipped.push(spec.slug.to_string());
            continue;
        }

        on_progress(&announce("downloading", &filename, 0, 0));

        let result = client.download(&file.url, &destination, Some(&checksum), |received, total| {
            on_progress(&announce("downloading", &filename, received, total));
        });

        match result {
            Ok(download) => {
                remember(
                    &mut manifest,
                    spec.slug,
                    &version.version_number,
                    &filename,
                    &checksum,
                );
                on_progress(&announce(
                    "done",
                    &format!("{} bytes", download.bytes),
                    download.bytes,
                    download.bytes,
                ));
                report.installed.push(spec.slug.to_string());
            }
            Err(error) => {
                let detail = error.to_string();
                on_progress(&announce("failed", &detail, 0, 0));
                report.failed.push(FailedMod {
                    slug: spec.slug.to_string(),
                    error: detail,
                });
            }
        }
    }

    save_manifest(paths, &manifest);
    report
}

/// Asks Modrinth for the newest Fabric build of a mod for the game version.
fn resolve(client: &Client, slug: &str, game_version: &str) -> Result<ModrinthVersion, String> {
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version?loaders=%5B%22fabric%22%5D&game_versions=%5B%22{}%22%5D",
        slug, game_version
    );

    let versions: Vec<ModrinthVersion> = client.get_json(&url).map_err(|error| error.to_string())?;
    versions
        .into_iter()
        .next()
        .ok_or_else(|| format!("no Fabric build for {} on {}", slug, game_version))
}

/// Adds or replaces the entry of a mod in the manifest.
fn remember(manifest: &mut Manifest, slug: &str, version: &str, filename: &str, sha1: &str) {
    manifest.mods.retain(|entry| entry.slug != slug);
    manifest.mods.push(InstalledMod {
        slug: slug.to_string(),
        version: version.to_string(),
        filename: filename.to_string(),
        sha1: sha1.to_string(),
    });
}

fn manifest_path(paths: &Paths) -> PathBuf {
    paths.data.join(MANIFEST)
}

fn load_manifest(paths: &Paths) -> Manifest {
    std::fs::read_to_string(manifest_path(paths))
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_manifest(paths: &Paths, manifest: &Manifest) {
    if let Ok(content) = serde_json::to_string_pretty(manifest) {
        let _ = write_atomic(&manifest_path(paths), &content);
    }
}

/// One option of `options.txt` that a preset changes.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionChange {
    pub key: String,
    pub before: Option<String>,
    pub after: String,
}

/// What a tuning did.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsReport {
    pub path: String,
    pub backup: String,
    pub preset: String,
    pub changes: Vec<OptionChange>,
}

/// The values of the `max` preset: the lowest cost the game can render at.
///
/// Every value is inside the range the game accepts; a value out of range is rejected at start up
/// (`Error parsing option value …`) and replaced by a default, which silently undoes the preset.
///
/// `graphicsPreset` must stay `custom`. Each named preset re-applies its whole bundle at start up
/// and overwrites the individual values written below: with `fast`, the game turned the ambient
/// occlusion, the clouds, the particles and the entity distance back to its own choices, so half
/// of the preset never reached the renderer. `custom` tells the game to keep the values as they
/// are, which is the whole point of this file.
const MAX_PRESET: &[(&str, &str)] = &[
    ("renderDistance", "8"),
    ("simulationDistance", "6"),
    ("graphicsPreset", "\"custom\""),
    ("renderClouds", "\"false\""),
    ("cloudRange", "32"),
    // 0 is all, 1 is decreased, 2 is minimal.
    ("particles", "2"),
    ("entityShadows", "false"),
    ("entityDistanceScaling", "0.5"),
    ("mipmapLevels", "2"),
    ("biomeBlendRadius", "1"),
    ("maxAnisotropyBit", "1"),
    ("syncChunkWrites", "false"),
    ("chunkSectionFadeInTime", "0.0"),
    ("prioritizeChunkUpdates", "1"),
    ("ao", "true"),
    ("enableVsync", "false"),
    ("maxFps", "260"),
    ("inactivityFpsLimit", "\"afk\""),
    ("weatherRadius", "5"),
    ("reducedDebugInfo", "false"),
];

/// The values of the `balanced` preset: a large gain that keeps the game looking normal.
const BALANCED_PRESET: &[(&str, &str)] = &[
    ("renderDistance", "12"),
    ("simulationDistance", "8"),
    ("graphicsPreset", "\"custom\""),
    ("renderClouds", "\"true\""),
    ("cloudRange", "64"),
    ("particles", "1"),
    ("entityShadows", "false"),
    ("entityDistanceScaling", "0.85"),
    ("mipmapLevels", "4"),
    ("biomeBlendRadius", "1"),
    ("maxAnisotropyBit", "1"),
    ("syncChunkWrites", "false"),
    ("chunkSectionFadeInTime", "0.0"),
    ("prioritizeChunkUpdates", "1"),
    ("ao", "true"),
    ("enableVsync", "false"),
    ("maxFps", "260"),
    ("inactivityFpsLimit", "\"afk\""),
    ("weatherRadius", "8"),
];

/// The settings of a preset.
pub fn preset_values(preset: &str) -> Result<&'static [(&'static str, &'static str)], String> {
    match preset {
        "max" => Ok(MAX_PRESET),
        "balanced" => Ok(BALANCED_PRESET),
        other => Err(format!(
            "unknown preset: {}, the presets are balanced, max and restore",
            other
        )),
    }
}

/// Writes a preset into `options.txt`, keeping every other line untouched.
///
/// The first call copies the original file to `options.txt.aether-backup`. `restore` copies that
/// backup back and never overwrites it, so the original options are always one command away.
pub fn tune_options(paths: &Paths, preset: &str) -> Result<OptionsReport, String> {
    let options = &paths.options_file;
    let backup = options.with_file_name(OPTIONS_BACKUP);

    if preset == "restore" {
        return restore_options(options, &backup);
    }

    let values = preset_values(preset)?;

    if !options.exists() {
        std::fs::create_dir_all(&paths.game).map_err(|error| error.to_string())?;
        write_atomic(options, "").map_err(|error| error.to_string())?;
    }

    if !backup.exists() {
        std::fs::copy(options, &backup).map_err(|error| error.to_string())?;
    }

    let content = std::fs::read_to_string(options).map_err(|error| error.to_string())?;
    let mut changes = Vec::new();
    let mut lines = parse_lines(&content);

    for (key, value) in values {
        let before = set_key(&mut lines, key, value);
        changes.push(OptionChange {
            key: key.to_string(),
            before,
            after: value.to_string(),
        });
    }

    write_atomic(options, &join_lines(&lines)).map_err(|error| error.to_string())?;

    Ok(OptionsReport {
        path: options.to_string_lossy().to_string(),
        backup: backup.to_string_lossy().to_string(),
        preset: preset.to_string(),
        changes,
    })
}

/// Copies the backup over the options.
fn restore_options(options: &Path, backup: &Path) -> Result<OptionsReport, String> {
    if !backup.exists() {
        return Err("no backup of the options was made yet".to_string());
    }

    let content = std::fs::read_to_string(backup).map_err(|error| error.to_string())?;
    write_atomic(options, &content).map_err(|error| error.to_string())?;

    Ok(OptionsReport {
        path: options.to_string_lossy().to_string(),
        backup: backup.to_string_lossy().to_string(),
        preset: "restore".to_string(),
        changes: Vec::new(),
    })
}

/// Splits `options.txt` into lines, a trailing newline is remembered as an empty last line.
fn parse_lines(content: &str) -> Vec<String> {
    content.lines().map(|line| line.to_string()).collect()
}

fn join_lines(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Sets one key, returns the value it had.
fn set_key(lines: &mut Vec<String>, key: &str, value: &str) -> Option<String> {
    let prefix = format!("{}:", key);

    for line in lines.iter_mut() {
        if let Some(rest) = line.strip_prefix(&prefix) {
            let before = rest.to_string();
            *line = format!("{}:{}", key, value);
            return Some(before);
        }
    }

    lines.push(format!("{}:{}", key, value));
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_replaced_in_place_and_appended_when_missing() {
        let mut lines = parse_lines("version:4671\nrenderDistance:16\nmaxFps:120\n");

        assert_eq!(
            set_key(&mut lines, "renderDistance", "8"),
            Some("16".to_string())
        );
        assert_eq!(lines[1], "renderDistance:8");

        assert_eq!(set_key(&mut lines, "particles", "0"), None);
        assert_eq!(lines.last().unwrap(), "particles:0");
        assert_eq!(lines[0], "version:4671", "the order of the file changed");
    }

    #[test]
    fn the_document_keeps_a_trailing_newline() {
        let text = join_lines(&parse_lines("a:1\nb:2\n"));
        assert_eq!(text, "a:1\nb:2\n");
    }

    #[test]
    fn the_presets_are_known() {
        assert!(preset_values("max").is_ok());
        assert!(preset_values("balanced").is_ok());
        assert!(preset_values("cinematic").is_err());
    }

    #[test]
    fn the_pack_lists_unique_slugs_and_carries_its_libraries() {
        let mut slugs: Vec<&str> = PACK.iter().map(|spec| spec.slug).collect();
        slugs.sort_unstable();
        let mut deduplicated = slugs.clone();
        deduplicated.dedup();
        assert_eq!(slugs, deduplicated, "a mod is listed twice");

        // MoreCulling and Dynamic FPS refuse to load without Cloth Config.
        assert!(
            PACK.iter().any(|spec| spec.slug == "cloth-config"),
            "the pack must ship the libraries its mods depend on"
        );
    }

    #[test]
    fn the_presets_of_the_options_stay_inside_the_ranges_of_the_game() {
        for preset in [MAX_PRESET, BALANCED_PRESET] {
            let raw = |key: &str| {
                preset
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, value)| value.trim_matches('"'))
            };
            let value = |key: &str| raw(key).and_then(|value| value.parse::<i32>().ok());

            // The game reports "Value 0 outside of range [1:7]" and resets the option otherwise.
            assert!(value("biomeBlendRadius").unwrap() >= 1);
            // "Value 0 outside of range [1:4]" for the anisotropic filtering.
            assert!(value("maxAnisotropyBit").unwrap() >= 1);
            // The particle option is an enum: 0 all, 1 decreased, 2 minimal.
            assert!((0..=2).contains(&value("particles").unwrap()));
            // A named graphics preset rewrites the options below it at start up.
            assert_eq!(raw("graphicsPreset"), Some("custom"));
        }
    }

    #[test]
    fn a_mod_name_cannot_leave_the_mods_folder() {
        assert_eq!(jar_name("sodium-0.6.1.jar").unwrap(), "sodium-0.6.1.jar");
        assert_eq!(jar_name("Cloth-Config.JAR").unwrap(), "Cloth-Config.JAR");

        assert!(jar_name("../../aether/settings.json").is_err());
        assert!(jar_name("..\\..\\aether\\settings.json").is_err());
        assert!(jar_name("sub/dir/mod.jar").is_err());
        assert!(jar_name("./mod.jar").is_err());
        assert!(jar_name("mod.zip").is_err());
        assert!(jar_name(".hidden.jar").is_err());
        assert!(jar_name("").is_err());
        assert!(jar_name("..").is_err());
    }

    #[test]
    fn a_tuning_writes_the_options_and_keeps_a_backup() {
        let directory = std::env::temp_dir().join(format!("aether-options-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join(".minecraft")).unwrap();

        let paths = Paths {
            home: directory.clone(),
            data: directory.join("aether"),
            game: directory.join(".minecraft"),
            assets: directory.join(".minecraft/assets"),
            asset_indexes: directory.join(".minecraft/assets/indexes"),
            asset_objects: directory.join(".minecraft/assets/objects"),
            libraries: directory.join(".minecraft/libraries"),
            versions: directory.join(".minecraft/versions"),
            mods: directory.join(".minecraft/mods"),
            settings_file: directory.join("aether/settings.json"),
            accounts_file: directory.join("aether/accounts.json"),
            servers_file: directory.join("aether/servers.json"),
            server_root: directory.join("servers"),
            options_file: directory.join(".minecraft/options.txt"),
            logs: directory.join("aether/logs"),
        };

        std::fs::write(&paths.options_file, "version:4671\nrenderDistance:16\n").unwrap();

        let report = tune_options(&paths, "max").expect("the options were tuned");
        assert_eq!(report.preset, "max");
        assert!(report.changes.iter().any(|change| change.key == "renderDistance"));

        let content = std::fs::read_to_string(&paths.options_file).unwrap();
        assert!(content.contains("renderDistance:8"));
        assert!(content.contains("graphicsPreset:\"custom\""));
        assert!(content.contains("version:4671"), "an unknown line was lost");

        let backup = std::fs::read_to_string(paths.options_file.with_file_name(OPTIONS_BACKUP)).unwrap();
        assert_eq!(backup, "version:4671\nrenderDistance:16\n", "the backup is not the original");

        tune_options(&paths, "restore").expect("the options were restored");
        let restored = std::fs::read_to_string(&paths.options_file).unwrap();
        assert_eq!(restored, backup);

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn restoring_without_a_backup_is_an_error() {
        let directory = std::env::temp_dir().join(format!("aether-restore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let options = directory.join("options.txt");
        let backup = directory.join(OPTIONS_BACKUP);
        assert!(restore_options(&options, &backup).is_err());

        std::fs::remove_dir_all(&directory).ok();
    }
}
