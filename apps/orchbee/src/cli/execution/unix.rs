//! Optional CLI execution is unavailable on Unix; no environment or native state is inspected.
use super::{Capability, Unsupported};

pub(super) fn admit(capability: Capability) -> Result<(), Unsupported> {
    Err(Unsupported(capability))
}
