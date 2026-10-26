//! Agent launch refuses before any lane, viewer, reservation or input effect.
use super::Unsupported;
/// Refuse Agent launch before a caller creates a lane, viewer, reservation or input submission.
///
/// # Errors
/// Always returns `Unsupported`; this adapter performs no observation or mutation.
pub(crate) fn admit() -> Result<(), Unsupported> {
    Err(Unsupported)
}
