//! Corroborate one selected Client's acknowledgement against the physical report owner.
use super::{
    ArmedSignals, ClientViewControlStatus, ClientViewIntent, ClientViewObservation, Failure,
    FailureKind, Instant, Snapshot, WorkbenchClientId, WorkbenchRequestId, read_control,
    view_failure,
};
use crate::cli::source::SignalWatch;
use crate::{cli::query::terminal_agent::TerminalAgentRead, local_client::LocalClient};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{
    ClientTerminalAgentAttentionSnapshot, PanelAgentState, WorkbenchRevision,
};

pub(in crate::cli) struct Read {
    client: WorkbenchClientId,
    operation: WorkbenchRequestId,
    view: ClientViewObservation,
    attention: ClientTerminalAgentAttentionSnapshot,
}

#[derive(Clone, Debug)]
pub(crate) struct Projection {
    pub(in crate::cli) client_id: WorkbenchClientId,
    operation_id: WorkbenchRequestId,
    view: ClientViewObservation,
    report_revision: WorkbenchRevision,
    sequence: Option<WorkbenchRevision>,
    pub(in crate::cli) seen: Option<bool>,
    pub(in crate::cli) status: Option<&'static str>,
    outer_focused: Option<bool>,
}
impl Projection {
    pub(in crate::cli) fn document(&self) -> serde_json::Value {
        serde_json::json!({"clientId":self.client_id,"operationId":self.operation_id,"view":self.view,
            "reportRevision":self.report_revision,"sequence":self.sequence,"seen":self.seen,
            "status":self.status,"outerFocused":self.outer_focused})
    }
}

pub(in crate::cli) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selected: &WorkbenchClientId,
    binding: &AgentLaunchBinding,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Read, Failure> {
    let (operation, status) = read_control(
        client,
        snapshot,
        selected,
        ClientViewIntent::TerminalAgentAttention {
            bindings: vec![binding.clone()],
        },
        deadline,
        signals,
    )?;
    let ClientViewControlStatus::TerminalAttentionRead { attention, view } = status else {
        return Err(view_failure(FailureKind::ProtocolInvalid));
    };
    if !attention.answers(std::slice::from_ref(binding)) {
        return Err(view_failure(FailureKind::ProtocolInvalid));
    }
    Ok(Read {
        client: selected.clone(),
        operation,
        view,
        attention: *attention,
    })
}

pub(in crate::cli) fn select_for_wait(
    client: &mut LocalClient,
    explicit: Option<&WorkbenchClientId>,
    until: &[crate::invocation::AgentWaitCondition],
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<WorkbenchClientId>, Failure> {
    if let Some(selected) = explicit {
        return Ok(Some(selected.clone()));
    }
    if !until
        .iter()
        .all(|c| *c == crate::invocation::AgentWaitCondition::Done)
    {
        return Ok(None);
    }
    let snapshot = crate::cli::query::read_agent_snapshot(client, deadline, signals.poll_fd())?;
    let caller =
        crate::cli::caller_context::PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|caller| caller.resolve(&snapshot))
            .transpose()?;
    let reply = super::exchange(
        client,
        &snapshot.site_id,
        orchbee_workbench_protocol::ClientViewAction::List {},
        deadline,
        signals,
    )?;
    let clients = match reply.outcome {
        orchbee_workbench_protocol::ClientViewOutcome::Listed { clients } => clients,
        orchbee_workbench_protocol::ClientViewOutcome::Denied {} => {
            return Err(view_failure(FailureKind::Denied));
        }
        orchbee_workbench_protocol::ClientViewOutcome::Unavailable { .. } => {
            return Err(view_failure(FailureKind::ServiceUnavailable));
        }
        _ => return Err(view_failure(FailureKind::ProtocolInvalid)),
    };
    Ok(Some(
        crate::cli::client_control::selection::select(&clients, None, caller.as_ref())?
            .client_id
            .clone(),
    ))
}
impl Read {
    pub(in crate::cli) fn corroborate(self, report: &TerminalAgentRead) -> Projection {
        let version = self
            .attention
            .entries
            .first()
            .filter(|entry| entry.binding == report.observation.binding)
            .and_then(|entry| entry.state.as_ref())
            .filter(|version| {
                version.sequence == report.observation.sequence
                    && self.attention.report_revision <= report.report_revision
            });
        let seen = version.map(|v| v.seen);
        let status = match report.observation.view.state {
            PanelAgentState::Idle => seen.map(|seen| if seen { "idle" } else { "done" }),
            PanelAgentState::Working => Some("working"),
            PanelAgentState::Blocked => Some("blocked"),
            PanelAgentState::Unknown => Some("unknown"),
        };
        Projection {
            client_id: self.client,
            operation_id: self.operation,
            view: self.view,
            report_revision: self.attention.report_revision,
            sequence: version.map(|v| v.sequence),
            seen,
            status,
            outer_focused: self.attention.outer_focused,
        }
    }
}

#[cfg(test)]
mod tests;
