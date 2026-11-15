//! Unix refuses extended CLI capabilities before effects. macOS and Linux admit the canonical
//! Workbench reads and mutations, process and Agent reads, Agent names, retained close
//! operations, runtime inspection, terminal input, Agent prompts and the read-only Client view
//! list and get, which their managed Sessions serve.
//! These commands borrow the interrupt-pair port: Linux captures `SIGINT` and `SIGTERM` there,
//! while macOS keeps their inherited action, so an interrupted command ends at once without a
//! document. A declared `terminal send` input source needs its own signal watch, which only
//! Linux has.
//! macOS and Linux admit release installation and uninstallation, but not the update download.
//! macOS admits `server stop`, which its managed Session serves.
//! Admission itself does not inspect environment or native state.
use super::{Capability, Unsupported};

pub(super) fn admit(capability: Capability) -> Result<(), Unsupported> {
    match capability {
        // The release store and its PATH step need no download, which stays Windows-only.
        Capability::ReleaseInstall => Ok(()),
        Capability::ServerStop if cfg!(target_os = "macos") => Ok(()),
        Capability::TerminalInputSource if cfg!(target_os = "linux") => Ok(()),
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
            if cfg!(any(target_os = "linux", target_os = "macos")) =>
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
        | Capability::TerminalInputSource
        | Capability::AgentPrompt
        | Capability::ClientViewRead
        | Capability::TerminalRun
        | Capability::ExtendedWorkbenchMutation
        | Capability::IntegrationOutdated
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
