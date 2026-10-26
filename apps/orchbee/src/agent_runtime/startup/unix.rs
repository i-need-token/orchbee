//! No Unix Agent-start effect is admitted. No connection, viewer, reservation or input is opened.
use super::Receipt;
use crate::{
    agent_runtime::AgentObservationRead,
    cli::{Failure, FailureKind},
    invocation::Target,
    local_client::{LocalClient, LocalClientOrigin, LocalOperationClock},
    tui_capabilities::{Capability, Unsupported},
};
use orchbee_agent_observation_protocol::AgentSessionId;
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::Snapshot;
use serde_json::Value;
use std::time::Instant;
fn unsupported() -> Failure {
    from_unsupported(Unsupported(Capability::AgentLaunch))
}
pub(crate) fn admit() -> Result<(), Failure> {
    Err(unsupported())
}

pub(crate) fn stream_snapshot(
    _deadline: Instant,
    _wake: Option<LocalIo<'_>>,
) -> Result<(LocalClient, Snapshot), Failure> {
    Err(unsupported())
}

fn from_unsupported(_unsupported: Unsupported) -> Failure {
    Failure::new(FailureKind::ServiceUnavailable).details(crate::cli::stage_reason_details(
        "platform",
        "unsupported-platform",
    ))
}
pub(crate) fn snapshot_from_origin(
    _origin: &LocalClientOrigin,
    _clock: LocalOperationClock,
    _deadline: Instant,
    _wake: Option<LocalIo<'_>>,
) -> Result<(LocalClient, Snapshot), Failure> {
    Err(unsupported())
}

pub(crate) fn read_agent_observation(
    _client: &mut LocalClient,
    _session: &AgentSessionId,
    _deadline: Instant,
    _wake: Option<LocalIo<'_>>,
) -> Result<AgentObservationRead, Failure> {
    Err(unsupported())
}
pub(crate) fn read_terminal_processes_cancellable(
    _client: &mut LocalClient,
    _deadline: Instant,
    _snapshot: &Snapshot,
    _terminal: &Target,
    _wake: Option<LocalIo<'_>>,
) -> Result<(Value, crate::cli::results::PanelProcessInfoDoc), Failure> {
    Err(unsupported())
}
pub(crate) fn submit_prompt_on_session(
    _client: &mut LocalClient,
    _snapshot: &Snapshot,
    _target: &Target,
    _text: &str,
    _codex_boundary: bool,
    _deadline: Instant,
    _wake: Option<LocalIo<'_>>,
) -> Result<Receipt, Failure> {
    Err(unsupported())
}
