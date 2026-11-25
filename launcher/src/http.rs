//! The HTTP layer of the launcher.
//!
//! Every download is written to a temporary file, verified with a streaming sha1 and only moved
//! into place once the checksum matched, an interrupted or corrupted download therefore never
//! replaces a good file.
//!
//! A download that was interrupted continues where it stopped: the `.part` file is the source of
//! truth of the bytes that are already acquired and the transfer resumes with a `Range` request.
//! A server that ignores the range (or answers `200` instead of `206`) makes the download start
//! over from zero, the partial file is truncated and never concatenated with the new body.
//!
//! Transient failures (I/O errors, `408`, `425`, `429` and `5xx`) are retried with an exponential
//! backoff, so a lost connection does not abort a whole installation.
//!
//! An installation builds a single [`Client`] for the whole session: the agent keeps the pool of
//! connections and the timeouts, every request of the session reuses them.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;

/// The user agent the launcher identifies itself with.
pub const USER_AGENT: &str = "AetherLauncher/1.0 (+https://github.com/aether-client)";

/// How many times a request or a download is attempted before it fails.
pub const MAX_ATTEMPTS: usize = 3;
/// The delay before the first retry, it is doubled on every further attempt.
pub const RETRY_BASE_DELAY: Duration = Duration::from_millis(250);
/// The longest delay between two attempts.
pub const RETRY_MAX_DELAY: Duration = Duration::from_secs(4);
/// The number of bytes of an error body that are kept.
const MAX_ERROR_BODY_BYTES: usize = 2048;
/// The size of the chunks that are read and hashed at once.
const CHUNK_SIZE: usize = 64 * 1024;

/// The error type of the HTTP layer.
#[derive(Debug)]
pub enum HttpError {
    /// The server answered with an error code.
    Status {
        url: String,
        status: u16,
        /// The body of the answer, when the server sent one.
        body: Option<String>,
    },
    /// The answer could not be read.
    Io(std::io::Error),
    /// The answer was not valid JSON.
    Json(String),
    /// The checksum of a file did not match.
    Checksum {
        path: String,
        expected: String,
        actual: String,
    },
    /// The player cancelled the download in the middle of a file.
    Cancelled,
    /// A URI could not be built.
    Uri(String),
}

/// @return true when a status code is worth another attempt.
fn retryable_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429) || status >= 500
}

impl HttpError {
    /// @return true when the request is worth another attempt.
    ///
    /// Rate limits, timeouts, server errors, I/O errors and corrupted downloads are transient, a
    /// malformed answer or an invalid URL is not.
    pub fn is_retryable(&self) -> bool {
        match self {
            HttpError::Status { status, .. } => retryable_status(*status),
            HttpError::Io(_) => true,
            HttpError::Checksum { .. } => true,
            // A cancelled download is not retried: the player asked it to stop.
            HttpError::Cancelled => false,
            HttpError::Json(_) | HttpError::Uri(_) => false,
        }
    }

    /// @return the body of the answer, when the server sent one.
    pub fn body(&self) -> Option<&str> {
        match self {
            HttpError::Status { body, .. } => body.as_deref(),
            _ => None,
        }
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::Status { url, status, .. } => {
                write!(formatter, "GET {} failed with HTTP {}", url, status)
            }
            HttpError::Io(error) => write!(formatter, "{}", error),
            HttpError::Json(error) => write!(formatter, "invalid JSON: {}", error),
            HttpError::Checksum {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "checksum mismatch for {} (expected {}, got {})",
                path, expected, actual
            ),
            HttpError::Cancelled => write!(formatter, "the download was cancelled"),
            HttpError::Uri(url) => write!(formatter, "invalid url: {}", url),
        }
    }
}

impl std::error::Error for HttpError {}

impl From<std::io::Error> for HttpError {
    fn from(error: std::io::Error) -> Self {
        HttpError::Io(error)
    }
}

impl From<serde_json::Error> for HttpError {
    fn from(error: serde_json::Error) -> Self {
        HttpError::Json(error.to_string())
    }
}

/// Builds the agent of a session.
///
/// The timeouts are the same for every request of the session, the pool of connections is kept by
/// the agent itself.
fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(USER_AGENT)
        // Preserve HTTP error response bodies for OAuth and server diagnostics.
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(config)
}

