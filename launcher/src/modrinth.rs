//! Modrinth discovery and installation for Fabric client mods.
//!
//! Search results are filtered for the active Minecraft version and Fabric. Downloads are accepted
//! only from Modrinth's HTTPS CDN, verified with the SHA-1 published by the API, and installed
//! together with every required dependency. A small manifest lets the launcher update its own
//! managed jars without touching files the player placed in the mods folder.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::http::{self, Client};
use crate::paths::Paths;
use crate::settings::write_atomic;

const API: &str = "https://api.modrinth.com/v2";
const CDN_HOST: &str = "cdn.modrinth.com";
const MANIFEST: &str = "modrinth-mods.json";
const MAX_QUERY_LENGTH: usize = 120;
const MAX_RESULTS: usize = 40;
const MAX_DEPENDENCY_DEPTH: usize = 12;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthProject {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub latest_version: String,
    pub client_side: String,
    pub server_side: String,
    pub installed: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<FailedMod>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedMod {
    pub project_id: String,
    pub name: String,
    pub error: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProgress {
    pub project_id: String,
    pub name: String,
    pub state: String,
    pub detail: String,
    pub received: u64,
    pub total: u64,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    hits: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    project_id: String,
    #[serde(default)]
    slug: Option<String>,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    follows: u64,
    #[serde(default)]
    latest_version: String,
    #[serde(default)]
    client_side: String,
    #[serde(default)]
    server_side: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct Manifest {
    mods: Vec<InstalledProject>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledProject {
    project_id: String,
    slug: String,
    title: String,
    version_id: String,
    version_number: String,
    filename: String,
    sha1: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ProjectVersion {
    id: String,
    #[serde(default)]
    project_id: String,
    #[serde(default)]
    name: String,
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
    #[serde(default)]
    files: Vec<VersionFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct Dependency {
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Clone, Debug, Deserialize)]
struct VersionFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: HashMap<String, String>,
}

/// Searches only Fabric mods that explicitly support the selected Minecraft version.
pub fn search(
    paths: &Paths,
    query: &str,
    game_version: &str,
    limit: usize,
) -> Result<Vec<ModrinthProject>, String> {
    if query.chars().count() > MAX_QUERY_LENGTH {
        return Err(format!("Search terms must be {} characters or fewer.", MAX_QUERY_LENGTH));
    }
    if game_version.trim().is_empty() {
        return Err("Choose a Minecraft version before browsing mods.".to_string());
    }

    let limit = limit.clamp(1, MAX_RESULTS);
    let facets = serde_json::json!([
        ["project_type:mod"],
        [format!("versions:{}", game_version)],
        ["categories:fabric"]
    ])
    .to_string();
    let url = format!(
        "{API}/search?query={}&facets={}&index=relevance&limit={limit}",
        encode_component(query.trim()),
        encode_component(&facets)
    );

    let response: SearchResponse = Client::new()
        .get_json(&url)
        .map_err(|error| format!("Modrinth search failed: {error}"))?;
    let manifest = load_manifest(paths);

    Ok(response
        .hits
        .into_iter()
        .filter(|hit| valid_project_id(&hit.project_id))
        .map(|hit| {
            let installed = manifest.mods.iter().any(|entry| {
                entry.project_id == hit.project_id && managed_file_is_valid(paths, entry)
            });
            ModrinthProject {
                project_id: hit.project_id,
                slug: hit.slug.unwrap_or_default(),
                title: hit.title,
                description: hit.description,
                author: hit.author,
                icon_url: hit.icon_url.filter(|url| trusted_icon_url(url)),
                downloads: hit.downloads,
                follows: hit.follows,
                latest_version: hit.latest_version,
                client_side: hit.client_side,
                server_side: hit.server_side,
                installed,
            }
        })
        .collect())
}

/// Installs a Fabric project and its required Modrinth dependencies for one game version.
pub fn install(
    paths: &Paths,
    project_id: &str,
    game_version: &str,
    mut on_progress: impl FnMut(&ModProgress),
) -> InstallReport {
    let mut report = InstallReport::default();

    if !valid_project_id(project_id) {
        report.failed.push(FailedMod {
            project_id: project_id.to_string(),
            name: project_id.to_string(),
            error: "The Modrinth project identifier is invalid.".to_string(),
        });
        return report;
    }

    if game_version.trim().is_empty() {
        report.failed.push(FailedMod {
            project_id: project_id.to_string(),
            name: project_id.to_string(),
            error: "Choose a Minecraft version before installing a mod.".to_string(),
        });
        return report;
    }

    if let Err(error) = paths.ensure() {
        report.failed.push(FailedMod {
            project_id: project_id.to_string(),
            name: project_id.to_string(),
            error: format!("The mods folder could not be prepared: {error}"),
        });
        return report;
    }

    let client = Client::new();
    let mut manifest = load_manifest(paths);
    let mut visiting = HashSet::new();
    let mut completed = HashSet::new();

    if let Err(error) = install_project(
        &client,
        paths,
        &mut manifest,
        project_id,
        game_version,
        0,
        &mut visiting,
        &mut completed,
        &mut report,
        &mut on_progress,
    ) {
        report.failed.push(FailedMod {
            project_id: project_id.to_string(),
            name: project_id.to_string(),
            error,
        });
    }

    report
}

#[allow(clippy::too_many_arguments)]
fn install_project(
    client: &Client,
    paths: &Paths,
    manifest: &mut Manifest,
    project_id: &str,
    game_version: &str,
    depth: usize,
    visiting: &mut HashSet<String>,
    completed: &mut HashSet<String>,
    report: &mut InstallReport,
    on_progress: &mut impl FnMut(&ModProgress),
) -> Result<(), String> {
    if depth > MAX_DEPENDENCY_DEPTH {
        return Err("The mod dependency chain is too deep to install safely.".to_string());
    }
    if completed.contains(project_id) {
        return Ok(());
    }
    if !valid_project_id(project_id) {
        return Err("A required dependency returned an invalid project identifier.".to_string());
    }
    if !visiting.insert(project_id.to_string()) {
        // Modrinth projects can declare a dependency cycle. The other side of the cycle is
        // already being installed, so do not recurse indefinitely.
        return Ok(());
    }

    let result = (|| {
        let versions = project_versions(client, project_id, game_version)?;
        let version = choose_version(versions, game_version)?;
        install_version(
            client,
            paths,
            manifest,
            version,
            game_version,
            depth,
            visiting,
            completed,
            report,
            on_progress,
        )
    })();

    visiting.remove(project_id);
    if result.is_ok() {
        completed.insert(project_id.to_string());
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn install_version(
    client: &Client,
    paths: &Paths,
    manifest: &mut Manifest,
    version: ProjectVersion,
    game_version: &str,
    depth: usize,
    visiting: &mut HashSet<String>,
    completed: &mut HashSet<String>,
    report: &mut InstallReport,
    on_progress: &mut impl FnMut(&ModProgress),
) -> Result<(), String> {
    if depth > MAX_DEPENDENCY_DEPTH {
        return Err("The mod dependency chain is too deep to install safely.".to_string());
    }
    if !valid_version_id(&version.id) {
        return Err("Modrinth returned an invalid version identifier.".to_string());
    }
    if !version.game_versions.iter().any(|value| value == game_version)
        || !version.loaders.iter().any(|value| value == "fabric")
    {
        return Err(format!(
            "{} does not have a Fabric build for Minecraft {}.",
            version.name, game_version
        ));
    }

    let version_key = format!("version:{}", version.id);
    if completed.contains(&version_key) || !visiting.insert(version_key.clone()) {
        return Ok(());
    }

    let result = (|| {
        for dependency in version
            .dependencies
            .iter()
            .filter(|dependency| dependency.dependency_type == "required")
        {
            if let Some(version_id) = dependency.version_id.as_deref() {
                let dependency_version = project_version_by_id(client, version_id)?;
                install_version(
                    client,
                    paths,
                    manifest,
                    dependency_version,
                    game_version,
                    depth + 1,
                    visiting,
                    completed,
                    report,
                    on_progress,
                )?;
            } else if let Some(project_id) = dependency.project_id.as_deref() {
                install_project(
                    client,
                    paths,
                    manifest,
                    project_id,
                    game_version,
                    depth + 1,
                    visiting,
                    completed,
                    report,
                    on_progress,
                )?;
            } else {
                return Err(format!(
                    "{} has a required dependency without a project or version ID.",
                    version.name
                ));
            }
        }

        install_version_file(client, paths, manifest, &version, report, on_progress)
    })();

    visiting.remove(&version_key);
    if result.is_ok() {
        completed.insert(version_key);
        if !version.project_id.is_empty() {
            completed.insert(version.project_id);
        }
    }
    result
}

fn project_versions(
    client: &Client,
    project_id: &str,
    game_version: &str,
) -> Result<Vec<ProjectVersion>, String> {
    let url = format!(
        "{API}/project/{project_id}/version?loaders=%5B%22fabric%22%5D&game_versions={}&include_changelog=false",
        encode_component(&serde_json::json!([game_version]).to_string())
    );
    client
        .get_json(&url)
        .map_err(|error| format!("Could not resolve a Fabric version: {error}"))
}

fn project_version_by_id(client: &Client, version_id: &str) -> Result<ProjectVersion, String> {
    if !valid_version_id(version_id) {
        return Err("A required dependency returned an invalid version identifier.".to_string());
    }
    client
        .get_json(&format!("{API}/version/{version_id}"))
        .map_err(|error| format!("Could not resolve a required dependency: {error}"))
}

fn choose_version(
    versions: Vec<ProjectVersion>,
    game_version: &str,
) -> Result<ProjectVersion, String> {
    let mut compatible: Vec<ProjectVersion> = versions
        .into_iter()
        .filter(|version| {
            version.game_versions.iter().any(|value| value == game_version)
                && version.loaders.iter().any(|value| value == "fabric")
        })
        .collect();

    if compatible.is_empty() {
        return Err(format!(
            "No Fabric build was found for Minecraft {game_version}."
        ));
    }

    // Prefer stable releases, but don't hide useful projects that publish only beta/alpha builds.
    let selected = compatible
        .iter()
        .position(|version| version.version_type == "release")
        .unwrap_or(0);
    Ok(compatible.swap_remove(selected))
}

#[allow(clippy::too_many_arguments)]
fn install_version_file(
    client: &Client,
    paths: &Paths,
    manifest: &mut Manifest,
    version: &ProjectVersion,
    report: &mut InstallReport,
    on_progress: &mut impl FnMut(&ModProgress),
) -> Result<(), String> {
    let file = version
        .files
        .iter()
        .find(|file| file.primary && valid_jar_filename(&file.filename))
        .or_else(|| version.files.iter().find(|file| valid_jar_filename(&file.filename)))
        .ok_or_else(|| format!("{} does not contain a usable .jar file.", version.name))?;

    if !trusted_download_url(&file.url) {
        return Err(format!("{} has a download URL outside the Modrinth CDN.", version.name));
    }

    let expected_sha1 = file
        .hashes
        .get("sha1")
        .filter(|value| valid_sha1(value))
        .ok_or_else(|| format!("Modrinth did not provide a valid SHA-1 for {}.", version.name))?;

    let project_id = if valid_project_id(&version.project_id) {
        version.project_id.clone()
    } else {
        // Some version-by-ID dependency responses omit the project identifier. The ID is still
        // useful in the managed file name and manifest, but will not be treated as a searchable
        // project until a full project identifier is available.
        version.id.clone()
    };
    let name = if version.name.trim().is_empty() {
        project_id.clone()
    } else {
        version.name.clone()
    };
    let filename = format!("modrinth-{}-{}", project_id, file.filename);
    if !valid_jar_filename(&filename) {
        return Err("The selected mod filename is not safe to install.".to_string());
    }
    let destination = paths.mods.join(&filename);

    let already_installed = manifest.mods.iter().any(|entry| {
        entry.project_id == project_id
            && entry.version_id == version.id
            && managed_file_is_valid(paths, entry)
    });

    if already_installed || (destination.is_file() && http::sha1(&destination).as_deref() == Some(expected_sha1)) {
        remember(
            manifest,
            InstalledProject {
                project_id: project_id.clone(),
                slug: name.clone(),
                title: name.clone(),
                version_id: version.id.clone(),
                version_number: version.version_number.clone(),
                filename: filename.clone(),
                sha1: expected_sha1.to_string(),
            },
        );
        save_manifest(paths, manifest)?;
        report.skipped.push(name.clone());
        on_progress(&progress(
            &project_id,
            &name,
            "skipped",
            "Already installed and checksum verified.",
            0,
            0,
        ));
        return Ok(());
    }

    on_progress(&progress(
        &project_id,
        &name,
        "resolving",
        &format!("Fabric {} · {}", version.version_number, file.filename),
        0,
        0,
    ));
    on_progress(&progress(
        &project_id,
        &name,
        "downloading",
        "Downloading from the verified Modrinth CDN.",
        0,
        0,
    ));

    let download = client
        .download(&file.url, &destination, Some(expected_sha1), |received, total| {
            on_progress(&progress(
                &project_id,
                &name,
                "downloading",
                "Downloading and verifying the mod.",
                received,
                total,
            ));
        })
        .map_err(|error| format!("Could not install {}: {error}", name))?;

    // Remove only an older file that this manifest owns and whose checksum still matches. A
    // player's own jars are never deleted as part of an update.
    for previous in manifest.mods.iter().filter(|entry| {
        entry.project_id == project_id && entry.filename != filename
    }) {
        if managed_file_is_valid(paths, previous) {
            let _ = fs::remove_file(paths.mods.join(&previous.filename));
        }
    }

    remember(
        manifest,
        InstalledProject {
            project_id: project_id.clone(),
            slug: name.clone(),
            title: name.clone(),
            version_id: version.id.clone(),
            version_number: version.version_number.clone(),
            filename: filename.clone(),
            sha1: expected_sha1.to_string(),
        },
    );
    save_manifest(paths, manifest)?;

    report.installed.push(name.clone());
    on_progress(&progress(
        &project_id,
        &name,
        "installed",
        &format!("Installed {} bytes and verified SHA-1.", download.bytes),
        download.bytes,
        download.bytes,
    ));
    Ok(())
}

fn progress(
    project_id: &str,
    name: &str,
    state: &str,
    detail: &str,
    received: u64,
    total: u64,
) -> ModProgress {
    ModProgress {
        project_id: project_id.to_string(),
        name: name.to_string(),
        state: state.to_string(),
        detail: detail.to_string(),
        received,
        total,
    }
}

fn manifest_path(paths: &Paths) -> PathBuf {
    paths.data.join(MANIFEST)
}

fn load_manifest(paths: &Paths) -> Manifest {
    fs::read_to_string(manifest_path(paths))
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_manifest(paths: &Paths, manifest: &Manifest) -> Result<(), String> {
    let content = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("Could not save the Modrinth install manifest: {error}"))?;
    write_atomic(&manifest_path(paths), &content)
        .map_err(|error| format!("Could not save the Modrinth install manifest: {error}"))
}

fn remember(manifest: &mut Manifest, installed: InstalledProject) {
    manifest
        .mods
        .retain(|entry| entry.project_id != installed.project_id);
    manifest.mods.push(installed);
}

fn managed_file_is_valid(paths: &Paths, entry: &InstalledProject) -> bool {
    valid_jar_filename(&entry.filename)
        && valid_sha1(&entry.sha1)
        && http::sha1(&paths.mods.join(&entry.filename)).as_deref() == Some(entry.sha1.as_str())
}

fn valid_project_id(value: &str) -> bool {
    (6..=32).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_version_id(value: &str) -> bool {
    (6..=32).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_sha1(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_jar_filename(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 180
        && value.ends_with(".jar")
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'))
}

fn trusted_download_url(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    if scheme != "https" {
        return false;
    }
    let authority = rest.split('/').next().unwrap_or_default();
    authority.eq_ignore_ascii_case(CDN_HOST) && !authority.contains('@')
}

fn trusted_icon_url(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    if scheme != "https" {
        return false;
    }
    let authority = rest.split('/').next().unwrap_or_default();
    authority.eq_ignore_ascii_case(CDN_HOST) && !authority.contains('@')
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_and_facets_are_percent_encoded() {
        assert_eq!(encode_component("sodium extra"), "sodium%20extra");
        assert_eq!(encode_component("[[\"project_type:mod\"]]"), "%5B%5B%22project_type%3Amod%22%5D%5D");
    }

    #[test]
    fn only_base62_style_project_and_version_ids_are_accepted() {
        assert!(valid_project_id("AABBCCDD"));
        assert!(valid_version_id("IIJJKKLL"));
        assert!(!valid_project_id("../mods"));
        assert!(!valid_project_id("https://example.com"));
        assert!(!valid_version_id("short"));
    }

    #[test]
    fn only_jar_names_without_path_components_are_accepted() {
        assert!(valid_jar_filename("sodium-fabric-1.21.11.jar"));
        assert!(!valid_jar_filename("../sodium.jar"));
        assert!(!valid_jar_filename("C:\\mods\\evil.jar"));
        assert!(!valid_jar_filename(".hidden.jar"));
        assert!(!valid_jar_filename("not-a-jar.zip"));
    }

    #[test]
    fn downloads_must_use_the_exact_https_modrinth_cdn_host() {
        assert!(trusted_download_url("https://cdn.modrinth.com/data/ABCD/file.jar"));
        assert!(!trusted_download_url("http://cdn.modrinth.com/data/ABCD/file.jar"));
        assert!(!trusted_download_url("https://cdn.modrinth.com.evil.example/file.jar"));
        assert!(!trusted_download_url("https://evil.example/cdn.modrinth.com/file.jar"));
        assert!(!trusted_download_url("https://user@cdn.modrinth.com/file.jar"));
    }

    #[test]
    fn checksums_must_be_exact_sha1_hex() {
        assert!(valid_sha1("0123456789012345678901234567890123456789"));
        assert!(!valid_sha1("abcd"));
        assert!(!valid_sha1("g123456789012345678901234567890123456789"));
    }
}
