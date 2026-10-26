//! Unix refuses extended CLI capabilities before effects; Linux retains its existing Workbench mutations and canonical process reads.
//! Admission itself does not inspect environment or native state.
use super::{Capability, Unsupported};

pub(super) fn admit(capability: Capability) -> Result<(), Unsupported> {
    match capability {
        Capability::WorkbenchMutation | Capability::CanonicalPanelProcess
            if cfg!(target_os = "linux") =>
        {
            Ok(())
        }
        Capability::WorkbenchMutation
        | Capability::CanonicalPanelProcess
        | Capability::Reports
        | Capability::AgentManifests
        | Capability::RuntimeStatus
        | Capability::TerminalContent
        | Capability::Plugins
        | Capability::ConfiguredCommands
        | Capability::NamedSessions
        | Capability::AgentProjection
        | Capability::WorkspaceGroupClose
        | Capability::WorktreeOperation
        | Capability::ClientControl
        | Capability::Layout
        | Capability::WorkbenchInspection
        | Capability::ExtendedCreation
        | Capability::TerminalCreation => Err(Unsupported(capability)),
    }
}