/// The delay before the attempt that follows the given failure.
fn backoff_delay(retry: u32) -> Duration {
    let factor = 1u32 << retry.min(5);
    RETRY_BASE_DELAY.saturating_mul(factor).min(RETRY_MAX_DELAY)
}

/// Converts an error of `ureq` into the error type of the launcher.
///
/// The body of an error answer is kept, the device code flow of Microsoft has to read the OAuth
/// error code from it.
fn map_error(url: &str, error: ureq::Error) -> HttpError {
    match error {
        ureq::Error::StatusCode(status) => HttpError::Status {
            url: url.to_string(),
            status,
            body: None,
        },
        other => HttpError::Io(std::io::Error::other(other.to_string())),
    }
}

/// Converts a non-success response while keeping only a bounded diagnostic body.
fn check_status(
    url: &str,
    response: &mut ureq::http::Response<ureq::Body>,
) -> Result<(), HttpError> {
    let status = response.status().as_u16();
    if status < 400 {
        return Ok(());
    }

    let body = read_error_body(response.body_mut().as_reader());
    Err(HttpError::Status {
        url: url.to_string(),
        status,
        body,
    })
}

/// Reads at most [`MAX_ERROR_BODY_BYTES`] from an error response.
///
/// Truncating after `into_string()` still allocates the complete remote body. Bound the read
/// itself so a server error cannot make the launcher allocate an arbitrary amount of memory.
fn read_error_body(reader: impl Read) -> Option<String> {
    let mut bytes = Vec::with_capacity(MAX_ERROR_BODY_BYTES);
    reader
        .take(MAX_ERROR_BODY_BYTES as u64)
        .read_to_end(&mut bytes)
        .ok()?;

    let text = String::from_utf8_lossy(&bytes);
    Some(text.trim().to_string())
}

/// Runs a request and retries it while the failure is transient.
fn with_retry<T>(mut attempt: impl FnMut() -> Result<T, HttpError>) -> Result<T, HttpError> {
    let mut last_error: Option<HttpError> = None;

    for retry in 0..MAX_ATTEMPTS {
        if retry > 0 {
            std::thread::sleep(backoff_delay(retry as u32));
        }

        match attempt() {
            Ok(value) => return Ok(value),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }
                last_error = Some(error);
            }
        }
    }

    Err(last_error.expect("at least one attempt was made"))
}

/// The number of HTTP clients that were built since the process started.
///
/// An installation builds a single one for the whole session, the counter is what proves it.
static CLIENTS_BUILT: AtomicUsize = AtomicUsize::new(0);

/// The number of HTTP clients that were built since the process started.
pub fn clients_built() -> usize {
    CLIENTS_BUILT.load(Ordering::Relaxed)
}

/// What a download did, the installer writes it to the log of the installation.
#[derive(Clone, Debug, Default)]
pub struct DownloadReport {
    /// The size of the file once it is complete.
    pub bytes: u64,
    /// The bytes that were already on the disk and were not fetched again.
    pub resumed_from: u64,
    /// The bytes that were fetched during this session.
    pub fetched: u64,
    /// How long the transfer took.
    pub elapsed: Duration,
}

impl DownloadReport {
    /// The number of bytes that were fetched per second.
    pub fn bytes_per_second(&self) -> f64 {
        let seconds = self.elapsed.as_secs_f64();

        if seconds <= 0.0 {
            0.0
        } else {
            self.fetched as f64 / seconds
        }
    }
}

/// A session of HTTP requests.
///
/// The agent keeps the pool of connections and the timeouts of the session. An installation builds
/// one client and passes it around, instead of paying a handshake for every file.
pub struct Client {
    agent: ureq::Agent,
}

impl Default for Client {
    fn default() -> Client {
        Client::new()
    }
}

impl Client {
    /// Opens a session.
    pub fn new() -> Client {
        CLIENTS_BUILT.fetch_add(1, Ordering::Relaxed);
        Client { agent: agent() }
    }
    /// Downloads a JSON document.
    pub fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, HttpError> {
        self.get_json_with_header(url, "", "")
    }

