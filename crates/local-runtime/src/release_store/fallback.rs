//! The store side of the fallback after a failed move: the transitions of the pointer's fallback
//! record, and the recorder that applies one of them under `install.lock`. Every completion of a
//! build whose automatic-update switch is on applies [`complete_move`]; the activator, crash
//! recovery, the Server and the previous version's Client-failure helper record through
//! [`FallbackRecorder`]. Each write reads `active.json` under the lock, applies one transition and
//! replaces the file through the store's flushed rename, so no two writes interleave and a crash
//! leaves the old file or the new one. Readers still take no lock.
//!
//! The bound: a fallback moves only to `previous`; never below a component's `securityFloor` in
//! the failed version's own verified manifest, nor to a release before slice 1, both settled as the
//! move's `back` when the move is written; only within ten minutes of the failed version taking
//! over; and at most once per move, since the failed version is then held until a higher version
//! has become active. The process that commits a fallback logs it.
use super::{RELEASE_POINTER, RELEASE_POINTER_LIMIT, RELEASE_VERSIONS};
use crate::{
    LogLevel, LogValue, PrivateCache, automatic_updates_live, compare_product_versions, log_event,
};
use orchbee_release_records::{
    ClientWindow, Entry, FALLBACK_FAILURES, FALLBACK_WINDOW_MILLISECONDS, Fallback, Move, Pointer,
    SessionWindow, valid_activation_id, valid_version,
};
use std::{
    cmp::Ordering,
    io,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// The most versions `held` lists; a ninth drops the earliest.
const HELD_LIMIT: usize = 8;
/// The most generations a Session window's `starts` lists; a ninth drops the oldest that did not
/// fail.
const STARTS_LIMIT: usize = 8;
/// How often a recorder tries the lock, and how long before it drops its record.
const LOCK_RETRY: Duration = Duration::from_millis(50);
const LOCK_BOUND: Duration = Duration::from_secs(2);

/// A completion refused its target because the target is held.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionHeld;

/// The held versions of `pointer`'s fallback record that lie above its active version: the
/// versions nothing may make active. The active version and `previous` are never among them.
pub fn held_versions(pointer: &Pointer) -> Vec<String> {
    pointer.fallback().map_or_else(Vec::new, |fallback| {
        keep_above(fallback.held, &pointer.active.product)
    })
}

/// The pointer after a completion of a build whose switch is on makes `entry` active, applied under
/// `install.lock` to the pointer read there. A held target is refused. Otherwise the pointer moves
/// as [`Pointer::activate`] moves it and drops held versions at or below the target; a real move
/// writes a fresh move for the target, and activating the active entry again keeps the record as it
/// is. The fresh move may fall back only when the record's move counted for the replaced entry,
/// which proves that the replaced version records, and `replaced_allows` accepts that entry: its
/// `release.json` still hashes to its manifest and it meets every component floor of the target's
/// own verified manifest. Without a readable pointer, the fresh move cannot fall back.
///
/// # Errors
/// [`VersionHeld`] when the target is held.
pub fn complete_move(
    current: Option<Pointer>,
    entry: Entry,
    replaced_allows: impl FnOnce(&Entry) -> bool,
) -> Result<Pointer, VersionHeld> {
    let Some(current) = current else {
        let mut pointer = Pointer::activate(None, entry.clone());
        pointer.set_fallback(&Fallback {
            held: Vec::new(),
            last_move: Move::fresh(entry, false, false),
        });
        return Ok(pointer);
    };
    if held_versions(&current).contains(&entry.product) {
        return Err(VersionHeld);
    }
    if current.active == entry {
        return Ok(Pointer::activate(Some(current), entry));
    }
    let proven = current.counting_move().is_some();
    let held = current
        .fallback()
        .map_or_else(Vec::new, |fallback| fallback.held);
    let replaced = current.active.clone();
    let mut pointer = Pointer::activate(Some(current), entry.clone());
    let back = proven && replaced_allows(&replaced);
    pointer.set_fallback(&Fallback {
        held: keep_above(held, &entry.product),
        last_move: Move::fresh(entry, back, false),
    });
    Ok(pointer)
}

/// One event a recorder writes into the counting move.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FallbackEvent<'a> {
    /// The activator of the recorder's version starts the generation `activation`, after
    /// `begin_activation` and before it starts the Server. Opens the Session window.
    SessionStart { activation: &'a str },
    /// The first TUI of the recorder's version starts. Opens the Client window.
    TuiStart,
    /// The generation `activation` failed to start or crashed: a Session failure.
    SessionFailure { activation: &'a str },
    /// A Client of `version` failed to start or to join, recorded by the previous version's helper.
    ClientFailure { version: &'a str },
}

impl FallbackEvent<'_> {
    const fn token(self) -> &'static str {
        match self {
            Self::SessionStart { .. } => "session-start",
            Self::TuiStart => "tui-start",
            Self::SessionFailure { .. } => "session-failure",
            Self::ClientFailure { .. } => "client-failure",
        }
    }

    const fn role(self) -> &'static str {
        match self {
            Self::SessionStart { .. } | Self::SessionFailure { .. } => "session",
            Self::TuiStart | Self::ClientFailure { .. } => "client",
        }
    }
}

