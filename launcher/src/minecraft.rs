//! The Mojang metadata: the version manifest, the rules and the argument templates.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::http::{get_json, HttpError};

/// The address of the version manifest.
pub const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
/// The address of the metadata of Fabric.
pub const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";
/// The address the assets are downloaded from.
pub const RESOURCE_HOST: &str = "https://resources.download.minecraft.net";
/// The address the libraries are downloaded from.
pub const LIBRARY_HOST: &str = "https://libraries.minecraft.net";

/// The description of the operating system the launcher runs on.
#[derive(Clone, Debug)]
pub struct OsContext {
    /// One of `windows`, `linux` or `osx`.
    pub name: String,
    /// `x86` or `x86_64`.
    pub arch: String,
    /// The version of the operating system.
    pub version: String,
}

/// Detects the operating system in the vocabulary of Minecraft.
pub fn detect_os() -> OsContext {
    let name = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        "x86"
    };

    OsContext {
        name: name.to_string(),
        arch: arch.to_string(),
        version: String::new(),
    }
}

/// The rule of a library or an argument.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Rule {
    /// `allow` or `disallow`.
    #[serde(default)]
    pub action: Option<String>,
    /// The condition on the operating system.
    #[serde(default)]
    pub os: Option<OsRule>,
    /// The condition on the features of the game.
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

/// The operating system of a rule.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

/// The context a rule is evaluated in.
#[derive(Clone, Debug, Default)]
pub struct FeatureContext {
    pub features: HashMap<String, bool>,
}

/// Evaluates a list of rules. The last matching rule wins.
pub fn evaluate_rules(rules: &[Rule], os: &OsContext, features: &FeatureContext) -> bool {
    if rules.is_empty() {
        return true;
    }

    let mut allowed = false;
    for rule in rules {
        if !matches_rule(rule, os, features) {
            continue;
        }
        allowed = rule.action.as_deref() != Some("disallow");
    }
    allowed
}

fn matches_rule(rule: &Rule, os: &OsContext, features: &FeatureContext) -> bool {
    if let Some(condition) = &rule.os {
        if let Some(name) = &condition.name {
            if name != &os.name {
                return false;
            }
        }
        if let Some(arch) = &condition.arch {
            if arch != &os.arch {
                return false;
            }
        }
        if let Some(version) = &condition.version {
            if !version_matches(version, &os.version) {
                return false;
            }
        }
    }

    if let Some(conditions) = &rule.features {
        for (key, value) in conditions {
            if features.features.get(key) != Some(value) {
                return false;
            }
        }
    }

    true
}

fn version_matches(pattern: &str, actual: &str) -> bool {
    if let Some(expression) = pattern.strip_prefix('^') {
        let expression = expression.replace("\\.", ".");
        let expression = format!("^{}$", expression);
        return regex_lite_match(&expression, actual);
    }
    actual == pattern || actual.starts_with(pattern)
}

/// A very small regular expression matcher, it only supports the patterns Mojang uses.
fn regex_lite_match(pattern: &str, text: &str) -> bool {
    let body = pattern.trim_start_matches('^').trim_end_matches('$');
    let parts: Vec<&str> = body.split('.').collect();
    let mut index = 0;

    for (position, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some(expected) = part.strip_prefix('(').and_then(|value| value.strip_suffix(')')) {
            let expected = expected.replace("\\", "");
            if !text[index..].starts_with(&expected) {
                return false;
            }
            index += expected.len();
        } else {
            if index >= text.len() || &text[index..index + 1] != *part {
                return false;
            }
            index += 1;
        }
        if position + 1 == parts.len() && index < text.len() {
            // A trailing wildcard such as "1.8.*" accepts everything that is left.
            return parts[position].ends_with('*');
        }
    }

    true
}

/// The manifest of all the versions of Minecraft.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<VersionEntry>,
}

/// The latest release and snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

/// One entry of the manifest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

/// The description of a version of Minecraft.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "mainClass", default)]
    pub main_class: String,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndex>,
    #[serde(default)]
    pub downloads: Downloads,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub arguments: Arguments,
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersion>,
}

