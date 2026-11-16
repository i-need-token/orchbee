//! Unix admits portable commands, and saved machines on macOS, and never installs a named-Session
//! override.
use super::Unsupported;
use crate::invocation::grammar::Availability;
use orchbee_local_runtime::SessionName;

pub(super) fn admit(availability: Availability, session_selected: bool) -> Result<(), Unsupported> {
    let admitted = match availability {
        Availability::Portable => true,
        Availability::Windows => false,
        Availability::WindowsAndMacos => cfg!(target_os = "macos"),
    };
    if admitted && !session_selected {
        Ok(())
    } else {
        Err(Unsupported)
    }
}

pub(super) fn select_session(_session: Option<SessionName>) {
    // Admission rejects runtime Session selection. Offline discovery may carry a selector,
    // which has always been ignored here and must not initialize native runtime state.
}