/// What one event does to a pointer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Recorded {
    /// Nothing to write: the pointer has no counting move, that move may not fall back, the event
    /// is not the moved version's, or the role's window is closed, full or already opened.
    Nothing,
    /// The pointer with the event recorded.
    Written(Pointer),
    /// The event was a role's third failure and the store falls back: `pointer` makes `to`, the
    /// former `previous`, active and holds `from`.
    FellBack {
        pointer: Pointer,
        from: Entry,
        to: Entry,
    },
}

/// Records `event` of a recorder running version `own` at `now`, in Unix milliseconds UTC, into
/// `pointer`'s counting move. Only a counting move whose `back` is true records anything. A role's
/// window opens with its first record, lasts 600 000 ms, and records nothing dated before it opened;
/// the third failure inside it falls back when `previous_present` finds the folder `previous`
/// names, and otherwise is recorded while nothing falls back and the role records nothing more.
pub fn record_event(
    pointer: &Pointer,
    own: &str,
    event: FallbackEvent<'_>,
    now: u64,
    previous_present: impl FnOnce(&Entry) -> bool,
) -> Recorded {
    let Some(mut fallback) = pointer.fallback() else {
        return Recorded::Nothing;
    };
    let moved = &mut fallback.last_move;
    if moved.entry != pointer.active || !moved.back {
        return Recorded::Nothing;
    }
    let open =
        |since: u64| since <= now && now < since.saturating_add(FALLBACK_WINDOW_MILLISECONDS);
    let third = match event {
        FallbackEvent::TuiStart => {
            if own != moved.entry.product || moved.client.is_some() {
                return Recorded::Nothing;
            }
            moved.client = Some(ClientWindow {
                since: now,
                failures: 0,
            });
            false
        }
        FallbackEvent::ClientFailure { version } => {
            if version != moved.entry.product {
                return Recorded::Nothing;
            }
            let window = moved.client.get_or_insert(ClientWindow {
                since: now,
                failures: 0,
            });
            if !open(window.since) || usize::from(window.failures) >= FALLBACK_FAILURES {
                return Recorded::Nothing;
            }
            window.failures += 1;
            usize::from(window.failures) == FALLBACK_FAILURES
        }
        FallbackEvent::SessionStart { activation } => {
            if own != moved.entry.product || !valid_activation_id(activation) {
                return Recorded::Nothing;
            }
            let window = moved.session.get_or_insert(SessionWindow {
                since: now,
                starts: Vec::new(),
                failed: Vec::new(),
            });
            if !open(window.since)
                || window.failed.len() >= FALLBACK_FAILURES
                || window.starts.iter().any(|start| start == activation)
            {
                return Recorded::Nothing;
            }
            window.starts.push(activation.to_owned());
            if window.starts.len() > STARTS_LIMIT
                && let Some(oldest) = window
                    .starts
                    .iter()
                    .position(|start| !window.failed.contains(start))
            {
                window.starts.remove(oldest);
            }
            false
        }
        FallbackEvent::SessionFailure { activation } => {
            let Some(window) = moved.session.as_mut() else {
                return Recorded::Nothing;
            };
            if !open(window.since)
                || window.failed.len() >= FALLBACK_FAILURES
                || !window.starts.iter().any(|start| start == activation)
                || window.failed.iter().any(|failure| failure == activation)
            {
                return Recorded::Nothing;
            }
            window.failed.push(activation.to_owned());
            window.failed.len() == FALLBACK_FAILURES
        }
    };
    let mut written = pointer.clone();
    if third
        && let Some(previous) = pointer.previous.clone()
        && previous_present(&previous)
    {
        let from = pointer.active.clone();
        let mut held = fallback.held;
        held.push(from.product.clone());
        written.active = previous.clone();
        written.previous = None;
        written.set_fallback(&Fallback {
            held: keep_above(held, &previous.product),
            last_move: Move::fresh(previous.clone(), false, true),
        });
        return Recorded::FellBack {
            pointer: written,
            from,
            to: previous,
        };
    }
    written.set_fallback(&fallback);
    Recorded::Written(written)
}

/// `held` with only the versions above `active`, each once, and at most the newest eight held.
fn keep_above(held: Vec<String>, active: &str) -> Vec<String> {
    let mut kept: Vec<String> = Vec::with_capacity(held.len());
    for version in held {
        if compare_product_versions(&version, active) == Ordering::Greater
            && !kept.contains(&version)
        {
            kept.push(version);
        }
    }
    let excess = kept.len().saturating_sub(HELD_LIMIT);
    kept.drain(..excess);
    kept
}

