//! Reads what each published Agent run's own files say about the conversation it holds, outside
//! every lock before a capture: its Agent's run table, a record one of the run's processes keeps
//! that names its process id, else, for an Agent that keeps none, its session store, where only the
//! one session in the run's directory written since the run started counts. Every read is bounded
//! and every parse strict; anything missing, unreadable, oversized, malformed or ambiguous is no
//! entry, never an error. The capture rule decides what an entry is worth.

use super::{ProcessSourceEntry, ProcessSourceKind, RunAnchor, RunProbe};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};
use std::{
    fs::{File, Metadata},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

mod claude;
mod run_tables;
mod session_stores;

/// A record or session written this long before the instant it must follow still counts: Agents
/// write some times in whole seconds.
const SLACK_MILLISECONDS: u64 = 1_000;
/// At most this many entries of one directory listing are looked at.
const MAXIMUM_LISTED: usize = 4_096;
/// A whole JSON file larger than this is not one an Agent keeps for its runs.
const MAXIMUM_FILE_BYTES: u64 = 1024 * 1024;
/// A session file's leading line larger than this is not its header.
const MAXIMUM_LINE_BYTES: u64 = 256 * 1024;

/// Whether `provider` has a process source this build reads.
pub(super) fn reads(provider: AgentResumeProvider) -> bool {
    run_tables::reads(provider) || session_stores::reads(provider)
}

/// The entry each probe's own files give, if any: its run table's, else its session store's sole
/// session while its directory is its own.
pub(crate) fn read(probes: &[RunProbe]) -> Vec<ProcessSourceEntry> {
    let Some(home) = std::env::home_dir() else {
        return Vec::new();
    };
    probes
        .iter()
        .filter_map(|probe| {
            let entry = |kind, session| ProcessSourceEntry {
                anchor: probe.anchor,
                session,
                kind,
            };
            if let Some(session) = run_tables::read(&home, probe) {
                return Some(entry(ProcessSourceKind::RunTable, session));
            }
            let directory = probe
                .directory
                .as_deref()
                .filter(|_| !probe.shared_directory)?;
            let started = unix_milliseconds(probe.anchor.created)?;
            session_stores::read(&home, probe.provider, directory, started)
                .map(|session| entry(ProcessSourceKind::SessionStore, session))
        })
        .collect()
}

/// Whether two directory paths name the same directory as Agents write them: separators and a
/// trailing separator aside, and on Windows without regard to case.
pub(super) fn same_directory(left: &str, right: &str) -> bool {
    let normal = |path: &str| {
        let path = path.trim_end_matches(['\\', '/']);
        if cfg!(windows) {
            path.replace('/', "\\").to_lowercase()
        } else {
            path.to_owned()
        }
    };
    !left.is_empty() && normal(left) == normal(right)
}

/// A sample row creation time, 100 ns units since 1601, as Unix milliseconds.
fn unix_milliseconds(created: u64) -> Option<u64> {
    const UNIX_EPOCH_FILETIME: u64 = 116_444_736_000_000_000;
    Some(created.checked_sub(UNIX_EPOCH_FILETIME)? / 10_000)
}

/// Whether a time an Agent wrote, in Unix milliseconds, is not before `instant`, give or take
/// [`SLACK_MILLISECONDS`].
fn since(written: u64, instant: u64) -> bool {
    written.saturating_add(SLACK_MILLISECONDS) >= instant
}

fn system_milliseconds(time: std::io::Result<SystemTime>) -> Option<u64> {
    let elapsed = time.ok()?.duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(elapsed.as_millis()).ok()
}

/// When `metadata`'s file was last written, in Unix milliseconds.
fn modified(metadata: &Metadata) -> Option<u64> {
    system_milliseconds(metadata.modified())
}

/// When `metadata`'s file was created, in Unix milliseconds.
fn created(metadata: &Metadata) -> Option<u64> {
    system_milliseconds(metadata.created())
}

/// Whether `path` was written at or after `instant`.
fn written_since(path: &Path, instant: u64) -> bool {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| modified(&metadata))
        .is_some_and(|written| since(written, instant))
}

/// Up to [`MAXIMUM_LISTED`] entries of `directory`, each with its name and metadata.
fn listed(directory: &Path) -> Vec<(String, PathBuf, Metadata)> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .take(MAXIMUM_LISTED)
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            let metadata = entry.metadata().ok()?;
            Some((name, entry.path(), metadata))
        })
        .collect()
}