/// The version of Java the game needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

/// The index of the assets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// The downloads of a version.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Downloads {
    #[serde(default)]
    pub client: Option<Download>,
    #[serde(default)]
    pub server: Option<Download>,
}

/// A single downloadable file.
///
/// The client and the server of a version are only described by their checksum and their address,
/// the libraries in addition carry the path they are stored at.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Download {
    #[serde(default)]
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// A library of the game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub url: Option<String>,
}

/// The downloadable files of a library.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Download>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Download>>,
}

/// The arguments of the game.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

/// An entry of an argument template.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    /// A value that is always used.
    Value(String),
    /// A value that depends on the rules and the features.
    Conditional {
        #[serde(default)]
        rules: Vec<Rule>,
        value: serde_json::Value,
    },
}

/// The parts of a maven coordinate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MavenCoordinate {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

/// Splits a maven coordinate into its parts.
pub fn parse_maven_coordinate(coordinate: &str) -> MavenCoordinate {
    let (body, extension) = match coordinate.split_once('@') {
        Some((body, extension)) => (body, extension.to_string()),
        None => (coordinate, "jar".to_string()),
    };

    let mut parts = body.split(':');
    MavenCoordinate {
        group: parts.next().unwrap_or_default().to_string(),
        artifact: parts.next().unwrap_or_default().to_string(),
        version: parts.next().unwrap_or_default().to_string(),
        classifier: parts.next().map(|value| value.to_string()),
        extension,
    }
}

/// Accepts a single path component that is safe to use as a local directory/file name.
pub fn is_safe_path_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'))
}

/// Accepts only a non-empty relative path made of ordinary forward-slash-separated components.
pub fn is_safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains('\\')
        && !value.contains(':')
        && !value.contains('\0')
        && value
            .split('/')
            .all(is_safe_path_component)
}

/// Minecraft asset object names are SHA-1 digests.
pub fn is_sha1_hash(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Converts a maven coordinate into the relative path inside the libraries folder.
pub fn maven_path(coordinate: &str) -> String {
    let parts = parse_maven_coordinate(coordinate);
    let file = match &parts.classifier {
        Some(classifier) => format!(
            "{}-{}-{}.{}",
            parts.artifact, parts.version, classifier, parts.extension
        ),
        None => format!("{}-{}.{}", parts.artifact, parts.version, parts.extension),
    };

    format!(
        "{}/{}/{}/{}",
        parts.group.replace('.', "/"),
        parts.artifact,
        parts.version,
        file
    )
}

/// Detects whether a coordinate refers to a native artifact.
///
/// Since 1.19 Mojang publishes the natives as regular libraries with a classifier
/// (`org.lwjgl:lwjgl:3.3.3:natives-windows`) instead of using the legacy `natives` key.
pub fn is_native_coordinate(coordinate: &str) -> bool {
    match parse_maven_coordinate(coordinate).classifier {
        Some(classifier) => classifier.starts_with("natives-")
            || classifier.starts_with("natives.")
            || classifier.starts_with("linux-")
            || classifier.starts_with("linux.")
            || classifier.starts_with("osx-")
            || classifier.starts_with("osx.")
            || classifier.starts_with("windows-")
            || classifier.starts_with("windows."),
        None => false,
    }
}

/// A library that was resolved into a concrete file.
#[derive(Clone, Debug)]
pub struct ResolvedLibrary {
    pub name: String,
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub native: bool,
}

/// Downloads the version manifest.
pub fn fetch_manifest() -> Result<Manifest, HttpError> {
    get_json(VERSION_MANIFEST_URL)
}

/// Every released version, newest first.
pub fn released_versions(manifest: &Manifest, limit: usize) -> Vec<VersionEntry> {
    manifest
        .versions
        .iter()
        .filter(|entry| entry.kind == "release" && is_safe_path_component(&entry.id))
        .take(limit)
        .cloned()
        .collect()
}

/// Downloads the description of a version.
pub fn fetch_version_json(version_id: &str) -> Result<VersionJson, HttpError> {
    if !is_safe_path_component(version_id) {
        return Err(HttpError::Uri(format!("invalid Minecraft version identifier: {}", version_id)));
    }
    let manifest = fetch_manifest()?;
    let entry = manifest
        .versions
        .iter()
        .find(|candidate| candidate.id == version_id)
        .ok_or_else(|| HttpError::Uri(format!("unknown Minecraft version: {}", version_id)))?;

    get_json(&entry.url)
}

/// Downloads the profile of Fabric for a version.
pub fn fetch_fabric_profile(
    game_version: &str,
    loader_version: &str,
) -> Result<VersionJson, HttpError> {
    if !is_safe_path_component(game_version) || !is_safe_path_component(loader_version) {
        return Err(HttpError::Uri("invalid Minecraft or Fabric version identifier".to_string()));
    }
    let url = format!(
        "{}/versions/loader/{}/{}/profile/json",
        FABRIC_META_URL, game_version, loader_version
    );
    get_json(&url)
}

/// The loaders that support a version.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FabricLoaderEntry {
    pub loader: FabricLoader,
}

