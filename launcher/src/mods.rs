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



/* --------------------------------------------------------- Modrinth catalog */

use serde::{Deserialize, Serialize};

use crate::http::{self, Client};

const MODRINTH_API: &str = "https://api.modrinth.com/v2";
const CATALOG_LIMIT: usize = 16;
const MAX_QUERY_LENGTH: usize = 96;
const MAX_DEPENDENCY_DEPTH: usize = 8;

/// One Fabric mod in the Modrinth search results.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct CatalogProject {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub latest_version: Option<String>,
    #[serde(default)]
    pub versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    hits: Vec<CatalogProject>,
}

#[derive(Clone, Debug, Deserialize)]
struct CatalogVersion {
    #[serde(default)]
    id: String,
    #[serde(default)]
    project_id: String,
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    files: Vec<CatalogFile>,
    #[serde(default)]
    dependencies: Vec<CatalogDependency>,
}

#[derive(Clone, Debug, Deserialize)]
struct CatalogFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: CatalogHashes,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CatalogHashes {
    #[serde(default)]
    sha1: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CatalogDependency {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    dependency_type: Option<String>,
}

/// Result of installing one project or one of its required dependencies.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInstallEntry {
    pub project_id: String,
    pub version: String,
    pub filename: String,
    pub already_installed: bool,
}

/// Searches the public Modrinth catalog for Fabric mods compatible with a Minecraft version.
pub fn search_catalog(query: &str, game_version: &str) -> Result<Vec<CatalogProject>, String> {
    validate_game_version(game_version)?;
    let query = query.trim();
    if query.chars().count() < 2 {
        return Err("Enter at least two characters to search the mod catalog.".to_string());
    }
    if query.chars().count() > MAX_QUERY_LENGTH {
        return Err(format!("Search text must be at most {MAX_QUERY_LENGTH} characters."));
    }

    let facets = format!(
        r#"[["project_type:mod"],["categories:fabric"],["versions:{game_version}"]]"#
    );
    let url = format!(
        "{MODRINTH_API}/search?query={}&facets={}&index=downloads&limit={CATALOG_LIMIT}",
        encode_query_component(query),
        encode_query_component(&facets),
    );

    let response: SearchResponse = Client::new()
        .get_json(&url)
        .map_err(|error| format!("Modrinth search failed: {error}"))?;

    Ok(response
        .hits
        .into_iter()
        .filter(|project| {
            valid_project_id(&project.project_id)
                && valid_project_component(&project.slug)
                && !project.title.trim().is_empty()
        })
        .map(|mut project| {
            // The renderer is allowed to load images only from this same official CDN host.
            project.icon_url = project.icon_url.filter(|url| is_official_cdn_url(url));
            project
        })
        .collect())
}

/// Installs a Modrinth project and its required Fabric dependencies.
///
/// Every downloaded file must be a JAR, come from the official Modrinth CDN and carry a SHA-1
/// published by the API. Existing files are never overwritten: a conflicting filename receives a
/// checksum suffix, and an identical JAR is reported as already installed.
pub fn install_catalog_project(
    mods_dir: &Path,
    project_id: &str,
    game_version: &str,
) -> Result<Vec<CatalogInstallEntry>, String> {
    validate_project_id(project_id)?;
    validate_game_version(game_version)?;
    fs::create_dir_all(mods_dir).map_err(|error| {
        format!("The mods folder could not be created: {error}")
    })?;

    let client = Client::new();
    let version = latest_fabric_version(&client, project_id, game_version)?;
    let mut visited = std::collections::HashSet::new();
    let mut installed = Vec::new();

    install_version_tree(
        &client,
        mods_dir,
        version,
        project_id,
        game_version,
        0,
        &mut visited,
        &mut installed,
    )?;

    Ok(installed)
}