    /// Downloads JSON with an optional request header.
    pub fn get_json_with_header<T: DeserializeOwned>(
        &self,
        url: &str,
        header: &str,
        value: &str,
    ) -> Result<T, HttpError> {
        let agent = &self.agent;
        with_retry(|| {
            let request = agent.get(url);
            let request = if header.is_empty() {
                request
            } else {
                request.header(header, value)
            };
            let mut response = request.call().map_err(|error| map_error(url, error))?;
            check_status(url, &mut response)?;
            response
                .body_mut()
                .read_json()
                .map_err(|error| map_error(url, error))
        })
    }

    /// Reads a document into a string.
    pub fn get_text(&self, url: &str) -> Result<String, HttpError> {
        let agent = &self.agent;
        with_retry(|| {
            let mut response = agent.get(url).call().map_err(|error| map_error(url, error))?;
            check_status(url, &mut response)?;
            response
                .body_mut()
                .read_to_string()
                .map_err(|error| map_error(url, error))
        })
    }

    /// Reads a document into a byte buffer.
    pub fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
        let agent = &self.agent;
        with_retry(|| {
            let mut response = agent.get(url).call().map_err(|error| map_error(url, error))?;
            check_status(url, &mut response)?;
            let mut buffer = Vec::new();
            response.body_mut().as_reader().read_to_end(&mut buffer)?;
            Ok(buffer)
        })
    }

    /// Posts a form and reads the JSON answer.
    pub fn post_form<T: DeserializeOwned>(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<T, HttpError> {
        let agent = &self.agent;
        with_retry(|| {
            let mut response = agent
                .post(url)
                .send_form(form.iter().copied())
                .map_err(|error| map_error(url, error))?;
            check_status(url, &mut response)?;
            response
                .body_mut()
                .read_json()
                .map_err(|error| map_error(url, error))
        })
    }

    /// Posts a JSON document and reads the JSON answer.
    pub fn post_json<T: DeserializeOwned>(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<T, HttpError> {
        let agent = &self.agent;
        with_retry(|| {
            let mut response = agent
                .post(url)
                .header("content-type", "application/json")
                .send_json(body)
                .map_err(|error| map_error(url, error))?;
            check_status(url, &mut response)?;
            response
                .body_mut()
                .read_json()
                .map_err(|error| map_error(url, error))
        })
    }

    /// Downloads a file, renews its checksum and resumes an interrupted transfer.
    ///
    /// The body is written to `<file>.part`, hashed while it is written, and only moved into place
    /// once the checksum matched. When a `.part` file is already there it is the source of truth of
    /// the bytes that are acquired: the transfer continues with a `Range` request from its size, so
    /// a lost connection only costs the bytes that were not sent yet.
    ///
    /// A server that ignores the range and answers with the whole file makes the download start
    /// over: the partial file is truncated and its bytes are never concatenated with the new body.
    /// A bad checksum discards the partial file, the next attempt fetches it entirely again.
    pub fn download(
        &self,
        url: &str,
        destination: &Path,
        expected_sha1: Option<&str>,
        on_progress: impl FnMut(u64, u64),
    ) -> Result<DownloadReport, HttpError> {
        self.download_cancellable(url, destination, expected_sha1, None, on_progress)
    }

    /// [`Client::download`], with a token the player can set to stop in the middle of a file.
    ///
    /// The token is read after every chunk: a large jar stops within one chunk instead of running
    /// to its end after the interface said “Cancel”. The partial file is kept, so a cancelled
    /// installation resumes from the bytes it already has.
    pub fn download_cancellable(
        &self,
        url: &str,
        destination: &Path,
        expected_sha1: Option<&str>,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        mut on_progress: impl FnMut(u64, u64),
    ) -> Result<DownloadReport, HttpError> {
        // A file that already carries the expected checksum is not downloaded again.
        if let Some(expected) = expected_sha1 {
            if sha1(destination).as_deref() == Some(expected) {
                let bytes = fs::metadata(destination)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);

                return Ok(DownloadReport {
                    bytes,
                    resumed_from: bytes,
                    fetched: 0,
                    elapsed: Duration::ZERO,
                });
            }
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        let temporary = destination.with_extension("part");
        let mut last_error: Option<HttpError> = None;

        for retry in 0..MAX_ATTEMPTS {
            if retry > 0 {
                std::thread::sleep(backoff_delay(retry as u32));
            }

            match self.download_once(
                url,
                destination,
                &temporary,
                expected_sha1,
                cancel,
                &mut on_progress,
            ) {
                Ok(report) => return Ok(report),
                Err(error) => {
                    // A cancellation keeps the partial file: it is exactly what a resumed
                    // installation continues from.
                    if matches!(error, HttpError::Cancelled) {
                        return Err(error);
                    }

                    // A checksum failure or a permanent error means the bytes are worthless. A
                    // network failure keeps the partial file: it is the point where the next
                    // attempt, or the next installation, resumes.
                    if !error.is_retryable() || matches!(error, HttpError::Checksum { .. }) {
                        let _ = fs::remove_file(&temporary);
                    }

                    if !error.is_retryable() {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }

        Err(last_error.expect("at least one attempt was made"))
    }

    /// Sends the request of a download, with a range when a partial file is already there.
    fn request(
        &self,
        url: &str,
        offset: u64,
    ) -> Result<ureq::http::Response<ureq::Body>, HttpError> {
        let request = self.agent.get(url);
        let request = if offset > 0 {
            request.header("range", &format!("bytes={}-", offset))
        } else {
            request
        };

        let mut response = request.call().map_err(|error| map_error(url, error))?;
        check_status(url, &mut response)?;
        Ok(response)
    }

    /// A single attempt of [`Client::download`].
    fn download_once(
        &self,
        url: &str,
        destination: &Path,
        temporary: &Path,
        expected_sha1: Option<&str>,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        on_progress: &mut impl FnMut(u64, u64),
    ) -> Result<DownloadReport, HttpError> {
        let started = Instant::now();
        let mut offset = fs::metadata(temporary)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        let mut response = match self.request(url, offset) {
            Ok(response) => response,
            // The offset is beyond the end of the file the server knows, the download starts over.
            Err(HttpError::Status { status: 416, .. }) if offset > 0 => {
                let _ = fs::remove_file(temporary);
                offset = 0;
                self.request(url, 0)?
            }
            Err(error) => return Err(error),
        };

        let mut resumed = offset > 0 && response.status() == 206;

        // A resume appends the bytes that follow the ones on the disk, so the range the server
        // announces has to start exactly where the request asked. Anything else, a missing or
        // unreadable header included, would mix two different parts of the file.
        let shifted = resumed && content_range_start(&response) != Some(offset);

        if shifted {
            // The answer is not the continuation of the partial file, the whole file is asked for.
            let _ = fs::remove_file(temporary);
            offset = 0;
            response = self.request(url, 0)?;
            resumed = false;
        } else if offset > 0 && !resumed {
            // The server ignored the range and sent the whole file from its first byte: that body is
            // the one that is needed once the partial file is truncated, it is not requested again.
            let _ = fs::remove_file(temporary);
            offset = 0;
        }

        let length = response
            .header("content-length")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);

        let total = if resumed {
            offset.saturating_add(length)
        } else {
            length
        };

        let mut hasher = expected_sha1.map(|_| sha1::Sha1::new());

        if resumed {
            if let Some(hasher) = hasher.as_mut() {
                // The bytes that are already there are part of the checksum of the final file.
                hash_prefix(temporary, offset, hasher)?;
            }
        }

        let mut file = if resumed {
            fs::OpenOptions::new().append(true).open(temporary)?
        } else {
            fs::File::create(temporary)?
        };

        let mut reader = response.body_mut().as_reader();
        let mut buffer = [0u8; CHUNK_SIZE];
        let mut received = offset;

        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }

            file.write_all(&buffer[..read])?;
            if let Some(hasher) = hasher.as_mut() {
                hasher.update(&buffer[..read]);
            }

            received += read as u64;
            on_progress(received, total);

            // The token is read after every chunk, so a large jar stops within one chunk instead
            // of finishing after the interface said “Cancel”.
            if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                file.flush()?;
                drop(file);
                return Err(HttpError::Cancelled);
            }
        }

        file.flush()?;
        drop(file);

        // A body that stops before the announced length is a cut, the file is not complete.
        if total > 0 && received < total {
            return Err(HttpError::Io(std::io::Error::other(format!(
                "the transfer stopped at {} of {} bytes",
                received, total
            ))));
        }

        if let (Some(expected), Some(hasher)) = (expected_sha1, hasher) {
            let actual = hex_digest(&hasher.finalize());
            if actual != expected {
                return Err(HttpError::Checksum {
                    path: destination.display().to_string(),
                    expected: expected.to_string(),
                    actual,
                });
            }
        }

        fs::rename(temporary, destination)?;

        Ok(DownloadReport {
            bytes: received,
            resumed_from: offset,
            fetched: received.saturating_sub(offset),
            elapsed: started.elapsed(),
        })
    }
}

