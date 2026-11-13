//! Place a full-screen Agent's input area for one recent read; every refusal means unknown.
use super::{HostTerminalId, Instant, LocalClient, ProtocolVersion, Snapshot, TerminalTextFrame};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{PanelAgentAction, PanelAgentOutcome, PanelAgentRequest};

/// The Server's input row together with the one screen it was computed on.
pub(super) struct InputArea {
    binding: AgentLaunchBinding,
    generation: u64,
    revision: u64,
    row: u16,
}

impl InputArea {
    /// The row for exactly the sampled screen: the same Host terminal generation at the same
    /// content revision. Any other frame, however close, gets none.
    pub(super) fn row_for(
        &self,
        host_terminal: &HostTerminalId,
        frame: &TerminalTextFrame,
    ) -> Option<u16> {
        (frame.route.host_epoch.to_text() == self.binding.host_epoch.as_str()
            && host_terminal.to_text() == self.binding.host_terminal_id.as_str()
            && frame.route.generation.get() == self.generation
            && frame.route.revision == self.revision
            && self.row < frame.geometry.rows)
            .then_some(self.row)
    }
}

/// One physical Explain that asks for the input area, sent only to a Session that serves it. A
/// refused, failed or rowless answer is unknown; a broken connection surfaces on the next exchange.
pub(super) fn explain(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    binding: AgentLaunchBinding,
    deadline: Instant,
) -> Option<InputArea> {
    if !client.server_serves(orchbee_connection_handshake::AGENT_INPUT_AREA) {
        return None;
    }
    let request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: super::request_id().ok()?,
        session_id: snapshot.session_id.clone(),
        action: PanelAgentAction::Explain {
            binding: binding.clone(),
            input_area: true,
        },
    };
    let PanelAgentOutcome::Explained { explanation, .. } =
        client.panel_agent(request, deadline, None).ok()?.outcome
    else {
        return None;
    };
    Some(InputArea {
        generation: explanation.sample.frame_generation.value(),
        revision: explanation.sample.frame_revision.value(),
        row: explanation.input_row?,
        binding,
    })
}

#[cfg(test)]
mod tests;
