//! Admission for optional CLI execution adapters, independent of the platform-neutral grammar.

use super::{Failure, FailureKind, stage_reason_details};

#[derive(Clone, Copy)]
pub(super) enum Capability {
    AgentProjection,
    WorkspaceGroupClose,
    WorktreeOperation,
    ClientControl,
    Layout,
    TerminalCreation,
    ExtendedCreation,
    WorkbenchMutation,
    WorkbenchInspection,
    CanonicalPanelProcess,
    Reports,
    AgentManifests,
    RuntimeStatus,
    TerminalContent,
    Plugins,
    ConfiguredCommands,
    NamedSessions,
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
        | Capability::WorkspaceGroupClose
        | Capability::WorktreeOperation
        | Capability::ClientControl
        | Capability::Layout
        | Capability::TerminalCreation
        | Capability::ExtendedCreation
        | Capability::WorkbenchMutation
        | Capability::WorkbenchInspection
        | Capability::CanonicalPanelProcess
        | Capability::Reports
        | Capability::AgentManifests
        | Capability::RuntimeStatus
        | Capability::TerminalContent
        | Capability::Plugins
        | Capability::ConfiguredCommands
        | Capability::NamedSessions => Failure::new(FailureKind::ServiceUnavailable)
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