/// Downloads a JSON document with a client that only lives for the call.
pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, HttpError> {
    Client::new().get_json(url)
}

/// Reads a document into a string with a client that only lives for the call.
pub fn get_text(url: &str) -> Result<String, HttpError> {
    Client::new().get_text(url)
}

/// Reads a document into a byte buffer with a client that only lives for the call.
pub fn get_bytes(url: &str) -> Result<Vec<u8>, HttpError> {
    Client::new().get_bytes(url)
}

/// Reads a document with its status code, without turning a `4xx` answer into an error.
///
/// Most endpoints of the launcher treat an error code as a failure. A name lookup is different:
/// Mojang answers `204` for a name nobody owns, and that is a useful answer, not a failure. A
/// retryable code (rate limit, server error) is still raised so the retry loop can run.
pub fn get_with_status(url: &str) -> Result<(u16, String), HttpError> {
    let agent = agent();

    with_retry(|| {
        let failure = |status: u16, body: String| HttpError::Status {
            url: url.to_string(),
            status,
            body: Some(body),
        };

        let mut response = agent.get(url).call().map_err(|error| map_error(url, error))?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| map_error(url, error))?;

        if retryable_status(status) {
            return Err(failure(status, body));
        }

        Ok((status, body))
    })
}

/// Posts a form with a client that only lives for the call.
pub fn post_form<T: DeserializeOwned>(url: &str, form: &[(&str, &str)]) -> Result<T, HttpError> {
    Client::new().post_form(url, form)
}

