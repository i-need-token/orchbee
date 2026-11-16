//! Unix refuses extended CLI capabilities before effects. macOS and Linux admit the canonical
//! Workbench reads and mutations, process and Agent reads, Agent names, retained close
//! operations, runtime inspection, terminal input, Agent prompts and the read-only Client view
//! list and get, which their managed Sessions serve.
//! These commands borrow the interrupt-pair port, which captures `SIGINT` and `SIGTERM` on both,
//! and both admit a declared `terminal send` input source under its own signal watch.
//! macOS and Linux admit release installation and uninstallation; only macOS admits the update
//! download, `update` and the automatic helper. macOS admits `server stop`, Worktree operations
//! and source queries, `project close-all`, the unified Agent projection with per-Client
//! attention, Client control, configured commands, Panel graphics, the runtime status reads
//! with the processes its managed Session reports, and the diagnostic bundle.
//! Admission itself does not inspect environment or native state.
use super::{Capability, Unsupported};

pub(super) fn admit(capability: Capability) -> Result<(), Unsupported> {
    match capability {
        // The release store and its PATH step need no download; the download runs on macOS.
        Capability::ReleaseInstall => Ok(()),
        Capability::ServerStop
        | Capability::WorktreeOperation
        | Capability::WorkspaceGroupClose
        | Capability::AgentProjection
        | Capability::ClientControl
        | Capability::ReleaseUpdate
        | Capability::ConfiguredCommands
        | Capability::RuntimeStatus
        | Capability::PaneGraphics
        | Capability::DiagnosticBundle
            if cfg!(target_os = "macos") =>
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
        | Capability::TerminalInputSource
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
        | Capability::TerminalCreation
        | Capability::DiagnosticBundle => Err(Unsupported(capability)),
    }
}
