//! Windows accepts the existing command surface, including a selected Session.
use super::Unsupported;
use crate::invocation::grammar::Availability;

pub(super) fn admit(
    _availability: Availability,
    _session_selected: bool,
) -> Result<(), Unsupported> {
    Ok(())
}
