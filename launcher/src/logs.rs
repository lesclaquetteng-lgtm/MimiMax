//! The log files of the launcher.
//!
//! Two kinds of files live in `aether/logs`:
//!
//! * `install.log`, one line per download of an installation;
//! * `session-<date>.log`, the whole output of one run of the game.
//!
//! The session files are written by the core as the game prints, line by line and flushed, so a
//! crash of the game or of the launcher still leaves the explanations on the disk. Only the newest
//! sessions are kept: a launcher that runs every day must not fill the disk with old logs.
//!
//! The dates of the names and of the headers are the local time of the player, the one they read
//! on their clock.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, TimeZone};

use crate::paths::Paths;

/// The name of the file that receives one line per download.
pub const INSTALL_LOG: &str = "install.log";
/// The prefix of the log of one run of the game.
pub const SESSION_PREFIX: &str = "session-";
/// The extension of the log of one run of the game.
pub const SESSION_SUFFIX: &str = ".log";
/// How many session logs are kept, the oldest ones are removed when a new session starts.
pub const KEEP_SESSIONS: usize = 20;
/// The total size the session logs are kept under, the oldest ones go first.
///
/// The count alone is not enough: twenty verbose sessions can weigh hundreds of megabytes. The
/// newest session is always kept, even when it alone passes the budget — it is the one being
/// written, and the one a crash report reads.
pub const MAX_SESSIONS_BYTES: u64 = 64 * 1024 * 1024;
/// The size above which the log of the installations is shortened.
///
/// A full installation writes one line per file, the file would grow with every repair.
pub const MAX_INSTALL_LOG_BYTES: u64 = 512 * 1024;

/// The last lines of a log file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Tail {
    /// The file that was read.
    pub path: String,
    /// `false` when the file does not exist yet, which is not an error.
    pub exists: bool,
    /// The number of lines the file holds.
    pub total: usize,
    /// The lines that are kept, oldest first.
    pub lines: Vec<String>,
}

/// One run of the game, as it is seen from the logs folder.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    /// The name of the file, which is what the interface asks to read.
    pub name: String,
    /// The full path of the file.
    pub path: String,
    /// The size of the file, in bytes.
    pub size: u64,
    /// The start of the session, read from the name of the file.
    pub started: String,
}

/// A file that receives the output of one run of the game.
///
/// Every line is written and flushed at once: the value of this log is to survive a crash, so
/// nothing may stay in a buffer.
pub struct SessionLog {
    file: File,
    path: PathBuf,
}

impl SessionLog {
    /// Creates the log of a new session in the logs folder.
    pub fn start(paths: &Paths) -> io::Result<SessionLog> {
        fs::create_dir_all(&paths.logs)?;

        let now = Local::now();
        let name = format!("{}{}{}", SESSION_PREFIX, stamp(now), SESSION_SUFFIX);
        let mut session = SessionLog::create(paths.logs.join(name))?;
        session.line(&format!("# Aether session {}", human(now)));

        // The session that just started is the newest one, the old files go now.
        prune(&paths.logs, KEEP_SESSIONS);

        Ok(session)
    }

    /// Opens a session log at an exact path.
    pub fn create(path: PathBuf) -> io::Result<SessionLog> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(SessionLog { file, path })
    }

    /// The file this session is written to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one line of the game.
    pub fn line(&mut self, line: &str) {
        let _ = writeln!(self.file, "{}", line);
        let _ = self.file.flush();
    }

    /// Closes the session with the code the game stopped with.
    pub fn finish(&mut self, code: i32) {
        self.line(&format!("# exit code {}", code));
    }
}

/// The path of the log of the installations.
pub fn install_log(paths: &Paths) -> PathBuf {
    paths.logs.join(INSTALL_LOG)
}

/// Reads the last `limit` lines of a log file.
pub fn read_tail(file: &Path, limit: usize) -> Result<Tail, io::Error> {
    let path = file.to_string_lossy().to_string();

    let content = match fs::read_to_string(file) {
        Ok(content) => content,
        // A launcher that was never installed has no log, it is an empty one and not a failure.
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Tail {
                path,
                exists: false,
                total: 0,
                lines: Vec::new(),
            });
        }
        Err(error) => return Err(error),
    };

    let all: Vec<&str> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    let total = all.len();
    let start = total.saturating_sub(limit.max(1));

    Ok(Tail {
        path,
        exists: true,
        total,
        lines: all[start..]
            .iter()
            .map(|line| line.to_string())
            .collect(),
    })
}

/// The session logs of the launcher, the newest first.
pub fn sessions(paths: &Paths, limit: usize) -> Vec<SessionEntry> {
    sessions_in(&paths.logs, limit)
}

