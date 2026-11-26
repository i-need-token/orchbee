//! macOS admits portable commands, saved machines among them, the Windows and macOS commands such
//! as named Sessions, and a selected Session. Linux admits portable commands only and refuses a
//! selected Session.
use super::Unsupported;
use crate::invocation::grammar::Availability;

pub(super) fn admit(availability: Availability, session_selected: bool) -> Result<(), Unsupported> {
    let admitted = match availability {
        Availability::Portable => true,
        Availability::Windows => false,
        Availability::WindowsAndMacos => cfg!(target_os = "macos"),
    };
    if admitted && (!session_selected || cfg!(target_os = "macos")) {
        Ok(())
    } else {
        Err(Unsupported)
    }
}
