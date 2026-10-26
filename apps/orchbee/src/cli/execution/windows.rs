//! The existing Windows CLI adapters admit these command capabilities before effects.
use super::{Capability, Unsupported};

pub(super) fn admit(_capability: Capability) -> Result<(), Unsupported> {
    Ok(())
}