/// The session logs of a folder, the newest first.
pub fn sessions_in(directory: &Path, limit: usize) -> Vec<SessionEntry> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut sessions: Vec<SessionEntry> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            if !is_session(&name) {
                return None;
            }

            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }

            Some(SessionEntry {
                started: started_from(&name),
                path: entry.path().to_string_lossy().to_string(),
                size: metadata.len(),
                name,
            })
        })
        .collect();

    // The name holds the date, so the newest file sorts first.
    sessions.sort_by(|left, right| right.name.cmp(&left.name));
    sessions.truncate(limit.max(1));
    sessions
}

/// Cuts a log file down to its last `keep` bytes when it grew past `limit`.
///
/// The cut falls on the start of a line, and the shorter file replaces the old one in one go: a
/// reader never sees half a log, and nothing is left behind if the write fails.
pub fn trim(file: &Path, limit: u64, keep: u64) -> io::Result<bool> {
    let Ok(metadata) = fs::metadata(file) else {
        return Ok(false);
    };
    if metadata.len() <= limit || limit == 0 || keep == 0 {
        return Ok(false);
    }

    let content = fs::read(file)?;
    let mut start = content.len().saturating_sub(keep as usize);

    // The bytes are cut on a character boundary first, a player name in the log must not be torn.
    while start < content.len() && (content[start] & 0b1100_0000) == 0b1000_0000 {
        start += 1;
    }
    let start = match content[start..].iter().position(|byte| *byte == b'\n') {
        Some(offset) => start + offset + 1,
        None => content.len(),
    };

    let temporary = file.with_extension("log.tmp");
    if let Err(error) = fs::write(&temporary, &content[start..]) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, file) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    Ok(true)
}

/// Removes the oldest session logs and keeps the `keep` most recent ones.
///
/// It returns the number of files that were removed, an unreadable file only costs its own log.
pub fn prune(directory: &Path, keep: usize) -> usize {
    prune_with_budget(directory, keep, MAX_SESSIONS_BYTES)
}

/// The same retention with an explicit byte budget, `0` meaning no budget at all.
///
/// A session is removed when it is past the `keep` most recent ones, or when keeping it would
/// push the total past `max_bytes`. The newest file is never removed: it is the session being
/// written. The bytes of the files that stay are the ones counted, a removed file frees its
/// whole size for the next ones.
pub fn prune_with_budget(directory: &Path, keep: usize, max_bytes: u64) -> usize {
    let mut kept_bytes: u64 = 0;
    let mut removed = 0;

    for (index, entry) in sessions_in(directory, usize::MAX).into_iter().enumerate() {
        let too_many = index >= keep.max(1);
        let too_big = max_bytes > 0
            && index > 0
            && kept_bytes.saturating_add(entry.size) > max_bytes;

        if too_many || too_big {
            if fs::remove_file(&entry.path).is_ok() {
                removed += 1;
            }
        } else {
            kept_bytes = kept_bytes.saturating_add(entry.size);
        }
    }

    removed
}

/// Reads the last lines of one session log.
pub fn read_session(paths: &Paths, name: &str, limit: usize) -> Result<Tail, String> {
    read_session_in(&paths.logs, name, limit)
}

/// Reads the last lines of one session log of a folder.
pub fn read_session_in(directory: &Path, name: &str, limit: usize) -> Result<Tail, String> {
    let path = Path::new(name);
    if !is_session(name) || path.file_name().and_then(|value| value.to_str()) != Some(name) {
        return Err(format!("invalid session name: {}", name));
    }

    read_tail(&directory.join(name), limit).map_err(|error| error.to_string())
}

/// Only the files written for a run of the game are sessions.
fn is_session(name: &str) -> bool {
    name.starts_with(SESSION_PREFIX) && name.ends_with(SESSION_SUFFIX)
}

/// `session-20261113-181500.log` is shown as `2026-11-13 18:15:00`.
fn started_from(name: &str) -> String {
    let stamp = name
        .strip_prefix(SESSION_PREFIX)
        .and_then(|value| value.strip_suffix(SESSION_SUFFIX))
        .unwrap_or_default();

    if stamp.len() != 15
        || !stamp
            .chars()
            .all(|character| character.is_ascii_digit() || character == '-')
    {
        return String::new();
    }

    format!(
        "{}-{}-{} {}:{}:{}",
        &stamp[0..4],
        &stamp[4..6],
        &stamp[6..8],
        &stamp[9..11],
        &stamp[11..13],
        &stamp[13..15]
    )
}

/// The date of an instant, as `20261113-181500`, which sorts like the clock.
///
/// The zone is the one of the moment it is given, the caller passes the local time.
pub fn stamp<Tz: TimeZone>(moment: DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    moment.format("%Y%m%d-%H%M%S").to_string()
}

