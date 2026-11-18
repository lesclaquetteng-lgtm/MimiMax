//! The fake core of the end to end check.
//!
//! `launch` starts a child process that lives long, like Java, and prints the same documents as the
//! real core. Every other command is forwarded to the real binary so that the interface behaves
//! exactly as it does in production.
//!
//! `AETHER_FAKE_FAIL` makes a command fail like a real core does:
//!
//! - `install` prints a progress document, then an error document, and stops on a non-zero code;
//! - `launch` prints only the error document and stops on a non-zero code;
//! - `launch-crash` stops on a non-zero code **without any document**, which is what a core that
//!   dies (panic, missing library, antivirus kill) actually looks like.

use std::io::Write;
use std::process::{Command, Stdio};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().cloned().unwrap_or_default();
    let mode = std::env::var("AETHER_FAKE_FAIL").unwrap_or_default();

    let tokens: Vec<String> = mode
        .split(',')
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .collect();

    if command == "install" && tokens.iter().any(|token| token == "install-slow") {
        slow_install();
    }

    if command == "install" && tokens.iter().any(|token| token == "install-stubborn") {
        stubborn_install();
    }

    if tokens.iter().any(|token| token == &command) {
        fail(&command, false);
    }

    if tokens.iter().any(|token| token == &format!("{}-crash", command)) {
        fail(&command, true);
    }

    if command == "launch" && tokens.iter().any(|token| token == "launch-exit") {
        launch_exit();
    }

    if command == "launch" {
        launch();
        return;
    }

    let real = std::env::var("AETHER_REAL_CORE").expect("AETHER_REAL_CORE is set");
    let status = Command::new(&real)
        .args(&arguments)
        .status()
        .expect("the real core starts");

    std::process::exit(status.code().unwrap_or(1));
}

/// Stops like a core that gave up, with a document unless `silent` leaves only the exit code.
fn fail(command: &str, silent: bool) -> ! {
    if !silent && command == "install" {
        // The interface shows the bar before the answer: a failure in the middle of a download
        // must put it back down.
        println!(
            r#"{{"type":"progress","task":"Downloading libraries","received":1024,"total":4096,"completed":1,"total_tasks":5}}"#
        );
    }

    if !silent {
        println!(
            r#"{{"ok":false,"error":"fake core: {} failed on purpose"}}"#,
            command
        );
    }

    let _ = std::io::stdout().flush();
    std::process::exit(3);
}

/// An installation that runs long enough to be cancelled.
///
/// It prints progress every 200 ms and stops as soon as `cancel` arrives on its standard input,
/// which is the channel the interface uses.
fn slow_install() -> ! {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);

    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().eq_ignore_ascii_case("cancel") {
                flag.store(true, Ordering::Relaxed);
            }
        }
    });

    let mut received = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            println!(r#"{{"ok":false,"cancelled":true,"error":"the installation was cancelled"}}"#);
            let _ = std::io::stdout().flush();
            std::process::exit(0);
        }

        received = (received + 3).min(100);
        println!(
            r#"{{"type":"progress","task":"Downloading assets","received":{},"total":100,"completed":1,"totalTasks":5}}"#,
            received
        );
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// An installation that ignores `cancel` on purpose, used to prove the safety kill of the
/// launcher: it never reads its standard input and never stops on its own.
fn stubborn_install() -> ! {
    let mut received = 0;
    loop {
        received = (received + 5).min(100);
        println!(
            r#"{{"type":"progress","task":"Downloading assets","received":{},"total":100,"completed":1,"totalTasks":5}}"#,
            received
        );
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
}

/// A game that starts and dies at once, like the real core reports it: two lines of output and a
/// non-zero exit code in the answer document.
fn launch_exit() -> ! {
    println!(r#"{{"type":"output","line":"fake java started"}}"#);
    println!(r#"{{"type":"output","line":"Error: the fake game died"}}"#);
    println!(r#"{{"ok":true,"exitCode":3}}"#);
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

/// Starts the fake game and waits until the process is killed.
fn launch() {
    let mut command = long_lived();
    let child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the child of the fake game starts");

    println!(r#"{{"type":"output","line":"fake java started"}}"#);
    println!(
        r#"{{"type":"output","line":"fake java child {}"}}"#,
        child.id()
    );
    let _ = std::io::stdout().flush();

    // The check reads both identifiers from this file and verifies that they are gone after a stop.
    if let Ok(path) = std::env::var("AETHER_FAKE_PID_FILE") {
        let content = format!("{}\n{}\n", std::process::id(), child.id());
        let _ = std::fs::write(&path, content);
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// A command that runs for about ten minutes.
fn long_lived() -> Command {
    #[cfg(windows)]
    {
        let mut command = Command::new("ping");
        command.args(["-n", "600", "127.0.0.1"]);
        command
    }

    #[cfg(not(windows))]
    {
        let mut command = Command::new("sleep");
        command.arg("600");
        command
    }
}
