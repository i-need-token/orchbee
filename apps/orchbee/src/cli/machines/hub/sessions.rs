//! Whether a local Session of the hub's data root lives. The read opens no Session record and
//! nothing a starting, stopping or deleting Session writes or replaces: it takes the Session names
//! from the folder names in the runtime base, then probes each folder's existing activation lock
//! and, under it, the lifetime lock, so that it never takes a free lifetime lock at the instant a
//! Server takes it.

use crate::local_activation;
use orchbee_local_runtime::{
    ModeCheckedLocalSessionRendezvous, SessionHolderObservation, SessionLifetimeObservation,
    SessionScope,
};
use std::{path::Path, time::Instant};

/// The most Session folders one read probes, as many Sessions as the catalog admits; a base that
/// holds more reads as holding no live Session.
const SESSIONS: usize = 256;

/// Whether any Session lives; a read that fails or reaches `deadline` finds none.
pub(super) fn any_live(deadline: Instant) -> bool {
    let Ok((base, _)) = local_activation::runtime_location() else {
        return false;
    };
    if local_activation::runtime_base_unstarted().unwrap_or(true) {
        return false;
    }
    let Some(names) = folder_names(&base, deadline) else {
        return false;
    };
    for name in names {
        if Instant::now() >= deadline {
            return false;
        }
        // The probe pins the folder only while it lives; a folder that is gone, not private, or
        // not a directory holds no Server.
        let Ok(session) =
            ModeCheckedLocalSessionRendezvous::from_runtime_directory(&base.join(name))
        else {
            continue;
        };
        if matches!(
            session.observe_session_holder(),
            Ok(SessionHolderObservation::ActivationHeld
                | SessionHolderObservation::Lifetime(SessionLifetimeObservation::Held))
        ) {
            return true;
        }
    }
    false
}

/// The names of `base`'s folders that spell a Session name, read whole before any folder is
/// opened; `None` when the base cannot be listed, the listing reaches `deadline`, or it holds more
/// than [`SESSIONS`] of them.
fn folder_names(base: &Path, deadline: Instant) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(base).ok()? {
        if Instant::now() >= deadline {
            return None;
        }
        let entry = entry.ok()?;
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
            return None;
        }
        names.push(name);
    }
    Some(names)
}