/// The same instant, as `2026-11-13 18:15:00`.
pub fn human<Tz: TimeZone>(moment: DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    moment.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};

    /// An empty folder that belongs to one test.
    fn temporary(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("aether-logs-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn the_tail_keeps_the_last_lines() {
        let root = temporary("tail");
        let file = root.join(INSTALL_LOG);

        let content: String = (0..260)
            .map(|index| format!("download {} — 4096 bytes\n", index))
            .collect();
        fs::write(&file, content).unwrap();

        let tail = read_tail(&file, 5).unwrap();
        assert!(tail.exists);
        assert_eq!(tail.total, 260);
        assert_eq!(tail.lines.len(), 5, "{:?}", tail.lines);
        assert!(tail.lines[4].contains("download 259"), "{:?}", tail.lines);
        assert!(tail.path.ends_with(INSTALL_LOG));

        // A limit larger than the file keeps everything.
        let everything = read_tail(&file, 1000).unwrap();
        assert_eq!(everything.lines.len(), 260);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_log_is_an_empty_tail() {
        let root = temporary("missing");

        let tail = read_tail(&root.join(INSTALL_LOG), 10).unwrap();
        assert!(!tail.exists);
        assert_eq!(tail.total, 0);
        assert!(tail.lines.is_empty());
        assert!(tail.path.ends_with(INSTALL_LOG));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_output_of_a_session_is_flushed_to_its_file() {
        let root = temporary("session-write");
        let path = root.join("session-20261113-181500.log");

        let mut session = SessionLog::create(path.clone()).unwrap();
        session.line("fake java started");
        session.line("fake java child 4242");
        session.finish(0);

        // The lines are on the disk before the session ends, that is the point of this log.
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("fake java started"), "{}", content);
        assert!(content.contains("fake java child 4242"), "{}", content);
        assert!(content.contains("# exit code 0"), "{}", content);
        assert_eq!(session.path(), path.as_path());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_sessions_are_listed_newest_first() {
        let root = temporary("session-list");
        fs::write(root.join("session-20260101-090000.log"), "old\n").unwrap();
        fs::write(root.join("session-20261113-181500.log"), "newer\nlonger\n").unwrap();
        fs::write(root.join(INSTALL_LOG), "download x\n").unwrap();
        fs::write(root.join("notes.txt"), "not a session\n").unwrap();

        let sessions = sessions_in(&root, 10);
        assert_eq!(sessions.len(), 2, "{:?}", sessions);

        assert_eq!(sessions[0].name, "session-20261113-181500.log");
        assert_eq!(sessions[0].started, "2026-11-13 18:15:00");
        assert_eq!(sessions[0].size, 13);
        assert!(sessions[0].path.ends_with("session-20261113-181500.log"));

        assert_eq!(sessions[1].name, "session-20260101-090000.log");
        assert_eq!(sessions[1].started, "2026-01-01 09:00:00");

        // The limit keeps the newest ones.
        let only_one = sessions_in(&root, 1);
        assert_eq!(only_one.len(), 1);
        assert_eq!(only_one[0].name, "session-20261113-181500.log");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_oldest_sessions_are_removed() {
        let root = temporary("prune");
        for day in 1..=25 {
            fs::write(
                root.join(format!("session-202611{:02}-090000.log", day)),
                "run\n",
            )
            .unwrap();
        }
        fs::write(root.join(INSTALL_LOG), "download x\n").unwrap();

        let removed = prune(&root, KEEP_SESSIONS);
        assert_eq!(removed, 5);

        let left = sessions_in(&root, 100);
        assert_eq!(left.len(), KEEP_SESSIONS, "{:?}", left);
        assert_eq!(left[0].name, "session-20261125-090000.log");
        assert_eq!(
            left[KEEP_SESSIONS - 1].name,
            format!("session-202611{:02}-090000.log", 6)
        );
        assert!(!root.join("session-20261105-090000.log").exists());

        // The log of the installations is not a session and is left alone.
        assert!(root.join(INSTALL_LOG).exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_sessions_are_bounded_in_bytes_not_only_in_count() {
        let root = temporary("prune-bytes");
        // Four sessions of 30 MiB each: under the count of 20, far over the budget of 64 MiB.
        let big = vec![b'x'; 30 * 1024 * 1024];
        for day in 1..=4 {
            fs::write(
                root.join(format!("session-202611{:02}-090000.log", day)),
                &big,
            )
            .unwrap();
        }
        fs::write(root.join(INSTALL_LOG), "download x\n").unwrap();

        let removed = prune(&root, KEEP_SESSIONS);
        assert_eq!(removed, 2, "the two oldest sessions pass the budget");

        let left = sessions_in(&root, 100);
        assert_eq!(left.len(), 2, "{:?}", left.iter().map(|s| &s.name).collect::<Vec<_>>());
        assert_eq!(left[0].name, "session-20261104-090000.log");
        assert_eq!(left[1].name, "session-20261103-090000.log");
        assert!(root.join(INSTALL_LOG).exists(), "the install log is not a session");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_newest_session_survives_even_when_it_alone_passes_the_budget() {
        let root = temporary("prune-newest");
        // One session bigger than the whole budget, and an older small one.
        fs::write(
            root.join("session-20261102-090000.log"),
            vec![b'x'; (MAX_SESSIONS_BYTES as usize) + 1024],
        )
        .unwrap();
        fs::write(root.join("session-20261101-090000.log"), "old\n").unwrap();

        let removed = prune(&root, KEEP_SESSIONS);
        assert_eq!(removed, 1);

        let left = sessions_in(&root, 100);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].name, "session-20261102-090000.log");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_zero_budget_keeps_everything_under_the_count() {
        let root = temporary("prune-no-budget");
        let big = vec![b'x'; 40 * 1024 * 1024];
        for day in 1..=3 {
            fs::write(
                root.join(format!("session-202611{:02}-090000.log", day)),
                &big,
            )
            .unwrap();
        }

        let removed = prune_with_budget(&root, KEEP_SESSIONS, 0);
        assert_eq!(removed, 0);
        assert_eq!(sessions_in(&root, 100).len(), 3);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_log_of_the_installations_is_shortened() {
        let root = temporary("trim");
        let file = root.join(INSTALL_LOG);
        let content: String = (0..4_000)
            .map(|index| {
                format!(
                    "download libraries/{} — 4096 bytes (4096 fetched, 0 resumed) in 42 ms\n",
                    index
                )
            })
            .collect();
        fs::write(&file, &content).unwrap();
        let keep = 64 * 1024;

        assert!(trim(&file, 200_000, keep).unwrap());

        let size = fs::metadata(&file).unwrap().len();
        assert!(size <= keep, "the log kept {} bytes", size);

        // The file starts on a whole line and the newest lines are the ones that survived.
        let kept = fs::read_to_string(&file).unwrap();
        assert!(kept.starts_with("download "), "{}", &kept[..40]);
        assert!(kept.ends_with('\n'), "the last line is complete");
        assert!(kept.contains("download libraries/3999"), "the newest line is kept");
        assert!(!root.join("install.log.tmp").exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_small_log_is_left_alone() {
        let root = temporary("trim-small");
        let file = root.join(INSTALL_LOG);
        fs::write(&file, "download a\n").unwrap();

        assert!(!trim(&file, MAX_INSTALL_LOG_BYTES, 1024).unwrap());
        assert_eq!(fs::read_to_string(&file).unwrap(), "download a\n");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_session_is_read_back_by_its_name() {
        let root = temporary("session-read");
        fs::write(
            root.join("session-20261113-181500.log"),
            "# Aether session\nfake java started\nfake java child 4242\n",
        )
        .unwrap();

        let tail = read_session_in(&root, "session-20261113-181500.log", 10).unwrap();
        assert!(tail.exists);
        assert_eq!(tail.total, 3);
        assert!(tail.lines[1].contains("fake java started"), "{:?}", tail.lines);

        // Anything that is not a session of this folder is refused.
        for name in ["../aether/settings.json", "install.log", "session-.log/../x", ""] {
            let error = read_session_in(&root, name, 10).expect_err(name);
            assert!(error.contains("invalid session name"), "{}: {}", name, error);
        }

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_date_is_written_the_same_way_for_a_file_name_and_for_a_human() {
        // The local time of the player, whatever the offset of the machine.
        let moment = Local
            .with_ymd_and_hms(2026, 11, 13, 18, 15, 0)
            .single()
            .expect("a valid local date");

        assert_eq!(stamp(moment), "20261113-181500");
        assert_eq!(human(moment), "2026-11-13 18:15:00");

        // The offset of the machine is the one of the player, not UTC.
        assert_eq!(moment.offset(), &Local::now().offset().to_owned());
    }

    #[test]
    fn the_same_instant_follows_the_zone_it_is_given() {
        // 2026-11-13T18:15:00Z, the instant a session would start.
        let instant = DateTime::from_timestamp(1_794_593_700, 0).expect("a valid instant");

        assert_eq!(stamp(instant.with_timezone(&Utc)), "20261113-181500");
        assert_eq!(human(instant.with_timezone(&Utc)), "2026-11-13 18:15:00");

        // The same instant, one hour east: the log shows the clock of the player.
        let paris = FixedOffset::east_opt(3_600).expect("a valid offset");
        assert_eq!(stamp(instant.with_timezone(&paris)), "20261113-191500");
        assert_eq!(human(instant.with_timezone(&paris)), "2026-11-13 19:15:00");
    }
}
