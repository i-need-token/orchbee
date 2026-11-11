//! Corroborate one selected Client's acknowledgement against the physical report owner.
use super::{
    ClientViewControlStatus, ClientViewIntent, ClientViewObservation, Failure, FailureKind,
    Instant, SignalWatch, Snapshot, WorkbenchClientId, WorkbenchRequestId, read_control,
    view_failure,
};
use crate::cli::results::terminal_agent::{ClientAttention, TerminalAgentRead};
use crate::local_client::LocalClient;
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{ClientTerminalAgentAttentionSnapshot, PanelAgentState};

pub(in crate::cli) struct Read {
    client: WorkbenchClientId,
    operation: WorkbenchRequestId,
    view: ClientViewObservation,
    attention: ClientTerminalAgentAttentionSnapshot,
}

pub(in crate::cli) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selected: &WorkbenchClientId,
    binding: &AgentLaunchBinding,
    deadline: Instant,
    signals: &mut impl SignalWatch,
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

impl Read {
    pub(in crate::cli) fn matches_view(&self, view: &ClientViewObservation) -> bool {
        &self.view == view
    }
    pub(in crate::cli) fn corroborate(self, report: &TerminalAgentRead) -> ClientAttention {
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
            PanelAgentState::Idle => Some("idle"),
            PanelAgentState::Done => Some("done"),
            PanelAgentState::Error => Some("error"),
            PanelAgentState::Working => Some("working"),
            PanelAgentState::Blocked => Some("blocked"),
            PanelAgentState::Unknown | PanelAgentState::Opening => Some("unknown"),
        };
        ClientAttention {
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
