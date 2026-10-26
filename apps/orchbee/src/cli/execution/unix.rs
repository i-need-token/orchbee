//! Unix refuses extended CLI capabilities before effects; Linux retains its existing Workbench mutations.
//! Admission itself does not inspect environment or native state.
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
        | Capability::WorkbenchInspection
        | Capability::TerminalCreation => Err(Unsupported(capability)),
    }
}