/// The description of a loader.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FabricLoader {
    pub version: String,
}

/// Lists the loaders of a version.
pub fn fetch_fabric_loaders(game_version: &str) -> Result<Vec<String>, HttpError> {
    if !is_safe_path_component(game_version) {
        return Err(HttpError::Uri(format!("invalid Minecraft version identifier: {}", game_version)));
    }
    let url = format!("{}/versions/loader/{}", FABRIC_META_URL, game_version);
    let entries: Vec<FabricLoaderEntry> = get_json(&url)?;
    Ok(entries
        .into_iter()
        .map(|entry| entry.loader.version)
        .collect())
}

/// Merges a Fabric profile into the vanilla description, the same way the vanilla launcher does.
pub fn merge_version_json(vanilla: &VersionJson, fabric: &VersionJson) -> VersionJson {
    let mut libraries = merge_libraries(&vanilla.libraries, &fabric.libraries);

    let mut game = vanilla.arguments.game.clone();
    game.extend(fabric.arguments.game.clone());
    let mut jvm = vanilla.arguments.jvm.clone();
    jvm.extend(fabric.arguments.jvm.clone());

    let id = if fabric.id.is_empty() {
        vanilla.id.clone()
    } else {
        fabric.id.clone()
    };
    let main_class = if fabric.main_class.is_empty() {
        vanilla.main_class.clone()
    } else {
        fabric.main_class.clone()
    };

    libraries.shrink_to_fit();

    VersionJson {
        id,
        main_class,
        assets: vanilla.assets.clone(),
        asset_index: vanilla.asset_index.clone(),
        downloads: vanilla.downloads.clone(),
        libraries,
        arguments: Arguments { game, jvm },
        inherits_from: Some(vanilla.id.clone()),
        kind: vanilla.kind.clone(),
        java_version: vanilla.java_version.clone(),
    }
}

/// Merges two library lists, the entry that carries the download descriptor wins.
pub fn merge_libraries(base: &[Library], extra: &[Library]) -> Vec<Library> {
    let mut result: Vec<Library> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for library in base.iter().chain(extra.iter()) {
        let parts = parse_maven_coordinate(&library.name);
        let key = format!(
            "{}:{}:{}:{}",
            parts.group,
            parts.artifact,
            parts.version,
            parts.classifier.unwrap_or_default()
        );

        match index.get(&key) {
            Some(position) => {
                let existing = result[*position].clone();
                let mut merged = existing.clone();
                if library.downloads.is_some() {
                    merged.downloads = library.downloads.clone();
                }
                if library.url.is_some() {
                    merged.url = library.url.clone();
                }
                if library.natives.is_some() {
                    merged.natives = library.natives.clone();
                }
                if !library.rules.is_empty() {
                    merged.rules = library.rules.clone();
                }
                result[*position] = merged;
            }
            None => {
                index.insert(key, result.len());
                result.push(library.clone());
            }
        }
    }

    result
}

