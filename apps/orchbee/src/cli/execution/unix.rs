//! Optional CLI execution is unavailable on Unix; no environment or native state is inspected.
use super::{Capability, Unsupported};

pub(super) fn admit(capability: Capability) -> Result<(), Unsupported> {
    match capability {
        Capability::WorkbenchMutation if cfg!(target_os = "linux") => Ok(()),
        Capability::WorkbenchMutation
        | Capability::AgentProjection
        | Capability::WorkspaceGroupClose
        | Capability::WorktreeOperation
        | Capability::ClientControl
        | Capability::Layout
        | Capability::TerminalCreation => Err(Unsupported(capability)),
    }
}
