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
//! with the processes its managed Session reports, the diagnostic bundle, schema export, the
//! Agent detection rule commands, the Panel, Workspace,
//! notification, title and configuration reports, the extended creation and move options, the
//! enriched Workbench inspection, `terminal run` with its managed-shell wait, `terminal clear`,
//! `panel create-terminal`, layout export and apply, the terminal streams: `terminal attach`,
//! `agent attach` and `terminal session observe/control`, the named-Session commands
//! `session list/attach/stop/delete`, and `events subscribe/wait`.
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
        | Capability::SchemaPublication
        | Capability::AgentManifests
        | Capability::Reports
        | Capability::ExtendedCreation
        | Capability::ExtendedWorkbenchMutation
        | Capability::WorkbenchInspection
        | Capability::TerminalRun
        | Capability::TerminalClear
        | Capability::TerminalCreation
        | Capability::Layout
        | Capability::TerminalStream
        | Capability::NamedSessions
        | Capability::Events
            if cfg!(target_os = "macos") =>
        {
            Ok(())
        }
        Capability::WorkbenchMutation
        | Capability::CanonicalPanelProcess
        | Capability::CanonicalAgentRead
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
        | Capability::RetainedCloseOperation
        | Capability::CanonicalWorkbenchRead
        | Capability::RuntimeInspection
        | Capability::TerminalInput
        | Capability::TerminalInputSource
        | Capability::AgentPrompt
        | Capability::ClientViewRead
        | Capability::TerminalRun
        | Capability::ExtendedWorkbenchMutation
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
