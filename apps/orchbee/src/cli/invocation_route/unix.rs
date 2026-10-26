//! Unix retains existing command admission and never installs a named-Session override.
use super::Unsupported;
use crate::invocation::grammar::Availability;
use orchbee_local_runtime::SessionName;

pub(super) fn admit(availability: Availability, session_selected: bool) -> Result<(), Unsupported> {
    if availability == Availability::Windows || session_selected {
        Err(Unsupported)
    } else {
        Ok(())
    }
}

pub(super) fn select_session(_session: Option<SessionName>) {
    // Admission rejects runtime Session selection. Offline discovery may carry a selector,
    // which has always been ignored here and must not initialize native runtime state.
}
