//! The installation of a version of Minecraft.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::http::{Client, DownloadReport, HttpError};
use crate::logs;
use crate::minecraft::{
    asset_path, asset_url_from, fetch_fabric_profile, fetch_version_json, merge_version_json,
    resolve_libraries, VersionJson, RESOURCE_HOST,
};
use crate::paths::Paths;

/// The number of assets that are installed at the same time.
///
/// The assets are the largest part of an installation (thousands of small files), downloading them
/// one by one wastes most of the bandwidth. The bound keeps a slow connection from being swamped.
pub const ASSET_CONCURRENCY: usize = 8;

/// The progress of the installation.
#[derive(Clone, Debug, Default, Serialize)]
pub struct InstallProgress {
    /// A human readable description of the current step.
    pub task: String,
    /// The bytes that were downloaded for the current step.
    pub received: u64,
    /// The total number of bytes of the current step.
    pub total: u64,
    /// The number of the finished steps.
    pub completed: usize,
    /// The number of the steps.
    pub total_tasks: usize,
}

/// The error type of the installation.
#[derive(Debug)]
pub enum InstallError {
    /// The player asked for the installation to stop.
    Cancelled,
    /// The version does not provide a client.
    MissingClient(String),
    /// A file could not be written.
    Io(io::Error),
    /// A download failed.
    Http(HttpError),
    /// The jar of the client is missing.
    MissingMod(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Cancelled => write!(formatter, "the installation was cancelled"),
            InstallError::MissingClient(version) => {
                write!(formatter, "version {} does not provide a client download", version)
            }
            InstallError::Io(error) => write!(formatter, "{}", error),
            InstallError::Http(error) => write!(formatter, "{}", error),
            InstallError::MissingMod(path) => write!(formatter, "the Aether client jar was not found at {}", path),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> Self {
        InstallError::Io(error)
    }
}

impl From<HttpError> for InstallError {
    fn from(error: HttpError) -> Self {
        InstallError::Http(error)
    }
}

/// The description of the asset index.
#[derive(Clone, Debug, Deserialize)]
struct AssetIndex {
    #[serde(default)]
    objects: std::collections::HashMap<String, AssetObject>,
}

/// A single asset.
#[derive(Clone, Debug, Deserialize)]
struct AssetObject {
    hash: String,
    #[serde(default)]
    size: u64,
}

/// The result of an installation.
pub struct Installed {
    pub version: VersionJson,
    pub client_jar: PathBuf,
    pub fabric_api_jar: PathBuf,
    pub mod_jar: PathBuf,
    pub natives_dir: PathBuf,
}

/// Installs a version of Minecraft together with the Fabric loader and the client.
pub fn install_version(
    game_version: &str,
    loader_version: &str,
    fabric_api_version: &str,
    mod_jar: &Path,
    paths: &Paths,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(&InstallProgress),
) -> Result<Installed, InstallError> {
    // A cancelled installation does not touch the network at all.
    check_cancelled(cancel)?;

    let total_tasks = 5;
    let mut completed = 0usize;

    let mut report = |task: &str, received: u64, total: u64, completed: usize| {
        on_progress(&InstallProgress {
            task: task.to_string(),
            received,
            total,
            completed,
            total_tasks,
        });
    };

    report("Resolving version metadata", 0, 0, completed);

    // A single session for the whole installation: the pool of connections and the timeouts are
    // shared by every file instead of paying a handshake for each of them.
    let client = Client::new();

    let vanilla = fetch_version_json(game_version)?;
    let fabric = fetch_fabric_profile(game_version, loader_version)?;
    let version = merge_version_json(&vanilla, &fabric);
    check_cancelled(cancel)?;

    let client_download = vanilla
        .downloads
        .client
        .clone()
        .ok_or_else(|| InstallError::MissingClient(game_version.to_string()))?;

    let client_jar = paths
        .versions
        .join(game_version)
        .join(format!("{}.jar", game_version));

    // --- the client ------------------------------------------------------------------
    report(
        "Downloading Minecraft client",
        0,
        client_download.size,
        completed,
    );
    let metrics = client.download(
        &client_download.url,
        &client_jar,
        Some(&client_download.sha1),
        |received, total| report("Downloading Minecraft client", received, total, completed),
    )?;
    log_download(
        paths,
        &format!("versions/{}/{}.jar", game_version, game_version),
        &metrics,
    );
    completed += 1;

    // --- the libraries ----------------------------------------------------------------
    report("Downloading libraries", 0, 0, completed);
    let libraries = resolve_libraries(&version, &crate::minecraft::detect_os());
    for library in &libraries {
        check_cancelled(cancel)?;

        let destination = paths.libraries.join(&library.path);
        let metrics = client.download(
            &library.url,
            &destination,
            library.sha1.as_deref(),
            |received, total| report("Downloading libraries", received, total, completed),
        )?;
        log_download(paths, &library.path, &metrics);
    }
    completed += 1;

    // --- the assets ---------------------------------------------------------------------
    report("Downloading assets", 0, 0, completed);
    install_assets(&vanilla, paths, &client, cancel, |received, total| {
        report("Downloading assets", received, total, completed)
    })?;
    completed += 1;

    // --- the Fabric API -----------------------------------------------------------------
    report("Downloading Fabric API", 0, 0, completed);
    check_cancelled(cancel)?;
    let fabric_api_jar = paths
        .mods
        .join(format!("fabric-api-{}.jar", fabric_api_version));
    let fabric_api_url = format!(
        "https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/{}/fabric-api-{}.jar",
        fabric_api_version, fabric_api_version
    );
    // Maven publishes the digest of every artifact next to it, the file is not trusted without it.
    let fabric_api_sha1 = client
        .get_text(&format!("{}.sha1", fabric_api_url))
        .ok()
        .as_deref()
        .and_then(published_sha1);
    let metrics = client.download(
        &fabric_api_url,
        &fabric_api_jar,
        fabric_api_sha1.as_deref(),
        |received, total| report("Downloading Fabric API", received, total, completed),
    )?;
    log_download(
        paths,
        &format!("mods/fabric-api-{}.jar", fabric_api_version),
        &metrics,
    );
    completed += 1;

    // --- the natives ---------------------------------------------------------------------
    report("Preparing natives", 0, 0, completed);
    check_cancelled(cancel)?;
    let natives_dir = paths.versions.join(game_version).join("natives");
    install_natives(&libraries, paths, &client, cancel, &natives_dir)?;
    completed += 1;

    // --- the description of the version ---------------------------------------------------
    let version_dir = paths.versions.join(game_version);
    fs::create_dir_all(&version_dir)?;
    let description = serde_json::to_string_pretty(&vanilla)
        .map_err(|error| InstallError::Io(io::Error::other(error.to_string())))?;
    fs::write(version_dir.join(format!("{}.json", game_version)), description)?;

    // --- the mod --------------------------------------------------------------------------
    check_cancelled(cancel)?;
    if !mod_jar.exists() {
        return Err(InstallError::MissingMod(mod_jar.display().to_string()));
    }
    fs::create_dir_all(&paths.mods)?;
    let target = paths.mods.join("aether-client.jar");
    fs::copy(mod_jar, &target)?;

    report("Ready", 0, 0, completed);

    Ok(Installed {
        version,
        client_jar,
        fabric_api_jar,
        mod_jar: target,
        natives_dir,
    })
}

/// Stops the installation when the player asked for it.
fn check_cancelled(cancel: &AtomicBool) -> Result<(), InstallError> {
    if cancel.load(Ordering::Relaxed) {
        Err(InstallError::Cancelled)
    } else {
        Ok(())
    }
}

/// Downloads every asset of a version.
fn install_assets(
    vanilla: &VersionJson,
    paths: &Paths,
    client: &Client,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), InstallError> {
    check_cancelled(cancel)?;

    let index_id = match &vanilla.assets {
        Some(id) => id.clone(),
        None => return Ok(()),
    };

    let index_file = paths.asset_indexes.join(format!("{}.json", index_id));
    let index = match read_asset_index(&index_file) {
        Some(index) => index,
        None => {
            // A broken index is replaced instead of being trusted: an empty index installs no
            // asset at all and the game only fails much later, with an unrelated message.
            let index_download = vanilla
                .asset_index
                .as_ref()
                .ok_or_else(|| InstallError::MissingClient(index_id.clone()))?;
            let _ = fs::remove_file(&index_file);
            let metrics = client.download(
                &index_download.url,
                &index_file,
                Some(&index_download.sha1),
                |_, _| {},
            )?;
            log_download(paths, &format!("assets/indexes/{}.json", index_id), &metrics);
            read_asset_index(&index_file).ok_or_else(|| {
                InstallError::Io(io::Error::other(format!(
                    "the index {} of the assets could not be read",
                    index_id
                )))
            })?
        }
    };

    let objects: Vec<AssetObject> = index.objects.into_values().collect();
    let total: u64 = objects.iter().map(|object| object.size).sum();

    // The files that are already there are not downloaded again.
    let mut received = 0u64;
    let mut pending: Vec<AssetObject> = Vec::new();
    for object in objects {
        let destination = paths.assets.join(asset_path(&object.hash));
        let present = fs::metadata(&destination)
            .map(|metadata| metadata.len() == object.size)
            .unwrap_or(false);

        if present {
            received += object.size;
        } else {
            pending.push(object);
        }
    }

    if pending.is_empty() {
        on_progress(total, total);
        return Ok(());
    }

    on_progress(received, total);
    download_assets(
        &pending,
        paths,
        client,
        RESOURCE_HOST,
        total,
        received,
        cancel,
        on_progress,
    )?;

    Ok(())
}

/// Reads the index of the assets, `None` when the file is missing or broken.
fn read_asset_index(path: &Path) -> Option<AssetIndex> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Reads the sha1 that Maven publishes next to an artifact.
///
/// The document only carries the digest, sometimes followed by the name of the file.
fn published_sha1(text: &str) -> Option<String> {
    let value = text.split_whitespace().next()?.to_ascii_lowercase();

    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(value)
    } else {
        None
    }
}