/// Posts a JSON document with a client that only lives for the call.
pub fn post_json<T: DeserializeOwned>(url: &str, body: &serde_json::Value) -> Result<T, HttpError> {
    Client::new().post_json(url, body)
}

/// Downloads a file with a client that only lives for the call.
pub fn download(
    url: &str,
    destination: &Path,
    expected_sha1: Option<&str>,
    on_progress: impl FnMut(u64, u64),
) -> Result<DownloadReport, HttpError> {
    Client::new().download(url, destination, expected_sha1, on_progress)
}

/// Reads the first byte of the range a `206` answer announces.
///
/// `Content-Range: bytes 512-1023/1024` starts at 512. A header that is missing or that does not
/// have that shape yields `None`: a resume never trusts a range it cannot read.
fn content_range_start(
    response: &ureq::http::Response<ureq::Body>,
) -> Option<u64> {
    response
        .headers()
        .get("content-range")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_range)
}

}

/// Reads the first byte of a `Content-Range` value.
fn parse_content_range(value: &str) -> Option<u64> {
    let lowered = value.trim().to_ascii_lowercase();
    let rest = lowered.strip_prefix("bytes")?.trim_start();
    let span = rest.split('/').next()?.trim();
    let start = span.split('-').next()?.trim();

    start.parse().ok()
}

/// Feeds the first bytes of a file to a hasher.
///
/// The prefix of a resumed download is part of the checksum of the complete file, it is read again
/// from the disk instead of being trusted.
fn hash_prefix(path: &Path, length: u64, hasher: &mut sha1::Sha1) -> Result<(), HttpError> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0u8; CHUNK_SIZE];
    let mut remaining = length;

    while remaining > 0 {
        let wanted = buffer.len().min(remaining as usize);
        let read = file.read(&mut buffer[..wanted])?;

        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("{} is shorter than its partial file", path.display()),
            )
            .into());
        }

        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }

    Ok(())
}

/// The sha1 of a file, `None` when the file does not exist or cannot be read.
///
/// The file is read in chunks, a large asset never has to be loaded into memory at once.
pub fn sha1(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = sha1::Sha1::new();
    let mut buffer = [0u8; CHUNK_SIZE];

    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Some(hex_digest(&hasher.finalize()))
}

