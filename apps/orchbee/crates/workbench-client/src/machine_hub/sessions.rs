//! Whether a local Session of the hub's data root lives, read by the hub for its lifetime and by a
//! client before it replaces an incompatible hub. The read opens no Session record and nothing a
//! starting, stopping or deleting Session writes or replaces: it takes the Session names from the
//! folder names in the runtime base, then probes each folder's existing activation lock and, under
//! it, the lifetime lock, so that it never takes a free lifetime lock at the instant a Server takes
//! it. A read that cannot finish says so, and each caller decides what that means: the hub finds no
//! live Session, and the replacement check counts the hub held.

use orchbee_local_runtime::{
    ModeCheckedLocalSessionRendezvous, SessionHolderObservation, SessionLifetimeObservation,
    SessionScope,
};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::Instant,
};

/// The most Session folders one read probes, as many Sessions as the catalog admits; a base that
/// holds more cannot be read.
const SESSIONS: usize = 256;

/// What one read of the Sessions found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRead {
    /// A Session lives.
    Live,
    /// No Session lives; a base that does not exist yet holds none.
    NoneLive,
    /// The read could not tell: the base could not be listed, it holds more than 256 Session
    /// folders, a probe failed with no live Session found, or the deadline passed.
    Unknown,
}

/// The runtime base Sessions run in under `environment`: `ORCHBEE_RUNTIME_BASE` when it applies to
/// this build, otherwise the data root's default. The hub keeps that variable under the same rule,
/// so a client and the hub it starts read the same base.
pub fn runtime_base(environment: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let routed = orchbee_local_runtime::session_route_environment(|name| environment(name));
    routed("ORCHBEE_RUNTIME_BASE")
        .map(PathBuf::from)
        .or_else(|| orchbee_local_runtime::default_runtime_base(|name| environment(name)))
}

/// Reads whether any Session in `base` lives, within `deadline`.
pub fn read_sessions(base: &Path, deadline: Instant) -> SessionRead {
    let names = match folder_names(base, deadline) {
        Listing::Absent => return SessionRead::NoneLive,
        Listing::Unreadable => return SessionRead::Unknown,
        Listing::Names(names) => names,
    };
    let mut unknown = false;
    for name in names {
        if Instant::now() >= deadline {
            return SessionRead::Unknown;
        }
        // The probe pins the folder only while it lives; a folder that is gone, not private, or
        // not a directory holds no Server.
        let Ok(session) =
            ModeCheckedLocalSessionRendezvous::from_runtime_directory(&base.join(name))
        else {
            continue;
        };
        match session.observe_session_holder() {
            Ok(
                SessionHolderObservation::ActivationHeld
                | SessionHolderObservation::Lifetime(SessionLifetimeObservation::Held),
            ) => return SessionRead::Live,
            Ok(_) => {}
            Err(_) => unknown = true,
        }
    }
    if unknown {
        SessionRead::Unknown
    } else {
        SessionRead::NoneLive
    }
}

/// What listing the runtime base found.
enum Listing {
    /// The base does not exist yet.
    Absent,
    /// The base could not be listed, the listing reached its deadline, or it holds more than
    /// [`SESSIONS`] Session folders.
    Unreadable,
    /// The names of its folders that spell a Session name, read whole before any folder is opened.
    Names(Vec<String>),
}

fn folder_names(base: &Path, deadline: Instant) -> Listing {
    let entries = match std::fs::read_dir(base) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Listing::Absent,
        Err(_) => return Listing::Unreadable,
    };
    let mut names = Vec::new();
    for entry in entries {
        if Instant::now() >= deadline {
            return Listing::Unreadable;
        }
        let Ok(entry) = entry else {
            return Listing::Unreadable;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Some(name) = entry
            .file_name()
            .to_str()
            .filter(|name| SessionScope::parse(name).is_ok())
            .map(str::to_owned)
        else {
            continue;
        };
        if names.len() == SESSIONS {
            return Listing::Unreadable;
        }
        names.push(name);
    }
    Listing::Names(names)
}