/// Downloads the missing assets with a bounded number of workers.
///
/// The workers only download, they never touch the progress callback: every chunk is reported
/// through a channel and the caller aggregates it, so the progress stays monotonic even when a
/// worker restarts a download.
#[allow(clippy::too_many_arguments)]
fn download_assets(
    pending: &[AssetObject],
    paths: &Paths,
    client: &Client,
    host: &str,
    total: u64,
    already_received: u64,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), InstallError> {
    check_cancelled(cancel)?;

    let workers = ASSET_CONCURRENCY.min(pending.len()).max(1);

    let next = AtomicUsize::new(0);
    let running = AtomicUsize::new(workers);
    let failure: Mutex<Option<InstallError>> = Mutex::new(None);
    let (sender, receiver) = mpsc::channel::<u64>();
    let received = std::cell::Cell::new(already_received);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let sender = sender.clone();
            let next = &next;
            let running = &running;
            let failure = &failure;

            scope.spawn(move || {
                loop {
                    if failure.lock().map(|guard| guard.is_some()).unwrap_or(true) {
                        break;
                    }

                    // The assets are the bulk of an installation: the token is read before every
                    // file, a cancelled installation stops within one asset.
                    if cancel.load(Ordering::Relaxed) {
                        if let Ok(mut guard) = failure.lock() {
                            if guard.is_none() {
                                *guard = Some(InstallError::Cancelled);
                            }
                        }
                        break;
                    }

                    let position = next.fetch_add(1, Ordering::Relaxed);
                    let Some(object) = pending.get(position) else {
                        break;
                    };

                    let destination = paths.assets.join(asset_path(&object.hash));
                    let mut reported = 0u64;

                    match client.download(
                        &asset_url_from(host, &object.hash),
                        &destination,
                        Some(object.hash.as_str()),
                        |file_received, _| {
                            // A download that is retried starts at zero again, only the bytes that
                            // were not counted yet are forwarded.
                            let delta = file_received.saturating_sub(reported);
                            if delta > 0 {
                                reported = file_received;
                                let _ = sender.send(delta);
                            }
                        },
                    ) {
                        Ok(metrics) => log_download(paths, &asset_path(&object.hash), &metrics),
                        Err(error) => {
                            if let Ok(mut guard) = failure.lock() {
                                if guard.is_none() {
                                    *guard = Some(InstallError::from(error));
                                }
                            }
                            break;
                        }
                    }
                }

                running.fetch_sub(1, Ordering::Relaxed);
            });
        }

        drop(sender);

        loop {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(delta) => {
                    let value = received.get().saturating_add(delta).min(total);
                    received.set(value);
                    on_progress(value, total);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if running.load(Ordering::Relaxed) == 0 {
                        // Read what is still queued before the workers are joined.
                        while let Ok(delta) = receiver.try_recv() {
                            let value = received.get().saturating_add(delta).min(total);
                            received.set(value);
                            on_progress(value, total);
                        }
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    match failure.into_inner() {
        Ok(Some(error)) => Err(error),
        Ok(None) => Ok(()),
        Err(poisoned) => Err(poisoned.into_inner().unwrap_or_else(|| {
            InstallError::Io(io::Error::other(
                "the asset installer stopped unexpectedly",
            ))
        })),
    }
}

/// Downloads and extracts the native libraries.
fn install_natives(
    libraries: &[crate::minecraft::ResolvedLibrary],
    paths: &Paths,
    client: &Client,
    cancel: &AtomicBool,
    natives_dir: &Path,
) -> Result<(), InstallError> {
    fs::create_dir_all(natives_dir)?;

    for library in libraries.iter().filter(|entry| entry.native) {
        check_cancelled(cancel)?;
        let destination = paths.libraries.join(&library.path);
        let metrics = client.download(
            &library.url,
            &destination,
            library.sha1.as_deref(),
            |_, _| {},
        )?;
        log_download(paths, &library.path, &metrics);

        let bytes = fs::read(&destination)?;
        extract_zip(&bytes, natives_dir)?;
    }

    Ok(())
}

/// Extracts a zip archive.
fn extract_zip(bytes: &[u8], destination: &Path) -> Result<(), InstallError> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|error| {
        InstallError::Io(io::Error::other(format!("not a valid zip archive: {}", error)))
    })?;

    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if entry.is_dir() {
            continue;
        }

        let name = entry.name().to_string();
        let target = destination.join(&name);

        // Protection against the zip slip vulnerability.
        if !target.starts_with(destination) {
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut output = fs::File::create(&target)?;
        io::copy(&mut entry, &mut output)?;
    }

    Ok(())
}

