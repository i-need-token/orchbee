//! Windows accepts the existing command surface and preserves selected-Session initialization.
use super::Unsupported;
use crate::invocation::grammar::Availability;
use orchbee_local_runtime::SessionName;

pub(super) fn admit(
    _availability: Availability,
    _session_selected: bool,
) -> Result<(), Unsupported> {
    Ok(())
}

pub(super) fn select_session(session: Option<SessionName>) {
    crate::local_activation::select_session(session);
}
