//! The interruption of a download, proven against a local HTTP server.
//!
//! Every test counts the bytes the server really sent, a resume that downloads the file again would
//! show up in those numbers. The tests share a mutex so that the counter of the HTTP clients stays
//! meaningful.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use aether_launcher::http::{clients_built, hex_digest, sha1_digest, Client};

/// The tests of this file run one after the other.
static SERIAL: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What the server does with the connections it receives.
#[derive(Clone, Copy, Default)]
struct Behaviour {
    /// Closes the first connection after this many bytes of the body.
    cut_after: Option<usize>,
    /// Answers with the whole file even when a range is requested.
    ignore_range: bool,
    /// Announces and serves a range that starts somewhere else than the requested one.
    shifted_start: Option<usize>,
}

struct Server {
    url: String,
    served: Arc<AtomicU64>,
    ranges: Arc<Mutex<Vec<Option<u64>>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Server {
    fn start(body: Vec<u8>, behaviour: Behaviour) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let address = listener.local_addr().expect("the address of the listener");
        listener
            .set_nonblocking(true)
            .expect("a listener that does not block");

        let served = Arc::new(AtomicU64::new(0));
        let ranges = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));

        let handle = {
            let served = Arc::clone(&served);
            let ranges = Arc::clone(&ranges);
            let stop = Arc::clone(&stop);

            std::thread::spawn(move || {
                let mut first = true;

                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            answer(stream, &body, behaviour, first, &served, &ranges);
                            first = false;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            })
        };

        Server {
            url: format!("http://{}", address),
            served,
            ranges,
            stop,
            handle: Some(handle),
        }
    }

    fn ranges(&self) -> Vec<Option<u64>> {
        self.ranges
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn served(&self) -> u64 {
        self.served.load(Ordering::Relaxed)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Answers one request, with the range it asks for unless the behaviour ignores it.
fn answer(
    mut stream: TcpStream,
    body: &[u8],
    behaviour: Behaviour,
    first: bool,
    served: &AtomicU64,
    ranges: &Mutex<Vec<Option<u64>>>,
) {
    // TCP is a byte stream: one read is not guaranteed to contain all request
    // headers. Read until the header terminator so platform-specific packet
    // segmentation cannot hide a later header such as Range.
    let mut request = Vec::with_capacity(1024);
    let mut buffer = [0u8; 1024];

    while request.len() < 8192 {
        let available = (8192 - request.len()).min(buffer.len());
        match stream.read(&mut buffer[..available]) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\\r\\n\\r\\n") {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        }
    }

    let request = String::from_utf8_lossy(&request).to_string();

    let requested = range_start(&request);
    ranges
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(requested);

    let offset = match requested {
        Some(start) if !behaviour.ignore_range => start.min(body.len() as u64) as usize,
        _ => 0,
    };
    // A broken server may announce a different range than the one that was requested.
    let offset = match behaviour.shifted_start {
        Some(shift) if offset > 0 => shift.min(body.len()),
        _ => offset,
    };
    let remaining = &body[offset..];

    let header = if offset > 0 {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            offset,
            body.len() - 1,
            body.len(),
            remaining.len()
        )
    } else {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            remaining.len()
        )
    };

    let _ = stream.write_all(header.as_bytes());

    // The announced length stays the real one, a cut therefore looks like a truncated transfer.
    let send = if first {
        behaviour
            .cut_after
            .unwrap_or(remaining.len())
            .min(remaining.len())
    } else {
        remaining.len()
    };

    let _ = stream.write_all(&remaining[..send]);
    let _ = stream.flush();
    served.fetch_add(send as u64, Ordering::Relaxed);
}

/// Reads the first byte of the range of a request.
fn range_start(request: &str) -> Option<u64> {
    for line in request.lines() {
        let lower = line.to_ascii_lowercase();

        if let Some(rest) = lower.strip_prefix("range: bytes=") {
            return rest.split('-').next()?.trim().parse().ok();
        }
    }

    None
}

fn payload(size: usize) -> Vec<u8> {
    (0..size).map(|index| (index % 251) as u8).collect()
}