/// Resolves the libraries of a version into concrete files.
pub fn resolve_libraries(version: &VersionJson, os: &OsContext) -> Vec<ResolvedLibrary> {
    let features = FeatureContext::default();
    let mut resolved = Vec::new();

    for library in &version.libraries {
        if !evaluate_rules(&library.rules, os, &features) {
            continue;
        }

        let parts = parse_maven_coordinate(&library.name);
        let native = is_native_coordinate(&library.name);

        if let Some(downloads) = &library.downloads {
            if let Some(artifact) = &downloads.artifact {
                let path = if artifact.path.is_empty() {
                    maven_path(&library.name)
                } else {
                    artifact.path.clone()
                };
                resolved.push(ResolvedLibrary {
                    name: library.name.clone(),
                    path,
                    url: artifact.url.clone(),
                    sha1: Some(artifact.sha1.clone()),
                    size: Some(artifact.size),
                    native,
                });
            }

            if let Some(classifiers) = &downloads.classifiers {
                for (classifier, download) in classifiers {
                    resolved.push(ResolvedLibrary {
                        name: format!(
                            "{}:{}:{}:{}",
                            parts.group, parts.artifact, parts.version, classifier
                        ),
                        path: download.path.clone(),
                        url: download.url.clone(),
                        sha1: Some(download.sha1.clone()),
                        size: Some(download.size),
                        native: true,
                    });
                }
            }
        } else if let Some(base) = &library.url {
            // The Fabric profile carries the maven repository instead of a direct url.
            let relative = maven_path(&library.name);
            resolved.push(ResolvedLibrary {
                name: library.name.clone(),
                path: relative.clone(),
                url: format!(
                    "{}/{}",
                    base.trim_end_matches('/'),
                    relative.replace('\\', "/")
                ),
                sha1: None,
                size: None,
                native,
            });
        }

        // The legacy layout, the natives live in a separate classifier of the same artifact.
        if let Some(natives) = &library.natives {
            if let Some(classifier) = natives.get(&os.name) {
                let coordinate = format!(
                    "{}:{}:{}:{}",
                    parts.group, parts.artifact, parts.version, classifier
                );
                let relative = maven_path(&coordinate);

                let from_classifiers = library
                    .downloads
                    .as_ref()
                    .and_then(|downloads| downloads.classifiers.as_ref())
                    .and_then(|classifiers| classifiers.get(classifier));

                if let Some(download) = from_classifiers {
                    resolved.push(ResolvedLibrary {
                        name: coordinate,
                        path: download.path.clone(),
                        url: download.url.clone(),
                        sha1: Some(download.sha1.clone()),
                        size: Some(download.size),
                        native: true,
                    });
                } else {
                    let base = library
                        .downloads
                        .as_ref()
                        .and_then(|downloads| downloads.artifact.as_ref())
                        .map(|artifact| {
                            artifact
                                .url
                                .rsplit_once('/')
                                .map(|(prefix, _)| prefix.to_string())
                                .unwrap_or_else(|| LIBRARY_HOST.to_string())
                        })
                        .or_else(|| library.url.clone())
                        .unwrap_or_else(|| LIBRARY_HOST.to_string());

                    resolved.push(ResolvedLibrary {
                        name: coordinate,
                        path: relative.clone(),
                        url: format!("{}/{}", base.trim_end_matches('/'), relative.replace('\\', "/")),
                        sha1: None,
                        size: None,
                        native: true,
                    });
                }
            }
        }
    }

    resolved
}