/// The subdirectories of `directory` written at or after `instant`: those a new entry was added to
/// since then.
fn directories_since(directory: &Path, instant: u64) -> Vec<(String, PathBuf)> {
    listed(directory)
        .into_iter()
        .filter(|(_, _, metadata)| {
            metadata.is_dir() && modified(metadata).is_some_and(|written| since(written, instant))
        })
        .map(|(name, path, _)| (name, path))
        .collect()
}

/// The files of `directory` with `suffix` written at or after `instant`.
fn files_since(directory: &Path, suffix: &str, instant: u64) -> Vec<PathBuf> {
    listed(directory)
        .into_iter()
        .filter(|(name, _, metadata)| {
            metadata.is_file()
                && name.ends_with(suffix)
                && modified(metadata).is_some_and(|written| since(written, instant))
        })
        .map(|(_, path, _)| path)
        .collect()
}

/// A whole small file, when it is a regular file within [`MAXIMUM_FILE_BYTES`].
fn small_file(path: &Path) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_FILE_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAXIMUM_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (u64::try_from(bytes.len()).ok()? <= MAXIMUM_FILE_BYTES).then_some(bytes)
}

/// A small file parsed as one JSON value of type `T`.
fn json_file<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&small_file(path)?).ok()
}

/// The first `count` lines of a JSON Lines file parsed as JSON values, each within
/// [`MAXIMUM_LINE_BYTES`]; it stops at the first line that is not.
fn leading_lines(path: &Path, count: usize) -> Vec<serde_json::Value> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let mut reader = BufReader::new(file);
    let mut values = Vec::new();
    for _ in 0..count {
        let mut line = Vec::new();
        let read = (&mut reader)
            .take(MAXIMUM_LINE_BYTES + 1)
            .read_until(b'\n', &mut line);
        if !matches!(read, Ok(1..)) || line.last() != Some(&b'\n') {
            break;
        }
        match serde_json::from_slice(&line) {
            Ok(value) => values.push(value),
            Err(_) => break,
        }
    }
    values
}

/// A text field of a JSON object.
fn text<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    value.get(field)?.as_str()
}

/// An RFC 3339 UTC or offset time with optional fractional seconds, as Unix milliseconds.
fn rfc3339_milliseconds(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = text.get(range)?;
        digits
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| digits.parse().ok())
            .flatten()
    };
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    let mut index = 19;
    let mut milliseconds = 0;
    if bytes.get(index) == Some(&b'.') {
        let digits = bytes[index + 1..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if digits == 0 {
            return None;
        }
        let fraction = text.get(index + 1..index + 1 + digits.min(3))?;
        milliseconds = fraction.parse::<i64>().ok()? * 10_i64.pow(3 - fraction.len() as u32);
        index += 1 + digits;
    }
    let offset = match text.get(index..)? {
        "Z" | "z" => 0,
        zone if zone.len() == 6 && matches!(zone.as_bytes()[0], b'+' | b'-') => {
            let minutes = number(index + 1..index + 3)? * 60 + number(index + 4..index + 6)?;
            if zone.as_bytes()[3] != b':' {
                return None;
            }
            if zone.starts_with('-') {
                -minutes
            } else {
                minutes
            }
        }
        _ => return None,
    };
    // Days since 1970-01-01 in the proleptic Gregorian calendar.
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second - offset * 60;
    u64::try_from(seconds * 1_000 + milliseconds).ok()
}

/// The session of `provider` that `id` names, when it is valid there.
fn session(provider: AgentResumeProvider, id: &str) -> Option<RetainedAgentSession> {
    RetainedAgentSession::new(provider, id)
}

/// The one distinct session of `found`, or none when there are several or none.
fn sole(found: impl IntoIterator<Item = RetainedAgentSession>) -> Option<RetainedAgentSession> {
    let mut found = found.into_iter();
    let first = found.next()?;
    found.all(|other| other == first).then_some(first)
}

/// The creation time, in Unix milliseconds, of the probe's process with `pid`, if it has one.
fn process_started(processes: &[RunAnchor], pid: u64) -> Option<u64> {
    processes
        .iter()
        .find(|process| u64::from(process.pid) == pid)
        .and_then(|process| unix_milliseconds(process.created))
}