fn latest_fabric_version(
    client: &Client,
    project_id: &str,
    game_version: &str,
) -> Result<CatalogVersion, String> {
    validate_project_id(project_id)?;
    let loaders = encode_query_component(r#"["fabric"]"#);
    let versions = encode_query_component(&format!(r#"["{game_version}"]"#));
    let url = format!(
        "{MODRINTH_API}/project/{project_id}/version?loaders={loaders}&game_versions={versions}"
    );

    let versions: Vec<CatalogVersion> = client
        .get_json(&url)
        .map_err(|error| format!("Could not read compatible versions from Modrinth: {error}"))?;
    let version = versions
        .into_iter()
        .next()
        .ok_or_else(|| format!("No Fabric version of {project_id} supports Minecraft {game_version}."))?;

    validate_catalog_version(&version, game_version)?;
    Ok(version)
}

fn exact_fabric_version(
    client: &Client,
    version_id: &str,
    game_version: &str,
) -> Result<CatalogVersion, String> {
    validate_project_id(version_id)?;
    let url = format!("{MODRINTH_API}/version/{version_id}");
    let version: CatalogVersion = client
        .get_json(&url)
        .map_err(|error| format!("Could not read a required Modrinth dependency: {error}"))?;
    validate_catalog_version(&version, game_version)?;
    Ok(version)
}

fn validate_catalog_version(version: &CatalogVersion, game_version: &str) -> Result<(), String> {
    if !version.loaders.iter().any(|loader| loader == "fabric") {
        return Err("Modrinth returned a version that is not built for Fabric.".to_string());
    }
    if !version.game_versions.iter().any(|version| version == game_version) {
        return Err(format!(
            "Modrinth returned a version that does not support Minecraft {game_version}."
        ));
    }
    if version.version_number.trim().is_empty() {
        return Err("Modrinth returned a version without a version number.".to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn install_version_tree(
    client: &Client,
    mods_dir: &Path,
    version: CatalogVersion,
    fallback_project_id: &str,
    game_version: &str,
    depth: usize,
    visited: &mut std::collections::HashSet<String>,
    installed: &mut Vec<CatalogInstallEntry>,
) -> Result<(), String> {
    if depth > MAX_DEPENDENCY_DEPTH {
        return Err("The required mod dependency chain is too deep to install safely.".to_string());
    }

    let project_id = if version.project_id.is_empty() {
        fallback_project_id.to_string()
    } else {
        version.project_id.clone()
    };
    validate_project_id(&project_id)?;

    let identity = if version.id.is_empty() {
        project_id.clone()
    } else {
        version.id.clone()
    };
    if !visited.insert(identity) {
        return Ok(());
    }

    // Install required dependencies before the project that relies on them.
    for dependency in &version.dependencies {
        if dependency.dependency_type.as_deref() != Some("required") {
            continue;
        }

        let dependency_version = if let Some(version_id) = dependency.version_id.as_deref() {
            exact_fabric_version(client, version_id, game_version)?
        } else if let Some(dependency_project) = dependency.project_id.as_deref() {
            latest_fabric_version(client, dependency_project, game_version)?
        } else {
            return Err(format!(
                "A required dependency of {project_id} did not provide a Modrinth project or version ID."
            ));
        };

        let fallback = dependency
            .project_id
            .as_deref()
            .or_else(|| {
                if dependency_version.project_id.is_empty() {
                    None
                } else {
                    Some(dependency_version.project_id.as_str())
                }
            })
            .unwrap_or(&project_id)
            .to_string();

        install_version_tree(
            client,
            mods_dir,
            dependency_version,
            &fallback,
            game_version,
            depth + 1,
            visited,
            installed,
        )?;
    }

    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| format!("Modrinth did not provide a JAR file for {project_id}."))?;

    if !is_official_cdn_url(&file.url) {
        return Err(format!("Modrinth returned an untrusted download host for {project_id}."));
    }
    if !file.filename.to_ascii_lowercase().ends_with(".jar") {
        return Err(format!("Modrinth returned a non-JAR file for {project_id}."));
    }

    let checksum = file.hashes.sha1.trim().to_ascii_lowercase();
    if checksum.len() != 40 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("Modrinth did not provide a valid SHA-1 for {project_id}."));
    }

    let slug = if valid_project_component(&project_id) {
        project_id.as_str()
    } else {
        return Err("Modrinth returned an invalid project identifier.".to_string());
    };
    let version_component = safe_filename_component(&version.version_number)?;
    let base_filename = format!("{slug}-{version_component}.jar");

    // Reuse an identical installed file regardless of its current name or enabled state.
    if let Some(existing) = find_installed_hash(mods_dir, &checksum) {
        installed.push(CatalogInstallEntry {
            project_id,
            version: version.version_number,
            filename: existing,
            already_installed: true,
        });
        return Ok(());
    }

    let destination = mods_dir.join(&base_filename);
    let destination = if destination.exists() {
        let checksum_filename = format!(
            "{slug}-{version_component}-{}.jar",
            &checksum[..8]
        );
        mods_dir.join(checksum_filename)
    } else {
        destination
    };

    if destination.exists() {
        return Err(format!(
            "The file {} already exists with different contents. It was left untouched.",
            destination.file_name().and_then(|value| value.to_str()).unwrap_or("unknown")
        ));
    }

    http::download(&file.url, &destination, Some(&checksum), |_, _| {})
        .map_err(|error| format!("Could not install {project_id}: {error}"))?;

    installed.push(CatalogInstallEntry {
        project_id,
        version: version.version_number,
        filename: destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        already_installed: false,
    });

    Ok(())
}

fn find_installed_hash(mods_dir: &Path, expected_sha1: &str) -> Option<String> {
    let entries = fs::read_dir(mods_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name()?.to_str()?;
        if !is_mod(name) || !path.is_file() {
            continue;
        }
        if http::sha1(&path).as_deref() == Some(expected_sha1) {
            return Some(name.to_string());
        }
    }
    None
}

fn is_official_cdn_url(url: &str) -> bool {
    let Some(path) = url.strip_prefix("https://cdn.modrinth.com/") else {
        return false;
    };
    !path.is_empty()
        && !url.contains('@')
        && !url.contains('\\')
        && !url.chars().any(char::is_whitespace)
}

fn validate_project_id(value: &str) -> Result<(), String> {
    if valid_project_id(value) {
        Ok(())
    } else {
        Err("The Modrinth project ID is invalid.".to_string())
    }
}

fn valid_project_id(value: &str) -> bool {
    (4..=64).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn valid_project_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
        })
}

fn safe_filename_component(value: &str) -> Result<String, String> {
    if value.is_empty()
        || value.len() > 80
        || value.starts_with('.')
        || value.contains("..")
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+')
        })
    {
        return Err("Modrinth returned an unsafe version filename.".to_string());
    }
    Ok(value.to_string())
}

