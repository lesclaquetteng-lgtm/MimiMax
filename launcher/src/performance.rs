//! The optimisation pack and the tuning of the game options.
//!
//! Two levers are independent and complementary:
//!
//! * **the mods** are resolved from Modrinth for the exact version of the game, downloaded from the
//!   official CDN and verified with the SHA-1 that the API publishes;
//! * **`options.txt`** is rewritten with a preset that removes the expensive effects. The original
//!   file is copied once to `options.txt.aether-backup`, so that the player can go back.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::pending::{self, PendingInstall};

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
        description:
            "Skips the entities that are hidden behind blocks, using asynchronous ray casts.",
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
#[derive(Clone, Debug, Deserialize)]
struct ModrinthVersion {
    /// The project the version belongs to, two slugs can name the same project.
    #[serde(default)]
    project_id: String,
    version_number: String,
    /// The versions of Minecraft the build announces, the pack version has to be one of them.
    #[serde(default)]
    game_versions: Vec<String>,
    /// The loaders the build announces, the one of the pack has to be one of them.
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    dependencies: Vec<ModrinthDependency>,
    #[serde(default)]
    files: Vec<ModrinthFile>,
}

/// A dependency a version declares.
#[derive(Clone, Debug, Default, Deserialize)]
struct ModrinthDependency {
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    /// `required`, `optional`, `incompatible` or `embedded`.
    #[serde(default)]
    dependency_type: String,
}

/// The project a dependency identifies, the API names it by its identifier.
#[derive(Clone, Debug, Deserialize)]
struct ModrinthProject {
    slug: String,
    #[serde(default)]
    title: String,
}

/// One file of a version.
#[derive(Clone, Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: ModrinthHashes,
}

/// The checksums of a file.
#[derive(Clone, Debug, Default, Deserialize)]
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
    /// `true` when the mod is a dependency of the pack rather than one of its entries.
    #[serde(default)]
    dependency: bool,
}

/// What the installation of the pack did.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<FailedMod>,
    /// The dependencies the graph downloads with the pack.
    pub dependencies: Vec<ResolvedDependency>,
    /// The declared dependencies that were deliberately not installed.
    pub notes: Vec<DependencyNote>,
    /// The projects that declare each other incompatible.
    pub conflicts: Vec<Conflict>,
    /// The required part of the graph that could not be resolved.
    pub unresolved: Vec<FailedMod>,
    /// `true` when the graph was incomplete before the first download.
    pub aborted: bool,
}

/// A dependency that the graph downloads with the pack.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDependency {
    pub slug: String,
    pub name: String,
    pub required_by: String,
    /// The version the dependency was installed at.
    pub version: String,
    /// `true` when the dependent mod pinned that exact version.
    pub pinned: bool,
}

/// A declared dependency that is not installed, and why.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyNote {
    pub slug: String,
    pub required_by: String,
    /// `optional`, `embedded` or `incompatible`.
    pub kind: String,
}

/// Two projects that declare each other incompatible.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub slug: String,
    pub with: String,
    pub reason: String,
}