/// What a recorder did with one event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FallbackOutcome {
    /// Nothing was recorded: the switch is off, the store has no counting move that may fall back,
    /// or the event does not count, as read without the lock and again under it.
    Skipped,
    Recorded,
    /// The event committed the fallback from `from` to `to`.
    FellBack {
        from: Entry,
        to: Entry,
    },
    /// The lock stayed taken for the whole bound, or the write failed: the record is lost, which
    /// can only delay a fallback.
    Dropped,
}

/// The recorder of the store one version folder belongs to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FallbackRecorder {
    root: PathBuf,
    version: String,
}

impl FallbackRecorder {
    /// The recorder for the store `executable` runs from, so a process outside a store version
    /// folder records nothing.
    pub fn for_executable(executable: &Path) -> Option<Self> {
        let directory = executable.parent()?;
        if !super::is_release_version_directory(directory) {
            return None;
        }
        Some(Self {
            root: directory.parent()?.parent()?.to_owned(),
            version: directory.file_name()?.to_str()?.to_owned(),
        })
    }

    /// The version this recorder runs.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Records `event` now; see [`record_event`]. It decides first from the pointer read without
    /// the lock, so an event long after a move never takes the lock, then tries `install.lock`
    /// every 50 ms for at most 2 s and decides again from the pointer read under it. A record it
    /// cannot write is dropped and logged as `fallback-record-dropped`; a committed fallback is
    /// logged as `fallback` with both versions. Only a build whose switch is on records.
    pub fn record(&self, event: FallbackEvent<'_>) -> FallbackOutcome {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        self.record_at(event, now)
    }

    pub(crate) fn record_at(&self, event: FallbackEvent<'_>, now: u64) -> FallbackOutcome {
        if !automatic_updates_live() {
            return FallbackOutcome::Skipped;
        }
        let read = PrivateCache::read_existing(&self.root, RELEASE_POINTER, RELEASE_POINTER_LIMIT);
        let Some(pointer) = read.ok().flatten().and_then(|bytes| Pointer::parse(&bytes)) else {
            return FallbackOutcome::Skipped;
        };
        if self.decide(&pointer, event, now) == Recorded::Nothing {
            return FallbackOutcome::Skipped;
        }
        match self.write(event, now) {
            Ok(outcome) => outcome,
            Err(error) => {
                log_event(
                    LogLevel::Warning,
                    "fallback-record-dropped",
                    &[
                        ("record", LogValue::Token(event.token())),
                        ("reason", LogValue::from(error.kind())),
                    ],
                );
                FallbackOutcome::Dropped
            }
        }
    }

    fn decide(&self, pointer: &Pointer, event: FallbackEvent<'_>, now: u64) -> Recorded {
        record_event(pointer, &self.version, event, now, |previous| {
            std::fs::symlink_metadata(self.root.join(RELEASE_VERSIONS).join(&previous.product))
                .is_ok_and(|metadata| metadata.is_dir())
        })
    }

    /// Takes the lock within its bound, decides from the pointer read under it and writes.
    fn write(&self, event: FallbackEvent<'_>, now: u64) -> io::Result<FallbackOutcome> {
        let give_up = Instant::now() + LOCK_BOUND;
        let mut lock = loop {
            match PrivateCache::acquire_with_lock(&self.root, "install", "active") {
                Ok(lock) => break lock,
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < give_up =>
                {
                    std::thread::sleep(LOCK_RETRY);
                }
                Err(error) => return Err(error),
            }
        };
        let Some(pointer) = lock
            .read(RELEASE_POINTER, RELEASE_POINTER_LIMIT)?
            .and_then(|bytes| Pointer::parse(&bytes))
        else {
            return Ok(FallbackOutcome::Skipped);
        };
        let (written, outcome) = match self.decide(&pointer, event, now) {
            Recorded::Nothing => return Ok(FallbackOutcome::Skipped),
            Recorded::Written(written) => (written, FallbackOutcome::Recorded),
            Recorded::FellBack { pointer, from, to } => {
                (pointer, FallbackOutcome::FellBack { from, to })
            }
        };
        let bytes = written.encode();
        if bytes.len() > RELEASE_POINTER_LIMIT {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        lock.replace(RELEASE_POINTER, &bytes)?;
        if let FallbackOutcome::FellBack { from, to } = &outcome {
            log_event(
                LogLevel::Warning,
                "fallback",
                &[
                    ("from", LogValue::Token(version_token(&from.product))),
                    ("to", LogValue::Token(version_token(&to.product))),
                    ("role", LogValue::Token(event.role())),
                ],
            );
        }
        Ok(outcome)
    }
}

/// `version` as a log token. The log takes only `'static` tokens; a version is a validated token of
/// the log's grammar, and a process logs at most one fallback per move, so each distinct version is
/// kept once, at most sixteen in all.
fn version_token(version: &str) -> &'static str {
    static KEPT: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    if !valid_version(version) {
        return "invalid";
    }
    let mut kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(token) = kept.iter().find(|token| **token == version) {
        return token;
    }
    if kept.len() >= 16 {
        return "other";
    }
    let token: &'static str = Box::leak(version.to_owned().into_boxed_str());
    kept.push(token);
    token
}

#[cfg(test)]
mod tests;
