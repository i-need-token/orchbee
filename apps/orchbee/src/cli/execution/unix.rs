//! macOS and Linux admit every CLI capability but the Windows-only plugins and update channel
//! preference, which are refused before effects. Admission itself does not inspect environment or
//! native state.
use super::{Capability, Unsupported};

pub(super) fn admit(capability: Capability) -> Result<(), Unsupported> {
    match capability {
        Capability::AgentProjection
        | Capability::CanonicalAgentRead
        | Capability::RetainedCloseOperation
        | Capability::CanonicalWorkbenchRead
        | Capability::RuntimeInspection
        | Capability::TerminalInput
        | Capability::TerminalInputSource
        | Capability::AgentPrompt
        | Capability::TerminalRun
        | Capability::WorkspaceGroupClose
        | Capability::WorktreeOperation
        | Capability::ClientViewRead
        | Capability::ClientControl
        | Capability::Layout
        | Capability::TerminalCreation
        | Capability::ExtendedCreation
        | Capability::WorkbenchMutation
        | Capability::ExtendedWorkbenchMutation
        | Capability::WorkbenchInspection
        | Capability::CanonicalPanelProcess
        | Capability::Reports
        | Capability::AgentManifests
        | Capability::RuntimeStatus
        | Capability::TerminalStream
        | Capability::PaneGraphics
        | Capability::ConfiguredCommands
        | Capability::NamedSessions
        | Capability::ReleaseInstall
        | Capability::ReleaseUpdate
        | Capability::ServerStop
        | Capability::ServerRestart
        | Capability::TerminalClear
        | Capability::Events
        | Capability::SchemaPublication
        | Capability::DiagnosticBundle => Ok(()),
        Capability::Plugins | Capability::ChannelPreference => Err(Unsupported(capability)),
    }
}
