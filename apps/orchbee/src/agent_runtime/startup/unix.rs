//! Agent launch refuses before any lane, viewer, reservation or input effect.
use super::Unsupported;
pub(crate) fn admit() -> Result<(), Unsupported> {
    Err(Unsupported)
}
