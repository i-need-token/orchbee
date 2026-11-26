//! macOS and Linux admit portable commands, saved machines and named Sessions among them, and a
//! selected Session; Windows-only commands are refused.
use super::Unsupported;
use crate::invocation::grammar::Availability;

pub(super) fn admit(availability: Availability) -> Result<(), Unsupported> {
    match availability {
        Availability::Portable => Ok(()),
        Availability::Windows => Err(Unsupported),
    }
}