/// How many bytes of log are written between two checks of the file size.
///
/// A full installation writes thousands of lines under eight workers; asking the disk for the
/// size of the file on every line costs a `metadata()` each time and widens the window where two
/// workers trim at once. The size is only asked once per interval, so the file stays bounded by
/// `MAX_INSTALL_LOG_BYTES + TRIM_CHECK_BYTES` instead of `MAX_INSTALL_LOG_BYTES` alone.
const TRIM_CHECK_BYTES: u64 = 64 * 1024;

/// The bytes appended to the installation log since the last size check, all workers together.
static LOG_BYTES_SINCE_CHECK: AtomicU64 = AtomicU64::new(0);

/// Appends the metrics of a download to the log of the installation.
///
/// The file is opened in append mode, several asset workers may write at the same time.
fn log_download(paths: &Paths, name: &str, report: &DownloadReport) {
    use std::io::Write;

    fs::create_dir_all(&paths.logs).ok();

    let path = logs::install_log(paths);
    let line = format_download(name, report);

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(line.as_bytes());
    }

    // A full installation writes thousands of lines and every repair adds more: the file is cut
    // back to its newest half once it passes its limit, the oldest lines are the ones that go.
    // The check rides on a counter of the bytes written, not on a `metadata()` per line. The
    // worker that wins the exchange is the only one to trim for this interval.
    let written = line.len() as u64;
    let current = LOG_BYTES_SINCE_CHECK.fetch_add(written, Ordering::Relaxed) + written;
    if current >= TRIM_CHECK_BYTES
        && LOG_BYTES_SINCE_CHECK
            .compare_exchange(
                current,
                current - TRIM_CHECK_BYTES,
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok()
    {
        logs::trim(
            &path,
            logs::MAX_INSTALL_LOG_BYTES,
            logs::MAX_INSTALL_LOG_BYTES / 2,
        )
        .ok();
    }
}