fn validate_game_version(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 24
        || !value.contains('.')
        || !value.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.')
        || value.starts_with('.')
        || value.ends_with('.')
    {
        return Err("The active Minecraft version is invalid.".to_string());
    }
    Ok(())
}

fn encode_query_component(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            result.push(byte as char);
        } else {
            result.push('%');
            result.push_str(&format!("{byte:02X}"));
        }
    }
    result
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

    #[test]
    fn modrinth_search_queries_are_encoded_as_single_query_components() {
        assert_eq!(encode_query_component("Sodium & Iris"), "Sodium%20%26%20Iris");
        assert_eq!(encode_query_component("1.21.11"), "1.21.11");
        assert_eq!(encode_query_component(r#"[["project_type:mod"]]"#), "%5B%5B%22project_type%3Amod%22%5D%5D");
    }

    #[test]
    fn only_the_official_modrinth_cdn_is_accepted_for_mod_downloads() {
        assert!(is_official_cdn_url("https://cdn.modrinth.com/data/abc/file.jar"));
        assert!(!is_official_cdn_url("http://cdn.modrinth.com/data/abc/file.jar"));
        assert!(!is_official_cdn_url("https://cdn.modrinth.com.evil.test/file.jar"));
        assert!(!is_official_cdn_url("https://evil.test/cdn.modrinth.com/file.jar"));
        assert!(!is_official_cdn_url("https://user@cdn.modrinth.com/file.jar"));
    }

    #[test]
    fn modrinth_project_ids_versions_and_filename_components_are_validated() {
        assert!(valid_project_id("A1b2C3d4"));
        assert!(valid_project_id("sodium_1234"));
        assert!(!valid_project_id("../sodium"));
        assert!(!valid_project_id("x"));
        assert!(validate_game_version("1.21.11").is_ok());
        assert!(validate_game_version("26.1").is_ok());
        assert!(validate_game_version("../1.21").is_err());
        assert!(validate_game_version("1.21.11?facets=*").is_err());
        assert!(safe_filename_component("1.21.11-0.6.0+fabric").is_ok());
        assert!(safe_filename_component("../escape").is_err());
        assert!(safe_filename_component(".hidden").is_err());
    }

    #[test]
    fn modrinth_search_hits_deserialize_snake_case_and_serialize_camel_case() {
        let hit: CatalogProject = serde_json::from_str(r#"{
            "project_id": "A1b2C3d4",
            "slug": "sodium",
            "title": "Sodium",
            "description": "A rendering optimization mod.",
            "author": "JellySquid",
            "downloads": 1200000,
            "icon_url": "https://cdn.modrinth.com/data/A1b2C3d4/icon.png",
            "latest_version": "mc1.21.11-0.6.0",
            "versions": ["mc1.21.11-0.6.0"]
        }"#).unwrap();

        assert_eq!(hit.project_id, "A1b2C3d4");
        let rendered = serde_json::to_value(hit).unwrap();
        assert_eq!(rendered["projectId"], "A1b2C3d4");
        assert_eq!(rendered["iconUrl"], "https://cdn.modrinth.com/data/A1b2C3d4/icon.png");
        assert_eq!(rendered["latestVersion"], "mc1.21.11-0.6.0");
    }

    #[test]
    fn required_dependencies_are_parsed_without_treating_optional_mods_as_required() {
        let version: CatalogVersion = serde_json::from_str(r#"{
            "id": "version01",
            "project_id": "project01",
            "version_number": "1.0.0",
            "game_versions": ["1.21.11"],
            "loaders": ["fabric"],
            "files": [{
                "url": "https://cdn.modrinth.com/data/project01/file.jar",
                "filename": "project.jar",
                "primary": true,
                "hashes": {"sha1": "0123456789012345678901234567890123456789"}
            }],
            "dependencies": [
                {"project_id": "dependency1", "dependency_type": "required"},
                {"project_id": "dependency2", "dependency_type": "optional"}
            ]
        }"#).unwrap();

        assert_eq!(version.dependencies.len(), 2);
        assert_eq!(version.dependencies[0].dependency_type.as_deref(), Some("required"));
        assert_eq!(version.dependencies[1].dependency_type.as_deref(), Some("optional"));
        assert_eq!(version.files[0].filename, "project.jar");
    }

}