fn digest(body: &[u8]) -> String {
    hex_digest(&sha1_digest(body))
}

struct Workspace {
    directory: PathBuf,
}

impl Workspace {
    fn new(name: &str) -> Workspace {
        let directory =
            std::env::temp_dir().join(format!("aether-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a temporary directory");

        Workspace { directory }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn an_interrupted_transfer_resumes_from_the_exact_byte() {
    let _guard = lock();

    let body = payload(262_144);
    let half = body.len() / 2;
    let workspace = Workspace::new("resume");
    let destination = workspace.file("client.jar");
    let partial = destination.with_extension("part");

    // The partial file of a previous run is the source of truth.
    std::fs::write(&partial, &body[..half]).unwrap();

    let server = Server::start(body.clone(), Behaviour::default());
    let report = Client::new()
        .download(
            &format!("{}/client.jar", server.url),
            &destination,
            Some(&digest(&body)),
            |_, _| {},
        )
        .expect("the download was resumed");

    assert_eq!(
        std::fs::read(&destination).unwrap(),
        body,
        "the resumed file differs from the original"
    );
    assert_eq!(
        server.ranges(),
        vec![Some(half as u64)],
        "the request did not resume at the size of the partial file"
    );
    assert_eq!(
        server.served(),
        (body.len() - half) as u64,
        "bytes were fetched again"
    );
    assert_eq!(report.resumed_from, half as u64);
    assert_eq!(report.fetched, (body.len() - half) as u64);
    assert!(!partial.exists(), "the partial file was left behind");

    println!(
        "resume exact: size={} part={} served={} fetched={}",
        body.len(),
        half,
        server.served(),
        report.fetched
    );
}

#[test]
fn a_cut_in_the_middle_only_costs_the_missing_bytes() {
    let _guard = lock();

    let body = payload(512 * 1024);
    let workspace = Workspace::new("cut");
    let destination = workspace.file("client.jar");

    let server = Server::start(
        body.clone(),
        Behaviour {
            cut_after: Some(200 * 1024),
            ignore_range: false,
            shifted_start: None,
        },
    );

    let report = Client::new()
        .download(
            &format!("{}/client.jar", server.url),
            &destination,
            Some(&digest(&body)),
            |_, _| {},
        )
        .expect("the download survived the cut");

    assert_eq!(
        std::fs::read(&destination).unwrap(),
        body,
        "the file differs from the original"
    );

    let ranges = server.ranges();
    assert_eq!(ranges.len(), 2, "the transfer was not resumed: {:?}", ranges);

    let resumed_from = ranges[1].expect("the second request carried no range");
    assert!(
        resumed_from > 0 && resumed_from <= 200 * 1024,
        "unexpected resume point {}",
        resumed_from
    );

    let served = server.served();
    assert!(
        served < 2 * body.len() as u64,
        "{} bytes were served for a file of {} bytes",
        served,
        body.len()
    );
    assert_eq!(report.bytes, body.len() as u64);
    assert!(report.resumed_from > 0);

    println!(
        "cut mid transfer: size={} served={} resume_point={}",
        body.len(),
        served,
        resumed_from
    );
}

#[test]
fn a_server_that_answers_with_the_whole_file_restarts_from_zero() {
    let _guard = lock();

    let body = payload(128 * 1024);
    let workspace = Workspace::new("no-range");
    let destination = workspace.file("client.jar");
    let partial = destination.with_extension("part");

    // A partial file with the wrong bytes: concatenating it would corrupt the result.
    std::fs::write(&partial, vec![0xAAu8; 4096]).unwrap();

    let server = Server::start(
        body.clone(),
        Behaviour {
            cut_after: None,
            ignore_range: true,
            shifted_start: None,
        },
    );

    let report = Client::new()
        .download(
            &format!("{}/client.jar", server.url),
            &destination,
            Some(&digest(&body)),
            |_, _| {},
        )
        .expect("the download started over");

    assert_eq!(
        std::fs::read(&destination).unwrap(),
        body,
        "the partial file was concatenated with the new body"
    );
    assert_eq!(
        std::fs::metadata(&destination).unwrap().len(),
        body.len() as u64
    );
    assert_eq!(
        server.ranges(),
        vec![Some(4096)],
        "the range was not requested"
    );
    assert_eq!(server.served(), body.len() as u64);
    assert_eq!(report.resumed_from, 0);
    assert!(!partial.exists());

    println!(
        "200 instead of 206: size={} served={} resumed_from={}",
        body.len(),
        server.served(),
        report.resumed_from
    );
}

#[test]
fn a_range_that_is_not_the_requested_one_is_not_appended() {
    let _guard = lock();

    let body = payload(262_144);
    let half = body.len() / 2;
    let workspace = Workspace::new("shifted");
    let destination = workspace.file("client.jar");
    let partial = destination.with_extension("part");

    std::fs::write(&partial, &body[..half]).unwrap();

    // The request asks for the second half, the server announces and serves a range that starts
    // near the end of the file.
    let shifted = body.len() - 4096;
    let server = Server::start(
        body.clone(),
        Behaviour {
            cut_after: None,
            ignore_range: false,
            shifted_start: Some(shifted),
        },
    );

    let report = Client::new()
        .download(
            &format!("{}/client.jar", server.url),
            &destination,
            Some(&digest(&body)),
            |_, _| {},
        )
        .expect("the download started over");

    assert_eq!(
        std::fs::read(&destination).unwrap(),
        body,
        "the bytes of the shifted range were appended"
    );
    assert_eq!(
        server.ranges(),
        vec![Some(half as u64), None],
        "the shifted answer was accepted instead of starting over"
    );
    assert_eq!(report.resumed_from, 0);
    assert_eq!(
        server.served(),
        (body.len() - shifted) as u64 + body.len() as u64,
        "the full file was not fetched again"
    );
    assert!(!partial.exists(), "the partial file was left behind");

    println!(
        "shifted content-range: size={} requested={} announced={} served={} ranges={:?}",
        body.len(),
        half,
        shifted,
        server.served(),
        server.ranges()
    );
}

#[test]
fn a_corrupt_partial_file_is_rejected_by_the_checksum_and_fetched_again() {
    let _guard = lock();

    let body = payload(256 * 1024);
    let half = body.len() / 2;
    let workspace = Workspace::new("corrupt");
    let destination = workspace.file("client.jar");
    let partial = destination.with_extension("part");

    let mut wrong = body[..half].to_vec();
    for byte in wrong.iter_mut() {
        *byte ^= 0xFF;
    }
    std::fs::write(&partial, &wrong).unwrap();

    let server = Server::start(body.clone(), Behaviour::default());
    let report = Client::new()
        .download(
            &format!("{}/client.jar", server.url),
            &destination,
            Some(&digest(&body)),
            |_, _| {},
        )
        .expect("the download was fetched again");

    assert_eq!(
        std::fs::read(&destination).unwrap(),
        body,
        "the corrupt prefix was promoted"
    );
    assert_eq!(
        server.ranges(),
        vec![Some(half as u64), None],
        "the second attempt did not start from zero"
    );
    assert_eq!(
        server.served(),
        (body.len() - half) as u64 + body.len() as u64,
        "the corrupt transfer was not fetched again"
    );
    assert_eq!(report.resumed_from, 0);
    assert!(!partial.exists());

    println!(
        "corrupt part: size={} served={} ranges={:?}",
        body.len(),
        server.served(),
        server.ranges()
    );
}

#[test]
fn a_session_builds_a_single_http_client() {
    let _guard = lock();

    let body = payload(4096);
    let workspace = Workspace::new("client");
    let server = Server::start(body.clone(), Behaviour::default());

    let before = clients_built();
    let client = Client::new();

    for index in 0..3 {
        let destination = workspace.file(&format!("file-{}.bin", index));
        client
            .download(
                &format!("{}/file.bin", server.url),
                &destination,
                Some(&digest(&body)),
                |_, _| {},
            )
            .expect("the file was downloaded");
        assert_eq!(std::fs::read(&destination).unwrap(), body);
    }

    assert_eq!(
        clients_built() - before,
        1,
        "the session built more than one HTTP client"
    );

    println!("clients built for three downloads: {}", clients_built() - before);
}