/// The sha1 of a byte buffer.
pub fn sha1_digest(bytes: &[u8]) -> [u8; 20] {
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    hasher.finalize()
}

/// A minimal sha1 implementation, the launcher only needs it for the checksums of Mojang.
mod sha1 {
    pub struct Sha1 {
        state: [u32; 5],
        buffer: [u8; 64],
        buffered: usize,
        length: u64,
    }

    impl Sha1 {
        pub fn new() -> Sha1 {
            Sha1 {
                state: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0],
                buffer: [0; 64],
                buffered: 0,
                length: 0,
            }
        }

        pub fn update(&mut self, data: &[u8]) {
            self.length += data.len() as u64;

            let mut offset = 0;

            // The block that is still incomplete comes first.
            if self.buffered > 0 {
                let take = (64 - self.buffered).min(data.len());
                self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
                self.buffered += take;
                offset = take;

                if self.buffered == 64 {
                    let block = self.buffer;
                    self.process(&block);
                    self.buffered = 0;
                }
            }

            // Every complete block is processed directly from the input.
            while offset + 64 <= data.len() {
                let mut block = [0u8; 64];
                block.copy_from_slice(&data[offset..offset + 64]);
                self.process(&block);
                offset += 64;
            }

            // What is left over is kept for the next call.
            if offset < data.len() {
                let rest = data.len() - offset;
                self.buffer[..rest].copy_from_slice(&data[offset..]);
                self.buffered = rest;
            }
        }

        pub fn finalize(mut self) -> [u8; 20] {
            let bit_length = self.length * 8;

            self.buffer[self.buffered] = 0x80;
            self.buffered += 1;

            if self.buffered > 56 {
                for index in self.buffered..64 {
                    self.buffer[index] = 0;
                }
                let block = self.buffer;
                self.process(&block);
                self.buffered = 0;
            }

            for index in self.buffered..56 {
                self.buffer[index] = 0;
            }
            self.buffer[56..64].copy_from_slice(&bit_length.to_be_bytes());
            let block = self.buffer;
            self.process(&block);

            let mut result = [0u8; 20];
            for (index, word) in self.state.iter().enumerate() {
                result[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
            }
            result
        }

        fn process(&mut self, block: &[u8; 64]) {
            let mut words = [0u32; 80];
            for index in 0..16 {
                words[index] = u32::from_be_bytes([
                    block[index * 4],
                    block[index * 4 + 1],
                    block[index * 4 + 2],
                    block[index * 4 + 3],
                ]);
            }
            for index in 16..80 {
                words[index] =
                    (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                        .rotate_left(1);
            }

            let [mut a, mut b, mut c, mut d, mut e] = self.state;

            for (index, word) in words.iter().enumerate() {
                let (f, k) = match index {
                    0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                    20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                    _ => (b ^ c ^ d, 0xCA62C1D6),
                };

                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(*word);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
            }

            self.state[0] = self.state[0].wrapping_add(a);
            self.state[1] = self.state[1].wrapping_add(b);
            self.state[2] = self.state[2].wrapping_add(c);
            self.state[3] = self.state[3].wrapping_add(d);
            self.state[4] = self.state[4].wrapping_add(e);
        }
    }
}

/// Formats a byte buffer as a hexadecimal string.
pub fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backoff_grows_and_is_capped() {
        assert_eq!(backoff_delay(1), Duration::from_millis(500));
        assert_eq!(backoff_delay(2), Duration::from_millis(1000));
        assert!(backoff_delay(9) <= RETRY_MAX_DELAY);
    }

    #[test]
    fn transient_failures_are_retryable() {
        let status = |status| HttpError::Status {
            url: "https://example.invalid".to_string(),
            status,
            body: None,
        };

        assert!(status(429).is_retryable());
        assert!(status(503).is_retryable());
        assert!(!status(404).is_retryable());
        assert!(!status(400).is_retryable());
        assert!(!HttpError::Json("bad".to_string()).is_retryable());
    }

    #[test]
    fn a_retry_repeats_a_transient_failure() {
        let mut attempts = 0;
        let value = with_retry(|| {
            attempts += 1;
            if attempts < 2 {
                Err(HttpError::Status {
                    url: "https://example.invalid".to_string(),
                    status: 503,
                    body: None,
                })
            } else {
                Ok(42)
            }
        })
        .unwrap();

        assert_eq!(value, 42);
        assert_eq!(attempts, 2);
    }

