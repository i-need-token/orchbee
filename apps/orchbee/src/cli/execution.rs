//! Admission for optional CLI execution adapters, independent of the platform-neutral grammar.

use super::{Failure, FailureKind, stage_reason_details};

#[derive(Clone, Copy)]
pub(super) enum Capability {
    AgentProjection,
    /// `agent list` and `agent get` over the canonical Agent sessions, and `agent wait`.
    CanonicalAgentRead,
    AgentNames,
    RetainedCloseOperation,
    CanonicalWorkbenchRead,
    RuntimeInspection,
    TerminalInput,
    /// A declared `terminal send --file` or `--stdin` source, read under its own signal watch.
    TerminalInputSource,
    AgentPrompt,
    IntegrationOutdated,
    TerminalRun,
    WorkspaceGroupClose,
    WorktreeOperation,
    ClientViewRead,
    ClientControl,
    Layout,
    TerminalCreation,
    ExtendedCreation,
    WorkbenchMutation,
    ExtendedWorkbenchMutation,
    WorkbenchInspection,
    CanonicalPanelProcess,
    Reports,
    AgentManifests,
    RuntimeStatus,
    TerminalStream,
    PaneGraphics,
    Plugins,
    ConfiguredCommands,
    NamedSessions,
    /// `install`, `uninstall` and the completion helper: the release store, the PATH step and
    /// their inverse, without a download.
    ReleaseInstall,
    /// `update`, the automatic helper and channel changes, which download.
    ReleaseUpdate,
    ServerStop,
    TerminalClear,
    ChannelPreference,
    Events,
    SchemaPublication,
    /// `diagnostics create`: the diagnostic bundle, written where the person chooses.
    DiagnosticBundle,
}

pub(super) struct Unsupported(Capability);

#[cfg(windows)]
#[path = "execution/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "execution/unix.rs"]
mod platform;

pub(super) fn admit(capability: Capability) -> Result<(), Failure> {
    platform::admit(capability).map_err(|Unsupported(capability)| match capability {
        Capability::AgentProjection
        | Capability::CanonicalAgentRead
        | Capability::AgentNames
        | Capability::RetainedCloseOperation
        | Capability::CanonicalWorkbenchRead
        | Capability::RuntimeInspection
        | Capability::TerminalInput
        | Capability::TerminalInputSource
        | Capability::AgentPrompt
        | Capability::IntegrationOutdated
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
        | Capability::Plugins
        | Capability::ConfiguredCommands
        | Capability::NamedSessions
        | Capability::ReleaseInstall
        | Capability::ReleaseUpdate
        | Capability::ServerStop
        | Capability::TerminalClear
        | Capability::ChannelPreference
        | Capability::Events
        | Capability::SchemaPublication
        | Capability::DiagnosticBundle => Failure::new(FailureKind::ServiceUnavailable)
            .details(stage_reason_details("platform", "unsupported-platform")),
    })
}

/// Canonical Unix selectors remain available; only projection selectors need this port.
pub(super) fn admit_agent_target(target: &crate::invocation::AgentTarget) -> Result<(), Failure> {
    match target {
        crate::invocation::AgentTarget::Projection { .. } => admit(Capability::AgentProjection),
        crate::invocation::AgentTarget::Session { .. }
        | crate::invocation::AgentTarget::Name { .. }
        | crate::invocation::AgentTarget::Panel { .. }
        | crate::invocation::AgentTarget::Terminal { .. }
        | crate::invocation::AgentTarget::Auto { .. } => Ok(()),
    }
}