/// A mod that could not be installed.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedMod {
    pub slug: String,
    pub error: String,
    /// `true` when the pack cannot be installed without it.
    #[serde(default)]
    pub required: bool,
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
    // A filename from a remote catalogue is untrusted, even when it is joined only to the mods
    // directory. Reject separators and Windows alternate data streams explicitly instead of
    // depending on the host platform's Path parser.
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains(':')
        || filename.contains('\0')
        || filename.chars().any(char::is_control)
        || filename.contains('<')
        || filename.contains('>')
        || filename.contains('"')
        || filename.contains('|')
        || filename.contains('?')
        || filename.contains('*')
        || filename.ends_with(' ')
        || filename.ends_with('.')
    {
        return Err(format!("{:?} is not a safe file name", filename));
    }

    let name = Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{:?} is not a file name", filename))?;

    if name != filename {
        return Err(format!("{:?} contains a directory", filename));
    }

    // Windows reserves these device names even when an extension is present (for example,
    // CON.jar). Such names do not behave like ordinary files on the supported target.
    let device = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    if matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (device.len() == 4
            && (device.starts_with("COM") || device.starts_with("LPT"))
            && matches!(device.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(format!("{:?} is a reserved file name", filename));
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
            let installed = manifest.mods.iter().find(|entry| {
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

/// The address of the Modrinth API the pack is resolved from.
const MODRINTH_API: &str = "https://api.modrinth.com/v2";

/// The mod loader of the pack.
const MODRINTH_LOADER: &str = "fabric";

/// One project the graph decided to install.
struct Planned {
    slug: String,
    name: String,
    /// The project of the API, the key that removes a duplicate from the graph.
    project_id: String,
    version: String,
    /// The version a dependency pinned, `None` for a build the graph chose.
    pinned_version: Option<String>,
    /// The project that required it, `None` for an entry of the pack.
    required_by: Option<String>,
    url: String,
    filename: String,
    checksum: String,
    /// `true` when the pack cannot be installed without it.
    required: bool,
}

/// One project the graph still has to resolve.
struct Job {
    slug: String,
    name: String,
    required: bool,
    /// The project that declared it, `None` for an entry of the pack.
    required_by: Option<String>,
    /// The exact version a dependency pinned. A pinned job never falls back to the newest build.
    version: Option<String>,
}

/// A required dependency a version declared, kept until the graph has one version per project.
struct Declaration {
    slug: String,
    name: String,
    required_by: String,
    /// The project identifier the dependency named, when it named one.
    project_id: Option<String>,
}

/// What the resolution produced, before a single byte is downloaded.
#[derive(Default)]
struct Graph {
    planned: Vec<Planned>,
    dependencies: Vec<ResolvedDependency>,
    notes: Vec<DependencyNote>,
    conflicts: Vec<Conflict>,
    unresolved: Vec<FailedMod>,
    aborted: bool,
}

/// Why a version cannot be installed in the pack, `None` when it can.
///
/// A version that lists its `game_versions` or its `loaders` has to contain the ones of the pack.
/// A pinned version that lists none is rejected: nothing proves that a version the API did not
/// filter runs on this Minecraft. An unpinned version comes from a query filtered on exactly those
/// fields, so a field the API left out is still its answer for the pack.
fn rejection(
    version: &ModrinthVersion,
    game_version: &str,
    loader: &str,
    pinned: bool,
) -> Option<String> {
    let missing = |values: &[String], wanted: &str| {
        !values.iter().any(|value| value == wanted) && (pinned || !values.is_empty())
    };

    if missing(&version.game_versions, game_version) {
        Some(format!("does not support Minecraft {}", game_version))
    } else if missing(&version.loaders, loader) {
        Some(format!("is not published for {}", loader))
    } else {
        None
    }
}

/// Resolves the pack and its required dependencies from the API.
///
/// The graph is complete before the first download: a required library that does not resolve stops
/// the installation while nothing has been written, instead of leaving a pack that the game
/// refuses to start. A project appears once, whether it is an entry of the pack, a dependency of
/// another entry, or both — Cloth Config is the case that matters, since MoreCulling and Dynamic
/// FPS need it and the pack already ships it.
fn build_graph(
    client: &Client,
    api: &str,
    game_version: &str,
    only: &[String],
    on_progress: &mut impl FnMut(&ModProgress),
) -> Graph {
    let mut graph = Graph::default();
    let mut queue: VecDeque<Job> = VecDeque::new();
    let mut declarations: Vec<Declaration> = Vec::new();
    // A slug is claimed as soon as it is queued, not when it is resolved: a project that the pack
    // lists and that another entry requires must be queued once, otherwise it is resolved twice
    // and downloaded twice. A pinned dependency is the exception: it is queued even when the
    // project is known, because its exact version has to be compared with the one already planned.
    let mut claimed_slugs: HashSet<String> = HashSet::new();

    for spec in PACK {
        if !only.is_empty() && !only.iter().any(|slug| slug == spec.slug) {
            continue;
        }

        claimed_slugs.insert(spec.slug.to_string());
        queue.push_back(Job {
            slug: spec.slug.to_string(),
            name: spec.name.to_string(),
            required: spec.required,
            required_by: None,
            version: None,
        });
    }

    while let Some(job) = queue.pop_front() {
        let slug = job.slug.to_ascii_lowercase();

        on_progress(&ModProgress {
            slug: slug.clone(),
            name: job.name.clone(),
            state: "resolving".to_string(),
            detail: match &job.version {
                Some(version) => format!("reading pinned version {}", version),
                None => "reading the version graph on Modrinth".to_string(),
            },
            received: 0,
            total: 0,
        });

        // A pinned dependency is installed at the version it names, with the digest of that
        // version: the newest build of the project is never substituted for it.
        let version = match &job.version {
            Some(version_id) => fetch_version(client, api, version_id),
            None => resolve(client, api, &slug, game_version),
        };

        let version = match version {
            Ok(version) => version,
            Err(error) => {
                graph.unresolved.push(FailedMod {
                    slug: slug.clone(),
                    error,
                    required: job.required,
                });
                continue;
            }
        };

        // Every node is checked against the game version and the loader of the pack: a build that
        // was published for another Minecraft or another loader would only fail at start up.
        if let Some(reason) = rejection(
            &version,
            game_version,
            MODRINTH_LOADER,
            job.version.is_some(),
        ) {
            graph.unresolved.push(FailedMod {
                slug: slug.clone(),
                error: format!("{} {} {}", job.name, version.version_number, reason),
                required: job.required,
            });
            continue;
        }

        let Some(planned) = plan(&slug, &job, &version) else {
            graph.unresolved.push(FailedMod {
                slug: slug.clone(),
                error: "the build has no verifiable jar (file, name or SHA-1 missing)".to_string(),
                required: job.required,
            });
            continue;
        };

        // The incompatible declarations are read first: a project that another one rejects must not
        // be queued only to be dropped later.
        for dependency in &version.dependencies {
            if dependency.dependency_type != "incompatible" {
                continue;
            }

            let label = dependency_label(dependency);
            graph.conflicts.push(Conflict {
                slug: slug.clone(),
                with: label.clone(),
                reason: format!("{} declares it incompatible", job.name),
            });
            graph.notes.push(DependencyNote {
                slug: label,
                required_by: slug.clone(),
                kind: "incompatible".to_string(),
            });
        }

        for dependency in &version.dependencies {
            match dependency.dependency_type.as_str() {
                "required" => {
                    let label = dependency_label(dependency);
                    let target = match dependency_target(client, api, dependency) {
                        Ok(target) => target,
                        Err(error) => {
                            graph.unresolved.push(FailedMod {
                                slug: label,
                                error: format!("required dependency of {}: {}", job.name, error),
                                required: job.required,
                            });
                            continue;
                        }
                    };

                    let target_slug = target.slug.to_ascii_lowercase();
                    let pin = dependency
                        .version_id
                        .clone()
                        .filter(|id| !id.trim().is_empty());

                    if graph.conflicts.iter().any(|conflict| {
                        (conflict.slug == slug && conflict.with == target_slug)
                            || (conflict.slug == target_slug && conflict.with == slug)
                    }) {
                        graph.notes.push(DependencyNote {
                            slug: target_slug,
                            required_by: slug.clone(),
                            kind: "incompatible".to_string(),
                        });
                        continue;
                    }

                    declarations.push(Declaration {
                        slug: target_slug.clone(),
                        name: target_name(&target),
                        required_by: slug.clone(),
                        project_id: dependency
                            .project_id
                            .clone()
                            .filter(|id| !id.trim().is_empty()),
                    });

                    // An unpinned dependency of a project that is already planned is that node:
                    // only a pin has to be resolved, so that its version can be compared with the
                    // one the graph already holds.
                    if pin.is_none() && claimed_slugs.contains(&target_slug) {
                        continue;
                    }

                    claimed_slugs.insert(target_slug.clone());
                    queue.push_back(Job {
                        slug: target_slug,
                        name: target_name(&target),
                        // A dependency is only as required as the project that needs it.
                        required: job.required,
                        required_by: Some(slug.clone()),
                        version: pin,
                    });
                }
                "optional" => graph.notes.push(DependencyNote {
                    slug: dependency_label(dependency),
                    required_by: slug.clone(),
                    kind: "optional".to_string(),
                }),
                "embedded" => graph.notes.push(DependencyNote {
                    slug: dependency_label(dependency),
                    required_by: slug.clone(),
                    kind: "embedded".to_string(),
                }),
                _ => {}
            }
        }

        graph.planned.push(planned);
    }

    graph.planned = reconcile(graph.planned, &mut graph.conflicts);
    graph.dependencies = dependencies(&graph.planned, declarations);
    graph.aborted = graph.unresolved.iter().any(|entry| entry.required);
    graph
}

/// The key of a node in the graph: the project when the API named it, its slug otherwise.
fn project_key(planned: &Planned) -> String {
    if planned.project_id.is_empty() {
        planned.slug.clone()
    } else {
        planned.project_id.clone()
    }
}

/// Keeps one node per project and reports the pinned versions that contradict each other.
///
/// The first declaration of a project owns its download. A later pinned version of the same
/// project is never installed silently in place of it: the difference is a conflict of the report,
/// because two mods that pin two versions cannot both be satisfied. An explicit pin replaces a
/// build the graph chose on its own, since the dependent mod was built against that version.
fn reconcile(planned: Vec<Planned>, conflicts: &mut Vec<Conflict>) -> Vec<Planned> {
    let mut kept: Vec<Planned> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();

    for current in planned {
        let key = project_key(&current);

        let Some(&index) = positions.get(&key) else {
            positions.insert(key, kept.len());
            kept.push(current);
            continue;
        };

        let existing_pin = kept[index].pinned_version.clone();
        let existing_slug = kept[index].slug.clone();

        match (existing_pin, current.pinned_version.clone()) {
            (Some(pinned), Some(other)) if pinned != other => {
                let required_by = current
                    .required_by
                    .clone()
                    .unwrap_or_else(|| current.slug.clone());

                conflicts.push(Conflict {
                    slug: required_by.clone(),
                    with: existing_slug.clone(),
                    reason: format!(
                        "{} requires version {} of {} while the graph already pins {}",
                        required_by, other, current.slug, pinned
                    ),
                });
            }
            (None, Some(_)) => kept[index] = current,
            _ => {}
        }
    }

    kept
}

/// The reported edges, once every project of the graph carries one version.
fn dependencies(planned: &[Planned], declarations: Vec<Declaration>) -> Vec<ResolvedDependency> {
    let mut by_project: HashMap<&str, &Planned> = HashMap::new();
    let mut by_slug: HashMap<&str, &Planned> = HashMap::new();

    for node in planned {
        by_slug.insert(node.slug.as_str(), node);
        if !node.project_id.is_empty() {
            by_project.insert(node.project_id.as_str(), node);
        }
    }

    declarations
        .into_iter()
        .filter_map(|declaration| {
            let node = declaration
                .project_id
                .as_deref()
                .and_then(|id| by_project.get(id).copied())
                .or_else(|| by_slug.get(declaration.slug.as_str()).copied())?;

            Some(ResolvedDependency {
                slug: node.slug.clone(),
                name: declaration.name,
                required_by: declaration.required_by,
                version: node.version.clone(),
                pinned: node.pinned_version.is_some(),
            })
        })
        .collect()
}

/// The name a dependency is reported with.
fn target_name(project: &ModrinthProject) -> String {
    if project.title.trim().is_empty() {
        project.slug.clone()
    } else {
        project.title.clone()
    }
}

/// The name a dependency is reported with before its project is known.
fn dependency_label(dependency: &ModrinthDependency) -> String {
    if let Some(project) = dependency
        .project_id
        .as_ref()
        .filter(|id| !id.trim().is_empty())
    {
        return project.clone();
    }

    dependency
        .version_id
        .clone()
        .unwrap_or_else(|| "unknown".to_string())
}

/// Turns a resolved version into the download it describes.
fn plan(slug: &str, job: &Job, version: &ModrinthVersion) -> Option<Planned> {
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())?;
    let filename = jar_name(&file.filename).ok()?;
    let checksum = normalized_sha1(&file.hashes.sha1)?;
    let url = normalized_url(&file.url)?;

    Some(Planned {
        slug: slug.to_string(),
        name: job.name.clone(),
        project_id: version.project_id.clone(),
        version: version.version_number.clone(),
        pinned_version: job.version.clone(),
        required_by: job.required_by.clone(),
        url,
        filename,
        checksum,
        required: job.required,
    })
}

/// A published SHA-1, lower-cased, `None` when the API did not publish a usable one.
fn normalized_sha1(value: &str) -> Option<String> {
    let sha1 = value.trim().to_ascii_lowercase();

    if sha1.len() == 40 && sha1.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(sha1)
    } else {
        None
    }
}

/// A download address, `None` for another scheme.
///
/// The address comes from a remote catalogue and becomes a request of the launcher; a `file://` or
/// a custom scheme would read something other than the CDN.
fn normalized_url(url: &str) -> Option<String> {
    let url = url.trim();

    if url.starts_with("https://") || url.starts_with("http://") {
        Some(url.to_string())
    } else {
        None
    }
}

/// Installs the pack.
///
/// `only` limits the installation to some slugs; an empty slice installs the whole pack. A mod that
/// fails does not stop the others, but an incomplete dependency graph stops everything before the
/// first download: a pack that misses a library its mods need does not start.
pub fn install(
    paths: &Paths,
    game_version: &str,
    only: &[String],
    on_progress: impl FnMut(&ModProgress),
) -> InstallReport {
    install_from(paths, game_version, only, MODRINTH_API, on_progress)
}

/// [`install`] with an explicit API address, so a test can serve the graph from loopback.
fn install_from(
    paths: &Paths,
    game_version: &str,
    only: &[String],
    api: &str,
    mut on_progress: impl FnMut(&ModProgress),
) -> InstallReport {
    let client = Client::new();
    let graph = build_graph(&client, api, game_version, only, &mut on_progress);

    let mut report = InstallReport {
        dependencies: graph.dependencies,
        notes: graph.notes,
        conflicts: graph.conflicts,
        unresolved: graph.unresolved,
        aborted: graph.aborted,
        ..InstallReport::default()
    };

    if report.aborted {
        for entry in report.unresolved.iter().filter(|entry| entry.required) {
            on_progress(&ModProgress {
                slug: entry.slug.clone(),
                name: entry.slug.clone(),
                state: "failed".to_string(),
                detail: entry.error.clone(),
                received: 0,
                total: 0,
            });
        }

        // Nothing was written: the pack is not left half installed without the library a mod of
        // the pack needs to start.
        return report;
    }

    let mut manifest = load_manifest(paths);

    // The journal is what lets a later run say that a pack installation was interrupted and finish
    // it: the jars that are already verified are skipped, the others are fetched.
    let mut journal = PendingInstall::new("mods", game_version);
    journal.slugs = graph
        .planned
        .iter()
        .map(|planned| planned.slug.clone())
        .collect();
    journal.total = graph.planned.len();
    let _ = pending::begin(paths, &journal);

    for (index, planned) in graph.planned.iter().enumerate() {
        let announce = |state: &str, detail: &str, received: u64, total: u64| ModProgress {
            slug: planned.slug.clone(),
            name: planned.name.clone(),
            state: state.to_string(),
            detail: detail.to_string(),
            received,
            total,
        };

        let destination = paths.mods.join(&planned.filename);
        if destination.exists()
            && http::sha1(&destination).as_deref() == Some(planned.checksum.as_str())
        {
            remember(&mut manifest, planned);
            on_progress(&announce("skipped", "already installed", 0, 0));
            report.skipped.push(planned.slug.clone());
        } else {
            on_progress(&announce("downloading", &planned.filename, 0, 0));

            match client.download(
                &planned.url,
                &destination,
                Some(&planned.checksum),
                |received, total| {
                    on_progress(&announce("downloading", &planned.filename, received, total))
                },
            ) {
                Ok(download) => {
                    remember(&mut manifest, planned);
                    on_progress(&announce(
                        "done",
                        &format!("{} bytes", download.bytes),
                        download.bytes,
                        download.bytes,
                    ));
                    report.installed.push(planned.slug.clone());
                }
                Err(error) => {
                    let detail = error.to_string();
                    on_progress(&announce("failed", &detail, 0, 0));
                    report.failed.push(FailedMod {
                        slug: planned.slug.clone(),
                        error: detail,
                        required: planned.required,
                    });
                }
            }
        }

        pending::record(paths, "Downloading the pack", index + 1);
    }

    save_manifest(paths, &manifest);
    // The run is over, even when a mod of the pack failed: the journal only speaks about files
    // that still have to be fetched, and the failed mods are in the report.
    pending::close(paths);
    report
}

/// Reads one exact version of the API, the one a dependency pinned.
fn fetch_version(client: &Client, api: &str, version_id: &str) -> Result<ModrinthVersion, String> {
    client
        .get_json(&format!(
            "{}/version/{}",
            api.trim_end_matches('/'),
            version_id
        ))
        .map_err(|error| error.to_string())
}

/// Asks Modrinth for the newest Fabric build of a mod for the game version.
fn resolve(
    client: &Client,
    api: &str,
    slug: &str,
    game_version: &str,
) -> Result<ModrinthVersion, String> {
    let url = format!(
        "{}/project/{}/version?loaders=%5B%22{}%22%5D&game_versions=%5B%22{}%22%5D",
        api.trim_end_matches('/'),
        slug,
        MODRINTH_LOADER,
        game_version
    );

    let versions: Vec<ModrinthVersion> =
        client.get_json(&url).map_err(|error| error.to_string())?;
    versions
        .into_iter()
        .next()
        .ok_or_else(|| format!("no Fabric build for {} on {}", slug, game_version))
}

/// Resolves the project a required dependency names.
///
/// A dependency can carry the identifier of its project directly, or only the identifier of a
/// version; the version then names its project. The slug and the title have to be read from the
/// project, because a dependency never carries them.
fn dependency_target(
    client: &Client,
    api: &str,
    dependency: &ModrinthDependency,
) -> Result<ModrinthProject, String> {
    let api = api.trim_end_matches('/');

    let project_id = match dependency
        .project_id
        .as_ref()
        .filter(|id| !id.trim().is_empty())
    {
        Some(project) => project.clone(),
        None => {
            let version_id = dependency
                .version_id
                .as_ref()
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| {
                    "the dependency names neither a project nor a version".to_string()
                })?;

            let version: ModrinthVersion = client
                .get_json(&format!("{}/version/{}", api, version_id))
                .map_err(|error| error.to_string())?;
            version.project_id
        }
    };

    if project_id.trim().is_empty() {
        return Err("the dependency names no project".to_string());
    }

    client
        .get_json(&format!("{}/project/{}", api, project_id))
        .map_err(|error| error.to_string())
}

/// Adds or replaces the entry of a mod in the manifest.
fn remember(manifest: &mut Manifest, planned: &Planned) {
    manifest.mods.retain(|entry| entry.slug != planned.slug);
    manifest.mods.push(InstalledMod {
        slug: planned.slug.clone(),
        version: planned.version.clone(),
        filename: planned.filename.clone(),
        sha1: planned.checksum.clone(),
        dependency: planned.required_by.is_some(),
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

/// One jar of the pack that a verification looked at.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub slug: String,
    /// The path of the jar, relative to the mods folder.
    pub path: String,
    pub expected: String,
    /// `None` when the jar is missing.
    pub actual: Option<String>,
}

/// The result of the verification of the pack.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackReport {
    /// The number of jars the manifest lists.
    pub checked: usize,
    pub missing: Vec<PackFile>,
    pub altered: Vec<PackFile>,
}

impl PackReport {
    /// `true` when every jar the manifest lists is in place with the digest of its download.
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.altered.is_empty()
    }
}

/// Recomputes the SHA-1 of every jar the manifest lists.
///
/// The pack is verified from the manifest instead of the API: the manifest is what the launcher
/// actually installed, dependencies included, and it costs no request.
pub fn verify(paths: &Paths) -> PackReport {
    let manifest = load_manifest(paths);
    let mut report = PackReport::default();

    for entry in &manifest.mods {
        let Ok(name) = jar_name(&entry.filename) else {
            continue;
        };
        let Some(expected) = normalized_sha1(&entry.sha1) else {
            continue;
        };

        report.checked += 1;
        match http::sha1(&paths.mods.join(&name)) {
            None => report.missing.push(PackFile {
                slug: entry.slug.clone(),
                path: name,
                expected,
                actual: None,
            }),
            Some(actual) if actual != expected => report.altered.push(PackFile {
                slug: entry.slug.clone(),
                path: name,
                expected,
                actual: Some(actual),
            }),
            Some(_) => {}
        }
    }

    report
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
        assert!(jar_name("C:outside.jar").is_err());
        assert!(jar_name("evil:stream.jar").is_err());
        assert!(jar_name("bad\0name.jar").is_err());
        assert!(jar_name("bad\nname.jar").is_err());
        assert!(jar_name("bad?.jar").is_err());
        assert!(jar_name("NUL.jar").is_err());
        assert!(jar_name("COM1.jar").is_err());
        assert!(jar_name("mod.jar.").is_err());
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
        assert!(report
            .changes
            .iter()
            .any(|change| change.key == "renderDistance"));

        let content = std::fs::read_to_string(&paths.options_file).unwrap();
        assert!(content.contains("renderDistance:8"));
        assert!(content.contains("graphicsPreset:\"custom\""));
        assert!(content.contains("version:4671"), "an unknown line was lost");

        let backup =
            std::fs::read_to_string(paths.options_file.with_file_name(OPTIONS_BACKUP)).unwrap();
        assert_eq!(
            backup, "version:4671\nrenderDistance:16\n",
            "the backup is not the original"
        );

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

    /* ------------------------------------------------------- the pack graph */

    use crate::http::{hex_digest, sha1_digest};
    use crate::test_server::TestServer;

    /// The home of a pack test.
    fn scratch(name: &str) -> (PathBuf, Paths) {
        let root = std::env::temp_dir().join(format!("aether-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        let paths = Paths {
            home: root.clone(),
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
        };

        std::fs::create_dir_all(&paths.mods).unwrap();
        std::fs::create_dir_all(&paths.data).unwrap();
        (root, paths)
    }

    /// The body of a mod of the pack, the bytes the server serves and the test pre-writes.
    fn pack_body(slug: &str) -> Vec<u8> {
        format!("jar of {}", slug).into_bytes()
    }

    /// The name of the jar of a mod of the pack.
    fn pack_filename(slug: &str) -> String {
        format!("{}-1.0.jar", slug)
    }

    /// A Modrinth version document whose download points at the test server.
    fn version_body(
        project: &str,
        filename: &str,
        sha1: &str,
        dependencies: serde_json::Value,
    ) -> Vec<u8> {
        version_body_for(
            project,
            "1.0",
            filename,
            sha1,
            dependencies,
            serde_json::json!(["1.21.11"]),
            serde_json::json!(["fabric"]),
        )
    }

    /// A version document for a pinned version: `/version/{id}` answers an object, not a list.
    fn pinned_body(
        project: &str,
        number: &str,
        filename: &str,
        sha1: &str,
        game_versions: serde_json::Value,
    ) -> Vec<u8> {
        pinned_body_for_loader(
            project,
            number,
            filename,
            sha1,
            game_versions,
            serde_json::json!(["fabric"]),
        )
    }

    /// [`pinned_body`] with the loaders the version announces.
    fn pinned_body_for_loader(
        project: &str,
        number: &str,
        filename: &str,
        sha1: &str,
        game_versions: serde_json::Value,
        loaders: serde_json::Value,
    ) -> Vec<u8> {
        serde_json::to_vec(&version_document(
            project,
            number,
            filename,
            sha1,
            serde_json::json!([]),
            game_versions,
            loaders,
        ))
        .unwrap()
    }

    /// A version listed by the project endpoint, with explicit game versions.
    fn version_body_for(
        project: &str,
        number: &str,
        filename: &str,
        sha1: &str,
        dependencies: serde_json::Value,
        game_versions: serde_json::Value,
        loaders: serde_json::Value,
    ) -> Vec<u8> {
        // The version endpoint answers a list, newest first.
        serde_json::to_vec(&[version_document(
            project,
            number,
            filename,
            sha1,
            dependencies,
            game_versions,
            loaders,
        )])
        .unwrap()
    }

    /// One version document, as both endpoints describe it.
    fn version_document(
        project: &str,
        number: &str,
        filename: &str,
        sha1: &str,
        dependencies: serde_json::Value,
        game_versions: serde_json::Value,
        loaders: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "id": format!("v-{}-{}", project, number),
            "project_id": project,
            "version_number": number,
            "game_versions": game_versions,
            "loaders": loaders,
            "dependencies": dependencies,
            "files": [{
                "url": format!("{{base}}/files/{}", filename),
                "filename": filename,
                "primary": true,
                "hashes": { "sha1": sha1 }
            }]
        })
    }

    /// A server that answers every entry of the pack.
    ///
    /// MoreCulling requires the project of Cloth Config, which the pack already lists: that is the
    /// incident this test keeps from coming back. Sodium declares an optional project, Clumps
    /// declares Krypton incompatible.
    fn pack_server() -> TestServer {
        let mut routes = Vec::new();

        for spec in PACK {
            let body = pack_body(spec.slug);
            let sha1 = hex_digest(&sha1_digest(&body));
            let filename = pack_filename(spec.slug);
            let dependencies = match spec.slug {
                "moreculling" => {
                    serde_json::json!([{ "project_id": "Pcloth", "dependency_type": "required" }])
                }
                "sodium" => {
                    serde_json::json!([{ "project_id": "Piris", "dependency_type": "optional" }])
                }
                "clumps" => {
                    serde_json::json!([{ "project_id": "Pkrypton", "dependency_type": "incompatible" }])
                }
                _ => serde_json::json!([]),
            };

            // The project id of Cloth Config is the one MoreCulling's dependency names, so the
            // pack entry and the dependency are the same project for the graph.
            let project = if spec.slug == "cloth-config" {
                "Pcloth".to_string()
            } else {
                format!("P{}", spec.slug)
            };

            routes.push((
                format!("/project/{}/version", spec.slug),
                version_body(&project, &filename, &sha1, dependencies),
            ));
            routes.push((format!("/files/{}", filename), body));
        }

        routes.push((
            "/project/Pcloth".to_string(),
            br#"{"slug":"cloth-config","title":"Cloth Config API"}"#.to_vec(),
        ));
        routes.push((
            "/project/Piris".to_string(),
            br#"{"slug":"iris","title":"Iris"}"#.to_vec(),
        ));
        routes.push((
            "/project/Pkrypton".to_string(),
            br#"{"slug":"krypton","title":"Krypton"}"#.to_vec(),
        ));

        TestServer::start(routes)
    }

    #[test]
    fn the_pack_is_installed_from_its_dependency_graph_without_a_duplicate() {
        let (root, paths) = scratch("pack-graph");
        let server = pack_server();

        let report = install_from(&paths, "1.21.11", &[], &server.url(""), |_| {});

        assert!(!report.aborted, "{:?}", report.unresolved);
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!(report.installed.len(), PACK.len(), "{:?}", report.installed);

        // Cloth Config is an entry of the pack and a required dependency of MoreCulling: one
        // project, one download.
        assert_eq!(
            server.count("/files/cloth-config-1.0.jar"),
            1,
            "{}",
            server.report()
        );
        assert!(report
            .dependencies
            .iter()
            .any(|dependency| dependency.slug == "cloth-config"
                && dependency.required_by == "moreculling"));

        let manifest = load_manifest(&paths);
        assert_eq!(
            manifest
                .mods
                .iter()
                .filter(|entry| entry.slug == "cloth-config")
                .count(),
            1,
            "the manifest lists Cloth Config twice"
        );

        // One file per project, no project fetched twice.
        assert_eq!(
            server
                .hits()
                .iter()
                .filter(|hit| hit.starts_with("/files/"))
                .count(),
            PACK.len(),
            "{}",
            server.report()
        );

        // An optional dependency is reported and deliberately not installed; an incompatible one
        // is reported without removing the entry of the pack.
        assert!(report
            .notes
            .iter()
            .any(|note| note.slug == "Piris" && note.kind == "optional"));
        assert_eq!(
            server.count("/files/iris-1.0.jar"),
            0,
            "{}",
            server.report()
        );
        assert!(report
            .conflicts
            .iter()
            .any(|conflict| conflict.slug == "clumps" && conflict.with == "Pkrypton"));

        let verified = verify(&paths);
        assert!(verified.is_clean(), "{:?}", verified);
        assert_eq!(verified.checked, PACK.len());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_jar_with_the_wrong_sha1_is_refused_and_never_moved_into_place() {
        let (root, paths) = scratch("pack-sha1");
        let filename = pack_filename("sodium");
        // The API announces the digest of another file: the bytes that arrive do not match.
        let expected = hex_digest(&sha1_digest(b"another jar"));

        let server = TestServer::start(vec![
            (
                "/project/sodium/version".to_string(),
                version_body("Psodium", &filename, &expected, serde_json::json!([])),
            ),
            (
                format!("/files/{}", filename),
                b"the bytes that arrive".to_vec(),
            ),
        ]);

        let report = install_from(
            &paths,
            "1.21.11",
            &["sodium".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(report.installed.is_empty(), "{:?}", report.installed);
        assert_eq!(report.failed.len(), 1, "{:?}", report.failed);
        assert_eq!(report.failed[0].slug, "sodium");
        assert!(
            report.failed[0].error.contains("checksum mismatch"),
            "{}",
            report.failed[0].error
        );
        assert!(
            !paths.mods.join(&filename).exists(),
            "a jar with a wrong digest was moved into place"
        );
        assert!(
            !paths.mods.join("sodium-1.0.part").exists(),
            "the rejected download kept its partial file"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_required_dependency_downloads_nothing_at_all() {
        let (root, paths) = scratch("pack-missing");
        let filename = pack_filename("moreculling");
        let body = pack_body("moreculling");
        let sha1 = hex_digest(&sha1_digest(&body));

        // The dependency names a project the API does not know: the server answers 404.
        let server = TestServer::start(vec![
            (
                "/project/moreculling/version".to_string(),
                version_body(
                    "Pmoreculling",
                    &filename,
                    &sha1,
                    serde_json::json!([{ "project_id": "Pghost", "dependency_type": "required" }]),
                ),
            ),
            (format!("/files/{}", filename), body),
        ]);

        let report = install_from(
            &paths,
            "1.21.11",
            &["moreculling".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(report.aborted, "the incomplete graph was not reported");
        assert!(
            report
                .unresolved
                .iter()
                .any(|entry| entry.required && entry.slug == "Pghost"),
            "{:?}",
            report.unresolved
        );
        assert!(report.installed.is_empty());
        assert_eq!(
            server
                .hits()
                .iter()
                .filter(|hit| hit.starts_with("/files/"))
                .count(),
            0,
            "a broken graph still downloaded a file: {}",
            server.report()
        );
        assert!(!paths.mods.join(&filename).exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_interrupted_pack_installation_resumes_from_its_journal() {
        let (root, paths) = scratch("pack-resume");
        let server = pack_server();

        // The previous run downloaded sodium, verified it, and stopped before lithium.
        std::fs::write(
            paths.mods.join(pack_filename("sodium")),
            pack_body("sodium"),
        )
        .unwrap();
        let mut journal = crate::pending::PendingInstall::new("mods", "1.21.11");
        journal.slugs = vec!["sodium".to_string(), "lithium".to_string()];
        journal.step = "Downloading the pack".to_string();
        journal.completed = 1;
        journal.total = 2;
        crate::pending::begin(&paths, &journal).unwrap();

        let report = install_from(
            &paths,
            "1.21.11",
            &["sodium".to_string(), "lithium".to_string()],
            &server.url(""),
            |_| {},
        );

        assert_eq!(report.skipped, vec!["sodium".to_string()]);
        assert_eq!(report.installed, vec!["lithium".to_string()]);
        assert_eq!(
            server.count(&format!("/files/{}", pack_filename("sodium"))),
            0,
            "a jar that was already verified was downloaded again: {}",
            server.report()
        );
        assert_eq!(
            server.count(&format!("/files/{}", pack_filename("lithium"))),
            1
        );
        assert_eq!(
            std::fs::read(paths.mods.join(pack_filename("lithium"))).unwrap(),
            pack_body("lithium")
        );

        // The run is over and nothing is left half installed.
        assert!(
            crate::pending::read(&paths).is_none(),
            "a finished run left its journal"
        );
        assert!(verify(&paths).is_clean());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_pinned_dependency_is_installed_at_its_exact_version() {
        let (root, paths) = scratch("pack-pinned");

        let moreculling = pack_body("moreculling");
        let moreculling_sha1 = hex_digest(&sha1_digest(&moreculling));
        let old = b"cloth config 1.0".to_vec();
        let old_sha1 = hex_digest(&sha1_digest(&old));
        let newest = b"cloth config 2.0".to_vec();
        let newest_sha1 = hex_digest(&sha1_digest(&newest));

        let server = TestServer::start(vec![
            (
                "/project/moreculling/version".to_string(),
                version_body(
                    "Pmoreculling",
                    &pack_filename("moreculling"),
                    &moreculling_sha1,
                    serde_json::json!([{
                        "project_id": "Pcloth",
                        "version_id": "v-cloth-old",
                        "dependency_type": "required"
                    }]),
                ),
            ),
            (
                format!("/files/{}", pack_filename("moreculling")),
                moreculling,
            ),
            (
                "/project/Pcloth".to_string(),
                br#"{"slug":"cloth-config","title":"Cloth Config API"}"#.to_vec(),
            ),
            // The project also has a newest build: the pin must win over it.
            (
                "/project/cloth-config/version".to_string(),
                version_body(
                    "Pcloth",
                    "cloth-config-2.0.jar",
                    &newest_sha1,
                    serde_json::json!([]),
                ),
            ),
            ("/files/cloth-config-2.0.jar".to_string(), newest),
            (
                "/version/v-cloth-old".to_string(),
                pinned_body(
                    "Pcloth",
                    "1.0",
                    "cloth-config-1.0.jar",
                    &old_sha1,
                    serde_json::json!(["1.21.11"]),
                ),
            ),
            ("/files/cloth-config-1.0.jar".to_string(), old.clone()),
        ]);

        let report = install_from(
            &paths,
            "1.21.11",
            &["moreculling".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert!(!report.aborted, "{:?}", report.unresolved);

        let manifest = load_manifest(&paths);
        let cloth = manifest
            .mods
            .iter()
            .find(|entry| entry.slug == "cloth-config")
            .expect("Cloth Config was not installed");
        assert_eq!(
            cloth.version, "1.0",
            "the newest build was installed instead of the pinned one"
        );
        assert_eq!(cloth.filename, "cloth-config-1.0.jar");
        assert_eq!(
            cloth.sha1, old_sha1,
            "the digest is not the one of the pinned version"
        );
        assert!(paths.mods.join("cloth-config-1.0.jar").exists());
        assert!(!paths.mods.join("cloth-config-2.0.jar").exists());

        assert_eq!(
            server.count("/version/v-cloth-old"),
            1,
            "{}",
            server.report()
        );
        assert_eq!(
            server.count("/files/cloth-config-2.0.jar"),
            0,
            "the newest jar was downloaded despite the pin: {}",
            server.report()
        );
        assert_eq!(
            server.count("/project/cloth-config/version"),
            0,
            "the newest build was resolved despite the pin: {}",
            server.report()
        );

        let dependency = report
            .dependencies
            .iter()
            .find(|entry| entry.slug == "cloth-config")
            .expect("the dependency edge is missing");
        assert!(dependency.pinned);
        assert_eq!(dependency.version, "1.0");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_pin_replaces_the_newest_build_the_pack_would_have_installed() {
        let (root, paths) = scratch("pack-pin-wins");

        let pinned = b"cloth config 1.0".to_vec();
        let pinned_sha1 = hex_digest(&sha1_digest(&pinned));
        let newest = b"cloth config 2.0".to_vec();
        let moreculling = pack_body("moreculling");
        let moreculling_sha1 = hex_digest(&sha1_digest(&moreculling));
        let cloth = pack_body("cloth-config");
        let cloth_sha1 = hex_digest(&sha1_digest(&cloth));

        let server = TestServer::start(vec![
            (
                "/project/cloth-config/version".to_string(),
                version_body(
                    "Pcloth",
                    &pack_filename("cloth-config"),
                    &cloth_sha1,
                    serde_json::json!([]),
                ),
            ),
            (format!("/files/{}", pack_filename("cloth-config")), cloth),
            (
                "/project/moreculling/version".to_string(),
                version_body(
                    "Pmoreculling",
                    &pack_filename("moreculling"),
                    &moreculling_sha1,
                    serde_json::json!([{
                        "project_id": "Pcloth",
                        "version_id": "v-cloth-old",
                        "dependency_type": "required"
                    }]),
                ),
            ),
            (
                format!("/files/{}", pack_filename("moreculling")),
                moreculling,
            ),
            (
                "/project/Pcloth".to_string(),
                br#"{"slug":"cloth-config","title":"Cloth Config API"}"#.to_vec(),
            ),
            (
                "/version/v-cloth-old".to_string(),
                pinned_body(
                    "Pcloth",
                    "1.0",
                    "cloth-config-1.0.jar",
                    &pinned_sha1,
                    serde_json::json!(["1.21.11"]),
                ),
            ),
            ("/files/cloth-config-1.0.jar".to_string(), pinned),
            ("/files/cloth-config-2.0.jar".to_string(), newest),
        ]);

        let report = install_from(
            &paths,
            "1.21.11",
            &["moreculling".to_string(), "cloth-config".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(report.failed.is_empty(), "{:?}", report.failed);

        let manifest = load_manifest(&paths);
        let entries: Vec<&InstalledMod> = manifest
            .mods
            .iter()
            .filter(|entry| entry.slug == "cloth-config")
            .collect();
        assert_eq!(
            entries.len(),
            1,
            "the pack entry and the pin were both kept"
        );
        assert_eq!(
            entries[0].version, "1.0",
            "the newest build of the pack entry won over the pin"
        );
        assert_eq!(entries[0].sha1, pinned_sha1);
        assert_eq!(
            server.count("/files/cloth-config-2.0.jar"),
            0,
            "the newest jar was downloaded anyway: {}",
            server.report()
        );
        assert!(verify(&paths).is_clean());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn two_pinned_versions_of_one_project_are_an_explicit_conflict() {
        let (root, paths) = scratch("pack-conflict");

        let one = b"dep at 1.0".to_vec();
        let one_sha1 = hex_digest(&sha1_digest(&one));
        let two = b"dep at 2.0".to_vec();
        let two_sha1 = hex_digest(&sha1_digest(&two));

        let lithium = pack_body("lithium");
        let lithium_sha1 = hex_digest(&sha1_digest(&lithium));
        let clumps = pack_body("clumps");
        let clumps_sha1 = hex_digest(&sha1_digest(&clumps));

        let server = TestServer::start(vec![
            (
                "/project/lithium/version".to_string(),
                version_body(
                    "Plithium",
                    &pack_filename("lithium"),
                    &lithium_sha1,
                    serde_json::json!([{
                        "project_id": "Pdep",
                        "version_id": "v-dep-one",
                        "dependency_type": "required"
                    }]),
                ),
            ),
            (format!("/files/{}", pack_filename("lithium")), lithium),
            (
                "/project/clumps/version".to_string(),
                version_body(
                    "Pclumps",
                    &pack_filename("clumps"),
                    &clumps_sha1,
                    serde_json::json!([{
                        "project_id": "Pdep",
                        "version_id": "v-dep-two",
                        "dependency_type": "required"
                    }]),
                ),
            ),
            (format!("/files/{}", pack_filename("clumps")), clumps),
            (
                "/project/Pdep".to_string(),
                br#"{"slug":"dep-mod","title":"Dep Mod"}"#.to_vec(),
            ),
            (
                "/version/v-dep-one".to_string(),
                pinned_body(
                    "Pdep",
                    "1.0",
                    "dep-1.0.jar",
                    &one_sha1,
                    serde_json::json!(["1.21.11"]),
                ),
            ),
            (
                "/version/v-dep-two".to_string(),
                pinned_body(
                    "Pdep",
                    "2.0",
                    "dep-2.0.jar",
                    &two_sha1,
                    serde_json::json!(["1.21.11"]),
                ),
            ),
            ("/files/dep-1.0.jar".to_string(), one),
            ("/files/dep-2.0.jar".to_string(), two),
        ]);

        let report = install_from(
            &paths,
            "1.21.11",
            &["lithium".to_string(), "clumps".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!(
            report
                .conflicts
                .iter()
                .filter(|conflict| conflict.with == "dep-mod")
                .count(),
            1,
            "the contradiction was not reported: {:?}",
            report.conflicts
        );

        // One version of the project is kept, never both.
        let manifest = load_manifest(&paths);
        let entries: Vec<&InstalledMod> = manifest
            .mods
            .iter()
            .filter(|entry| entry.slug == "dep-mod")
            .collect();
        assert_eq!(
            entries.len(),
            1,
            "two versions of one project were installed"
        );
        assert_eq!(entries[0].version, "1.0", "the first pin was not kept");
        assert_eq!(
            server.count("/files/dep-2.0.jar"),
            0,
            "the second pinned jar was installed too: {}",
            server.report()
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn two_mods_that_pin_the_same_version_download_it_once() {
        let (root, paths) = scratch("pack-same-pin");

        let dep = b"dep at 1.0".to_vec();
        let dep_sha1 = hex_digest(&sha1_digest(&dep));
        let lithium = pack_body("lithium");
        let lithium_sha1 = hex_digest(&sha1_digest(&lithium));
        let clumps = pack_body("clumps");
        let clumps_sha1 = hex_digest(&sha1_digest(&clumps));

        let mut routes = Vec::new();
        for (slug, body, sha1) in [
            ("lithium", lithium, lithium_sha1),
            ("clumps", clumps, clumps_sha1),
        ] {
            routes.push((
                format!("/project/{}/version", slug),
                version_body(
                    &format!("P{}", slug),
                    &pack_filename(slug),
                    &sha1,
                    serde_json::json!([{
                        "project_id": "Pdep",
                        "version_id": "v-dep-one",
                        "dependency_type": "required"
                    }]),
                ),
            ));
            routes.push((format!("/files/{}", pack_filename(slug)), body));
        }
        routes.push((
            "/project/Pdep".to_string(),
            br#"{"slug":"dep-mod","title":"Dep Mod"}"#.to_vec(),
        ));
        routes.push((
            "/version/v-dep-one".to_string(),
            pinned_body(
                "Pdep",
                "1.0",
                "dep-1.0.jar",
                &dep_sha1,
                serde_json::json!(["1.21.11"]),
            ),
        ));
        routes.push(("/files/dep-1.0.jar".to_string(), dep));

        let server = TestServer::start(routes);
        let report = install_from(
            &paths,
            "1.21.11",
            &["lithium".to_string(), "clumps".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert!(
            report.conflicts.is_empty(),
            "the same pin was reported as a conflict: {:?}",
            report.conflicts
        );
        assert_eq!(
            server.count("/files/dep-1.0.jar"),
            1,
            "the shared dependency was downloaded twice: {}",
            server.report()
        );

        let manifest = load_manifest(&paths);
        assert_eq!(
            manifest
                .mods
                .iter()
                .filter(|entry| entry.slug == "dep-mod")
                .count(),
            1
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_pinned_version_published_for_another_loader_aborts_the_graph() {
        let (root, paths) = scratch("pack-loader");

        let moreculling = pack_body("moreculling");
        let moreculling_sha1 = hex_digest(&sha1_digest(&moreculling));
        let forge = b"cloth config for forge".to_vec();
        let forge_sha1 = hex_digest(&sha1_digest(&forge));

        let server = TestServer::start(vec![
            (
                "/project/moreculling/version".to_string(),
                version_body(
                    "Pmoreculling",
                    &pack_filename("moreculling"),
                    &moreculling_sha1,
                    serde_json::json!([{
                        "project_id": "Pcloth",
                        "version_id": "v-cloth-forge",
                        "dependency_type": "required"
                    }]),
                ),
            ),
            (
                format!("/files/{}", pack_filename("moreculling")),
                moreculling,
            ),
            (
                "/project/Pcloth".to_string(),
                br#"{"slug":"cloth-config","title":"Cloth Config API"}"#.to_vec(),
            ),
            (
                "/version/v-cloth-forge".to_string(),
                pinned_body_for_loader(
                    "Pcloth",
                    "1.0",
                    "cloth-config-1.0.jar",
                    &forge_sha1,
                    serde_json::json!(["1.21.11"]),
                    serde_json::json!(["forge"]),
                ),
            ),
            ("/files/cloth-config-1.0.jar".to_string(), forge),
        ]);

        let report = install_from(
            &paths,
            "1.21.11",
            &["moreculling".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(
            report.aborted,
            "a dependency published for another loader was accepted"
        );
        assert!(
            report
                .unresolved
                .iter()
                .any(|entry| entry.required && entry.error.contains("not published for fabric")),
            "{:?}",
            report.unresolved
        );
        assert!(report.installed.is_empty());
        assert_eq!(
            server
                .hits()
                .iter()
                .filter(|hit| hit.starts_with("/files/"))
                .count(),
            0,
            "a file was downloaded despite the aborted graph: {}",
            server.report()
        );
        assert!(!paths.mods.join("cloth-config-1.0.jar").exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_newest_build_published_for_another_loader_is_refused() {
        let (root, paths) = scratch("pack-loader-newest");

        let moreculling = pack_body("moreculling");
        let moreculling_sha1 = hex_digest(&sha1_digest(&moreculling));

        let server = TestServer::start(vec![
            (
                "/project/moreculling/version".to_string(),
                version_body_for(
                    "Pmoreculling",
                    "1.0",
                    &pack_filename("moreculling"),
                    &moreculling_sha1,
                    serde_json::json!([]),
                    serde_json::json!(["1.21.11"]),
                    serde_json::json!(["forge"]),
                ),
            ),
            (
                format!("/files/{}", pack_filename("moreculling")),
                moreculling,
            ),
        ]);

        let report = install_from(
            &paths,
            "1.21.11",
            &["moreculling".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(
            report.aborted,
            "a build published for another loader was accepted"
        );
        assert!(
            report
                .unresolved
                .iter()
                .any(|entry| entry.required && entry.error.contains("not published for fabric")),
            "{:?}",
            report.unresolved
        );
        assert!(report.installed.is_empty());
        assert_eq!(
            server
                .hits()
                .iter()
                .filter(|hit| hit.starts_with("/files/"))
                .count(),
            0,
            "a file was downloaded despite the aborted graph: {}",
            server.report()
        );
        assert!(!paths.mods.join(pack_filename("moreculling")).exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_dependency_that_does_not_support_the_pack_version_aborts_the_graph() {
        let (root, paths) = scratch("pack-game-version");

        let moreculling = pack_body("moreculling");
        let moreculling_sha1 = hex_digest(&sha1_digest(&moreculling));
        let old = b"cloth config for 1.20.1".to_vec();
        let old_sha1 = hex_digest(&sha1_digest(&old));

        let server = TestServer::start(vec![
            (
                "/project/moreculling/version".to_string(),
                version_body(
                    "Pmoreculling",
                    &pack_filename("moreculling"),
                    &moreculling_sha1,
                    serde_json::json!([{
                        "project_id": "Pcloth",
                        "version_id": "v-cloth-old",
                        "dependency_type": "required"
                    }]),
                ),
            ),
            (
                format!("/files/{}", pack_filename("moreculling")),
                moreculling,
            ),
            (
                "/project/Pcloth".to_string(),
                br#"{"slug":"cloth-config","title":"Cloth Config API"}"#.to_vec(),
            ),
            (
                "/version/v-cloth-old".to_string(),
                pinned_body(
                    "Pcloth",
                    "1.0",
                    "cloth-config-1.0.jar",
                    &old_sha1,
                    serde_json::json!(["1.20.1"]),
                ),
            ),
            ("/files/cloth-config-1.0.jar".to_string(), old),
        ]);

        let report = install_from(
            &paths,
            "1.21.11",
            &["moreculling".to_string()],
            &server.url(""),
            |_| {},
        );

        assert!(
            report.aborted,
            "a dependency published for another Minecraft was accepted"
        );
        assert!(
            report
                .unresolved
                .iter()
                .any(|entry| entry.required && entry.error.contains("1.21.11")),
            "{:?}",
            report.unresolved
        );
        assert!(report.installed.is_empty());
        assert_eq!(
            server
                .hits()
                .iter()
                .filter(|hit| hit.starts_with("/files/"))
                .count(),
            0,
            "a file was downloaded despite the aborted graph: {}",
            server.report()
        );
        assert!(!paths.mods.join("cloth-config-1.0.jar").exists());

        std::fs::remove_dir_all(&root).ok();
    }
}