    #[test]
    fn a_permanent_failure_is_not_repeated() {
        let mut attempts = 0;
        let result = with_retry::<()>(|| {
            attempts += 1;
            Err(HttpError::Status {
                url: "https://example.invalid".to_string(),
                status: 404,
                body: None,
            })
        });

        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }

    #[test]
    fn the_start_of_a_content_range_is_read() {
        assert_eq!(parse_content_range("bytes 512-1023/1024"), Some(512));
        assert_eq!(parse_content_range("Bytes 0-0/1"), Some(0));
        assert_eq!(
            parse_content_range("  bytes 131072-262143/262144 "),
            Some(131072)
        );
        assert_eq!(parse_content_range("bytes */1024"), None);
        assert_eq!(parse_content_range("later"), None);
        assert_eq!(parse_content_range(""), None);
    }

    #[test]
    fn the_error_body_is_kept() {
        let error = HttpError::Status {
            url: "https://example.invalid".to_string(),
            status: 400,
            body: Some("{\"error\":\"authorization_pending\"}".to_string()),
        };

        assert_eq!(error.body(), Some("{\"error\":\"authorization_pending\"}"));
    }

    #[test]
    fn an_error_body_is_bounded_while_reading() {
        let body = vec![b'x'; MAX_ERROR_BODY_BYTES * 8];
        let text = read_error_body(body.as_slice()).expect("the body is readable");

        assert_eq!(text.len(), MAX_ERROR_BODY_BYTES);
    }

