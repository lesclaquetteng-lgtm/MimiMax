//! A loopback HTTP server for the tests.
//!
//! The download, verification and dependency-resolution tests drive the real client against this
//! server instead of a mock: the bytes, the checksums and the HTTP status codes are the ones a
//! running launcher would see. A route answers `200` with its body, every other path answers `404`,
//! and every request is counted so a test can prove that a file was not fetched twice.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// A server that answers the fixed routes of a test.
pub struct TestServer {
    address: String,
    hits: Arc<Mutex<Vec<String>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Starts a server on a free loopback port.
    ///
    /// A key of `routes` is a path without its query string, for example `/project/sodium/version`;
    /// the body is sent as `200 OK` with the matching `content-type` when the key ends in `.json`
    /// or when it starts with `/project` or `/version`.
    pub fn start(routes: Vec<(String, Vec<u8>)>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        listener
            .set_nonblocking(true)
            .expect("a non-blocking listener");
        let address = listener
            .local_addr()
            .expect("the address of the listener")
            .to_string();
        let routes: HashMap<String, Vec<u8>> = routes.into_iter().collect();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let thread_hits = Arc::clone(&hits);
        let thread_shutdown = Arc::clone(&shutdown);
        let base = format!("http://{}", address);
        let handle = thread::spawn(move || {
            while !thread_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve(stream, &routes, &thread_hits, &base),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });

        TestServer {
            address,
            hits,
            shutdown,
            handle: Some(handle),
        }
    }

    /// The address of the server, without a scheme.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The absolute URL of a path.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    /// The paths that were requested, in order, without their query strings.
    pub fn hits(&self) -> Vec<String> {
        self.hits
            .lock()
            .map(|hits| hits.clone())
            .unwrap_or_default()
    }

    /// How many requests hit a path.
    pub fn count(&self, path: &str) -> usize {
        self.hits()
            .iter()
            .filter(|hit| hit.as_str() == path)
            .count()
    }

    /// Every request, as one string, so a failure message can show what really happened.
    pub fn report(&self) -> String {
        self.hits().join(", ")
    }

    fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Reads one request and answers it.
///
/// A body that contains `{base}` has it replaced with the address of the server: a route can then
/// name the URL of another route of the same server, which is what a version document does with
/// its download links.
fn serve(
    mut stream: TcpStream,
    routes: &HashMap<String, Vec<u8>>,
    hits: &Mutex<Vec<String>>,
    base: &str,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let mut buffer = [0u8; 4096];
    let Ok(read) = stream.read(&mut buffer) else {
        return;
    };
    if read == 0 {
        return;
    }

    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(line) = request.lines().next() else {
        return;
    };
    let mut parts = line.split_whitespace();
    let _method = parts.next();
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/").to_string();

    if let Ok(mut hits) = hits.lock() {
        hits.push(path.clone());
    }

    let (status, content_type, body) = match routes.get(&path) {
        Some(body) => ("200 OK", "application/json", with_base(body.clone(), base)),
        None => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };

    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

/// Replaces the `{base}` placeholder of a body with the address of the server.
fn with_base(body: Vec<u8>, base: &str) -> Vec<u8> {
    const PLACEHOLDER: &[u8] = b"{base}";

    let mut out = Vec::with_capacity(body.len() + base.len());
    let mut index = 0;

    while let Some(position) = body[index..]
        .windows(PLACEHOLDER.len())
        .position(|window| window == PLACEHOLDER)
    {
        out.extend_from_slice(&body[index..index + position]);
        out.extend_from_slice(base.as_bytes());
        index += position + PLACEHOLDER.len();
    }

    out.extend_from_slice(&body[index..]);
    out
}
