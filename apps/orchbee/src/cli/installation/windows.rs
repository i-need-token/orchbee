//! Admit the existing Windows installed-release observer without filesystem effects.
use super::Unsupported;

pub(super) fn admit() -> Result<(), Unsupported> {
    Ok(())
}
