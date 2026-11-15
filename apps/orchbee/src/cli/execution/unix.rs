//! Unix refuses extended CLI capabilities before effects; Linux retains its existing Workbench mutations and canonical process/Agent reads.
//! macOS and Linux admit release installation and uninstallation, but not the update download.
//! Linux also admits the read-only Client view list and get, which borrow its interrupt-pair port.
//! macOS admits `server stop`, which its managed Session serves.
//! Admission itself does not inspect environment or native state.
use super::{Capability, Unsupported};

pub(super) fn admit(capability: Capability) -> Result<(), Unsupported> {
    match capability {
        // The release store and its PATH step need no download, which stays Windows-only.
        Capability::ReleaseInstall => Ok(()),
        Capability::CanonicalAgentList if cfg!(any(target_os = "linux", target_os = "macos")) => {
            Ok(())
        }
        Capability::ServerStop if cfg!(target_os = "macos") => Ok(()),
        Capability::WorkbenchMutation
        | Capability::CanonicalPanelProcess
        | Capability::CanonicalAgentRead
        | Capability::AgentNames
        | Capability::RetainedCloseOperation
        | Capability::CanonicalWorkbenchRead
        | Capability::RuntimeInspection
        | Capability::TerminalInput
        | Capability::AgentPrompt
        | Capability::ClientViewRead
            if cfg!(target_os = "linux") =>
        {
            Ok(())
        }
        Capability::WorkbenchMutation
        | Capability::CanonicalPanelProcess
        | Capability::CanonicalAgentRead
        | Capability::AgentNames
        | Capability::RetainedCloseOperation
        | Capability::CanonicalWorkbenchRead
        | Capability::RuntimeInspection
        | Capability::TerminalInput
        | Capability::AgentPrompt
        | Capability::ClientViewRead
        | Capability::TerminalRun
        | Capability::ExtendedWorkbenchMutation
        | Capability::IntegrationOutdated
        | Capability::CanonicalAgentList
        | Capability::Reports
        | Capability::AgentManifests
        | Capability::RuntimeStatus
        | Capability::TerminalStream
        | Capability::PaneGraphics
        | Capability::Plugins
        | Capability::ConfiguredCommands
        | Capability::NamedSessions
        | Capability::ReleaseUpdate
        | Capability::ServerStop
        | Capability::TerminalClear
        | Capability::ChannelPreference
        | Capability::Events
        | Capability::SchemaPublication
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