/// Evaluates an argument template.
pub fn evaluate_argument_template(
    template: &[Argument],
    os: &OsContext,
    features: &FeatureContext,
) -> Vec<String> {
    let mut result = Vec::new();

    for argument in template {
        match argument {
            Argument::Value(value) => result.push(value.clone()),
            Argument::Conditional { rules, value } => {
                if !evaluate_rules(rules, os, features) {
                    continue;
                }
                match value {
                    serde_json::Value::String(text) => result.push(text.clone()),
                    serde_json::Value::Array(values) => {
                        for entry in values {
                            if let Some(text) = entry.as_str() {
                                result.push(text.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    result
}

/// Replaces the `${placeholder}` tokens of an argument.
pub fn apply_replacements(value: &str, replacements: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start..];
        match rest.find('}') {
            Some(end) => {
                let key = &rest[2..end];
                match replacements.get(key) {
                    Some(replacement) => result.push_str(replacement),
                    None => result.push_str(&rest[..=end]),
                }
                rest = &rest[end + 1..];
            }
            None => break,
        }
    }
    result.push_str(rest);
    result
}

/// The address of an asset object on the official resource host.
pub fn asset_url(hash: &str) -> String {
    asset_url_from(RESOURCE_HOST, hash)
}

/// The address of an asset object on the given host.
pub fn asset_url_from(host: &str, hash: &str) -> String {
    format!("{}/{}/{}", host.trim_end_matches('/'), &hash[..2], hash)
}

/// The relative path of an asset object inside the assets folder.
pub fn asset_path(hash: &str) -> String {
    format!("objects/{}/{}", &hash[..2], hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windows() -> OsContext {
        OsContext {
            name: "windows".to_string(),
            arch: "x86_64".to_string(),
            version: "10.0.19045".to_string(),
        }
    }

    #[test]
    fn splits_maven_coordinates() {
        let parts = parse_maven_coordinate("org.ow2.asm:asm:9.10.1");
        assert_eq!(parts.group, "org.ow2.asm");
        assert_eq!(parts.artifact, "asm");
        assert_eq!(parts.version, "9.10.1");
        assert!(parts.classifier.is_none());
        assert_eq!(parts.extension, "jar");
    }

    #[test]
    fn rejects_path_components_that_could_escape_launcher_directories() {
        for invalid in ["", ".", "..", "../outside", "C:\\outside", "/outside", "folder\\file", "a:b"] {
            assert!(!is_safe_path_component(invalid), "accepted component: {invalid}");
        }
        assert!(is_safe_path_component("1.21.11"));
        assert!(is_safe_path_component("0.141.6+1.21.11"));
    }

    #[test]
    fn validates_library_paths_and_asset_hashes() {
        assert!(is_safe_relative_path("org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"));
        assert!(!is_safe_relative_path("../outside.jar"));
        assert!(!is_safe_relative_path("org/../../outside.jar"));
        assert!(!is_safe_relative_path("/absolute/file.jar"));
        assert!(!is_safe_relative_path("org\\outside.jar"));
        assert!(is_sha1_hash("0123456789abcdef0123456789abcdef01234567"));
        assert!(!is_sha1_hash("../etc/passwd"));
    }

    #[test]
    fn converts_coordinates_into_paths() {
        assert_eq!(
            maven_path("org.ow2.asm:asm:9.10.1"),
            "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows"),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
    }

    #[test]
    fn detects_native_libraries() {
        assert!(is_native_coordinate("org.lwjgl:lwjgl:3.3.3:natives-windows"));
        assert!(is_native_coordinate(
            "io.netty:netty-transport-native-epoll:4.2.7:linux-x86_64"
        ));
        assert!(!is_native_coordinate("org.lwjgl:lwjgl:3.3.3"));
        assert!(!is_native_coordinate("com.google.guava:guava:33.5.0-jre"));
    }

    #[test]
    fn evaluates_the_rules() {
        let allow_osx = vec![Rule {
            action: Some("allow".to_string()),
            os: Some(OsRule {
                name: Some("osx".to_string()),
                version: None,
                arch: None,
            }),
            features: None,
        }];
        assert!(!evaluate_rules(&allow_osx, &windows(), &FeatureContext::default()));
        assert!(evaluate_rules(&[], &windows(), &FeatureContext::default()));
    }

    #[test]
    fn replaces_the_placeholders() {
        let mut replacements = HashMap::new();
        replacements.insert("a".to_string(), "1".to_string());
        assert_eq!(apply_replacements("${a}-${b}", &replacements), "1-${b}");
    }
}