/// The line that the log of the installation receives for a download.
fn format_download(name: &str, report: &DownloadReport) -> String {
    format!(
        "download {} — {} bytes ({} fetched, {} resumed) in {} ms, {:.1} KiB/s\n",
        name,
        report.bytes,
        report.fetched,
        report.resumed_from,
        report.elapsed.as_millis(),
        report.bytes_per_second() / 1024.0
    )
}

/// Reads the description of a version from the disk.
pub fn read_version_json(paths: &Paths, version: &str) -> Option<VersionJson> {
    let content = fs::read_to_string(
        paths
            .versions
            .join(version)
            .join(format!("{}.json", version)),
    )
    .ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{hex_digest, sha1_digest};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Builds a set of paths that live in a temporary directory.
    fn test_paths(root: &Path) -> Paths {
        let game = root.join(".minecraft");

        Paths {
            home: root.to_path_buf(),
            data: root.join("aether"),
            assets: game.join("assets"),
            asset_indexes: game.join("assets").join("indexes"),
            asset_objects: game.join("assets").join("objects"),
            libraries: game.join("libraries"),
            versions: game.join("versions"),
            mods: game.join("mods"),
            settings_file: root.join("aether").join("settings.json"),
            accounts_file: root.join("aether").join("accounts.json"),
            servers_file: root.join("aether").join("servers.json"),
            server_root: root.join("servers"),
            options_file: game.join("options.txt"),
            logs: root.join("aether").join("logs"),
            game,
        }
    }

    /// Serves one file per connection from a loopback port.
    fn serve(files: Vec<(String, Vec<u8>)>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let address = listener.local_addr().expect("the address of the listener");

        let handle = std::thread::spawn(move || {
            for _ in 0..files.len() {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };

                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                let request = String::from_utf8_lossy(&buffer).to_string();
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();

                let body = files
                    .iter()
                    .find(|(hash, _)| path.ends_with(hash.as_str()))
                    .map(|(_, body)| body.clone())
                    .unwrap_or_default();

                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });

        (format!("http://{}", address), handle)
    }

    /// The log tests share one counter of written bytes, they run one at a time and start from
    /// zero so the moment of a size check is known exactly.
    static LOG_WRITER_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn the_metrics_of_a_download_reach_the_log() {
        let _guard = LOG_WRITER_LOCK.lock().unwrap();
        LOG_BYTES_SINCE_CHECK.store(0, Ordering::Relaxed);

        let root = std::env::temp_dir().join(format!("aether-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = test_paths(&root);
        fs::create_dir_all(&paths.logs).unwrap();

        let report = DownloadReport {
            bytes: 4096,
            resumed_from: 1024,
            fetched: 3072,
            elapsed: std::time::Duration::from_millis(50),
        };

        log_download(&paths, "libraries/foo/bar.jar", &report);
        log_download(&paths, "libraries/foo/baz.jar", &report);

        let content = fs::read_to_string(paths.logs.join("install.log")).unwrap();
        assert!(content.contains("libraries/foo/bar.jar"), "{}", content);
        assert!(content.contains("libraries/foo/baz.jar"), "{}", content);
        assert!(content.contains("3072 fetched"), "{}", content);
        assert!(content.contains("1024 resumed"), "{}", content);
        assert!(content.contains("KiB/s"), "{}", content);

        // The throughput is the fetched bytes over the time of the transfer.
        let expected = 3072.0 / 0.050 / 1024.0;
        assert!(
            content.contains(&format!("{:.1} KiB/s", expected)),
            "{}",
            content
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_log_of_the_installations_is_bounded_by_the_real_writer() {
        let _guard = LOG_WRITER_LOCK.lock().unwrap();
        LOG_BYTES_SINCE_CHECK.store(0, Ordering::Relaxed);

        let root = std::env::temp_dir().join(format!("aether-log-bound-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = test_paths(&root);
        fs::create_dir_all(&paths.logs).unwrap();

        let report = DownloadReport {
            bytes: 4096,
            resumed_from: 0,
            fetched: 4096,
            elapsed: std::time::Duration::from_millis(10),
        };
        let padding = "x".repeat(120);

        // A line is about 200 bytes long: the writer itself trims the file while it grows, the
        // limit of the constant is not lowered for the test.
        for index in 0..4_000 {
            log_download(
                &paths,
                &format!("assets/o/{:04}/{}", index, padding),
                &report,
            );
        }

        let file = paths.logs.join(logs::INSTALL_LOG);
        let size = fs::metadata(&file).unwrap().len();
        // The size is checked once per TRIM_CHECK_BYTES written, so the file can pass the limit
        // by at most one interval before the next check cuts it back.
        assert!(
            size <= logs::MAX_INSTALL_LOG_BYTES + TRIM_CHECK_BYTES,
            "the log grew to {} bytes",
            size
        );
        assert!(
            size > logs::MAX_INSTALL_LOG_BYTES / 2,
            "the log was cut too far: {} bytes",
            size
        );

        let content = fs::read_to_string(&file).unwrap();
        assert!(
            content.contains("assets/o/3999/"),
            "the newest line is kept"
        );
        assert!(
            !content.contains("assets/o/0000/"),
            "the oldest lines are gone"
        );
        assert!(
            content.ends_with('\n'),
            "the log does not end in the middle of a line"
        );
        assert!(
            !paths.logs.join("install.log.tmp").exists(),
            "a temporary file was left behind"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_size_of_the_log_is_not_asked_on_every_line() {
        let _guard = LOG_WRITER_LOCK.lock().unwrap();
        LOG_BYTES_SINCE_CHECK.store(0, Ordering::Relaxed);

        let root = std::env::temp_dir().join(format!("aether-log-frequency-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = test_paths(&root);
        fs::create_dir_all(&paths.logs).unwrap();

        let report = DownloadReport {
            bytes: 4096,
            resumed_from: 0,
            fetched: 4096,
            elapsed: std::time::Duration::from_millis(10),
        };

        // A file already over the limit: the old writer cut it on the very next line.
        let file = paths.logs.join(logs::INSTALL_LOG);
        let mut over = vec![b'x'; (logs::MAX_INSTALL_LOG_BYTES + 1024) as usize];
        over.push(b'\n');
        fs::write(&file, &over).unwrap();

        log_download(&paths, "libraries/foo/small.jar", &report);
        assert!(
            fs::metadata(&file).unwrap().len() > logs::MAX_INSTALL_LOG_BYTES,
            "a single line triggered a size check"
        );

        // Once an interval of bytes is written the check happens, and the file is cut back to
        // its newest half even though it was already over the limit before the first line.
        let padding = "x".repeat(120);
        for index in 0..400 {
            log_download(
                &paths,
                &format!("assets/o/{:04}/{}", index, padding),
                &report,
            );
        }

        let size = fs::metadata(&file).unwrap().len();
        assert!(
            size <= logs::MAX_INSTALL_LOG_BYTES / 2 + TRIM_CHECK_BYTES,
            "the log was not cut: {} bytes",
            size
        );
        assert!(
            fs::read_to_string(&file).unwrap().contains("assets/o/0399/"),
            "the newest lines are kept"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_installation_that_is_cancelled_before_it_starts_does_nothing() {
        let root = std::env::temp_dir().join(format!("aether-cancel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = test_paths(&root);
        let cancel = AtomicBool::new(true);

        // The token is read before anything else: no network, no file.
        let error = install_version(
            "1.21.11",
            "0.20.1",
            "0.141.6+1.21.11",
            &root.join("aether-client.jar"),
            &paths,
            &cancel,
            |_| {},
        )
        .err()
        .expect("the installation was cancelled");

        assert!(matches!(error, InstallError::Cancelled), "{:?}", error);
        assert!(!paths.versions.exists(), "a cancelled installation wrote files");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_assets_stop_when_the_installation_is_cancelled() {
        let root = std::env::temp_dir().join(format!("aether-cancel-assets-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = test_paths(&root);
        fs::create_dir_all(&paths.assets).unwrap();

        let objects = vec![AssetObject {
            hash: "0".repeat(40),
            size: 16,
        }];
        let cancel = AtomicBool::new(true);

        // The address never answers: a cancelled installation must not even connect.
        let error = download_assets(
            &objects,
            &paths,
            &Client::new(),
            "http://127.0.0.1:1",
            16,
            0,
            &cancel,
            |_, _| {},
        )
        .expect_err("the assets were cancelled");

        assert!(matches!(error, InstallError::Cancelled), "{:?}", error);
        assert!(!paths.assets.join(asset_path(&objects[0].hash)).exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_broken_asset_index_is_not_trusted() {
        let root = std::env::temp_dir().join(format!("aether-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let file = root.join("29.json");

        fs::write(&file, "{\"objects\": {\"a\": {\"hash\":").unwrap();
        assert!(read_asset_index(&file).is_none(), "a truncated index was accepted");

        fs::write(&file, "{}").unwrap();
        assert!(read_asset_index(&file).is_some(), "a valid index was refused");

        fs::remove_file(&file).unwrap();
        assert!(read_asset_index(&file).is_none(), "a missing index was accepted");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_sha1_of_maven_is_read() {
        let digest = "0123456789abcdef0123456789abcdef01234567";

        assert_eq!(published_sha1(digest).as_deref(), Some(digest));
        assert_eq!(
            published_sha1(&format!("  {}  fabric-api.jar\n", digest.to_uppercase())).as_deref(),
            Some(digest)
        );
        assert!(published_sha1("<html>404</html>").is_none());
        assert!(published_sha1("0123456").is_none());
    }

    #[test]
    fn the_assets_are_downloaded_by_several_workers() {
        let root = std::env::temp_dir().join(format!("aether-assets-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = test_paths(&root);
        fs::create_dir_all(&paths.assets).unwrap();

        let first = vec![b'a'; 1024];
        let second = vec![b'b'; 4096];
        let objects = vec![
            AssetObject {
                hash: hex_digest(&sha1_digest(&first)),
                size: first.len() as u64,
            },
            AssetObject {
                hash: hex_digest(&sha1_digest(&second)),
                size: second.len() as u64,
            },
        ];

        let files = vec![
            (objects[0].hash.clone(), first.clone()),
            (objects[1].hash.clone(), second.clone()),
        ];
        let (host, server) = serve(files);

        let total: u64 = objects.iter().map(|object| object.size).sum();
        let mut highest = 0u64;
        let mut reports = 0usize;

        download_assets(
            &objects,
            &paths,
            &Client::new(),
            &host,
            total,
            0,
            &AtomicBool::new(false),
            |received, reported_total| {
            assert_eq!(reported_total, total);
            assert!(
                received >= highest,
                "the progress went backwards ({} after {})",
                received,
                highest
            );
                highest = received;
                reports += 1;
            },
        )
        .expect("the assets were downloaded");

        server.join().expect("the server stopped");

        assert_eq!(
            fs::read(paths.assets.join(asset_path(&objects[0].hash))).unwrap(),
            first
        );
        assert_eq!(
            fs::read(paths.assets.join(asset_path(&objects[1].hash))).unwrap(),
            second
        );
        assert_eq!(highest, total, "the progress never reached the total");
        assert!(reports > 0);

        fs::remove_dir_all(&root).ok();
    }
}