    #[test]
    fn a_cancelled_download_stops_within_one_chunk_and_keeps_its_partial_file() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let address = listener.local_addr().expect("the address of the listener");
        let body = vec![0x7bu8; CHUNK_SIZE * 3];
        let length = body.len();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);

        // Every attempt of the client finds the listener, and the loop ends when the test says so:
        // a single `accept` would leave the join blocked when no retry comes.
        let server = std::thread::spawn(move || {
            let _ = listener.set_nonblocking(true);

            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                        let mut buffer = [0u8; 4096];
                        let _ = stream.read(&mut buffer);

                        let header = format!(
                            "HTTP/1.1 200 OK
Content-Length: {}
Connection: close

",
                            length
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(&body);
                        let _ = stream.flush();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => return,
                }
            }
        });

        // A unique directory: a reused process id must not inherit a partial file of an old run.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let directory =
            std::env::temp_dir().join(format!("aether-cancel-{}-{}", std::process::id(), stamp));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let destination = directory.join("big.jar");
        let cancel = Arc::new(AtomicBool::new(false));
        let chunks = Arc::new(AtomicUsize::new(0));
        let flag = Arc::clone(&cancel);
        let seen = Arc::clone(&chunks);

        let result = Client::new().download_cancellable(
            &format!("http://{}/big.jar", address),
            &destination,
            None,
            Some(&cancel),
            |_, _| {
                // The player presses Cancel while the first chunk is being written.
                if seen.fetch_add(1, Ordering::Relaxed) == 0 {
                    flag.store(true, Ordering::Relaxed);
                }
            },
        );

        stop.store(true, Ordering::Relaxed);
        server.join().unwrap();

        assert!(
            matches!(result, Err(HttpError::Cancelled)),
            "the cancellation was not reported: {:?}",
            result.err()
        );
        assert_eq!(
            chunks.load(Ordering::Relaxed),
            1,
            "the download kept reading chunks after the cancel"
        );
        assert!(
            !destination.exists(),
            "a cancelled download was moved into place"
        );

        // The bytes are checked once the file system shows them: a freshly closed file can take a
        // moment under load, which is not the behaviour this test is about.
        let partial = destination.with_extension("part");
        let deadline = Instant::now() + Duration::from_secs(5);
        let written = loop {
            let written = std::fs::metadata(&partial)
                .map(|metadata| metadata.len())
                .unwrap_or(0);

            if written > 0 || Instant::now() >= deadline {
                break written;
            }

            std::thread::sleep(Duration::from_millis(10));
        };

        assert!(written > 0, "the bytes already fetched were thrown away");
        assert!(
            written < length as u64,
            "the cancelled download ran to the end of the file"
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_corrupted_download_is_fetched_again_and_only_the_verified_bytes_are_kept() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let address = listener.local_addr().expect("the address of the listener");
        let good = b"the jar that is announced".to_vec();
        let served = good.clone();

        let server = std::thread::spawn(move || {
            // The first answer is a body that does not match its digest, the second is the file.
            for attempt in 0..2 {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };

                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);

                let payload = if attempt == 0 {
                    b"a corrupted body".to_vec()
                } else {
                    served.clone()
                };

                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&payload);
                let _ = stream.flush();
            }
        });

        let directory =
            std::env::temp_dir().join(format!("aether-redownload-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let destination = directory.join("mod.jar");
        let expected = hex_digest(&sha1_digest(&good));

        let report = download(
            &format!("http://{}/mod.jar", address),
            &destination,
            Some(&expected),
            |_, _| {},
        )
        .expect("the corrupted body was replaced by the verified one");

        server.join().unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), good);
        assert_eq!(report.bytes, good.len() as u64);
        assert!(
            !destination.with_extension("part").exists(),
            "the partial file of the rejected body was kept"
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_download_is_retried_after_a_server_error() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let address = listener.local_addr().expect("the address of the listener");
        let body = b"the retried body".to_vec();
        let served = body.clone();

        let server = std::thread::spawn(move || {
            // The first answer is a server error, the second one carries the file.
            for attempt in 0..2 {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };

                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);

                let (status, payload) = if attempt == 0 {
                    ("HTTP/1.1 503 Service Unavailable", Vec::new())
                } else {
                    ("HTTP/1.1 200 OK", served.clone())
                };

                let header = format!(
                    "{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    status,
                    payload.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&payload);
                let _ = stream.flush();
            }
        });

        let directory =
            std::env::temp_dir().join(format!("aether-download-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let destination = directory.join("file.bin");
        let expected = hex_digest(&sha1_digest(&body));

        download(
            &format!("http://{}/file.bin", address),
            &destination,
            Some(&expected),
            |_, _| {},
        )
        .expect("the download was retried and then verified");

        server.join().unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), body);
        assert_eq!(sha1(&destination).as_deref(), Some(expected.as_str()));

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_failed_download_keeps_a_file_that_has_no_checksum() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let address = listener.local_addr().expect("the address of the listener");

        let server = std::thread::spawn(move || {
            for _ in 0..MAX_ATTEMPTS {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                let _ = stream.flush();
            }
        });

        let directory = std::env::temp_dir().join(format!("aether-keep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let destination = directory.join("fabric-api.jar");
        std::fs::write(&destination, b"a good jar").unwrap();

        let result = download(
            &format!("http://{}/fabric-api.jar", address),
            &destination,
            None,
            |_, _| {},
        );

        server.join().unwrap();

        assert!(result.is_err(), "the failure was not reported");
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"a good jar",
            "the file of the player was deleted"
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_failed_checked_download_preserves_the_previous_file() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let address = listener.local_addr().expect("the address of the listener");

        let server = std::thread::spawn(move || {
            for _ in 0..MAX_ATTEMPTS {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                let _ = stream.flush();
            }
        });

        let directory =
            std::env::temp_dir().join(format!("aether-keep-checksum-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let destination = directory.join("client.jar");
        let previous = b"previous verified client";
        std::fs::write(&destination, previous).unwrap();
        let expected = hex_digest(&sha1_digest(b"new client"));

        let result = download(
            &format!("http://{}/client.jar", address),
            &destination,
            Some(&expected),
            |_, _| {},
        );

        server.join().unwrap();

        assert!(
            result.is_err(),
            "the failed update was reported as successful"
        );
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            previous,
            "the existing client was removed before the replacement was verified"
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn the_file_sha1_is_computed_in_chunks() {
        let directory = std::env::temp_dir().join(format!("aether-sha1-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("data.bin");

        // Larger than a single chunk so that the streaming path is exercised.
        let data = vec![0x61u8; CHUNK_SIZE * 2 + 17];
        std::fs::write(&file, &data).unwrap();

        assert_eq!(
            sha1(&file),
            Some(hex_digest(&sha1_digest(&data))),
            "the streaming sha1 differs from the buffered one"
        );

        std::fs::remove_dir_all(&directory).ok();
    }
}
