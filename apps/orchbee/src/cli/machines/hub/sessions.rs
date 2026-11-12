//! Whether a local Session of the hub's data root lives: one read over every Session `session list`
//! shows in the environment the hub started in, each under its existing activation lock so that
//! the read never takes a free lifetime lock at the instant a Server takes it. A Session marked
//! `deleting` is never opened: its deleter holds both locks, so no Server lives there, and a handle
//! of the hub's could fail the folder removal midway.

use crate::local_activation;
use orchbee_local_runtime::{
    DataRoot, ModeCheckedLocalSessionRendezvous, SessionCatalog, SessionHolderObservation,
    SessionLifetimeObservation,
};
use std::time::Instant;

/// Whether any Session lives; a read that fails or reaches `deadline` finds none.
pub(super) fn any_live(deadline: Instant) -> bool {
    let Ok((base, _)) = local_activation::runtime_location() else {
        return false;
    };
    if local_activation::runtime_base_unstarted().unwrap_or(true) {
        return false;
    }
    let Ok(root) = DataRoot::resolve(|name| std::env::var_os(name)) else {
        return false;
    };
    let Ok(entries) = SessionCatalog::list(&base, &root, deadline) else {
        return false;
    };
    for entry in entries.into_iter().filter(|entry| !entry.deleting) {
        if Instant::now() >= deadline {
            return false;
        }
        // A missing or unusable runtime folder holds no Server.
        let Ok(session) = ModeCheckedLocalSessionRendezvous::from_runtime_directory(&entry.runtime)
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
