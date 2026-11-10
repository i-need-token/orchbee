//! One read of the Server's published Panel Agent state for the startup Terminal, on the admitted
//! Client.
use crate::local_client::LocalClient;
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, PanelAgentState, Snapshot,
    TerminalPresentationId,
};
use std::time::Instant;

/// The published state and the Agent it names for the presentation's current Terminal generation.
pub(crate) struct Published {
    pub agent: Option<String>,
    pub state: PanelAgentState,
}

/// Read the presentation's published Agent state once.
///
/// `None` means no usable result: an exchange failure, a Server refusal, no current physical
/// observation, or a checkpoint for another Terminal generation than `binding`. This read is never
/// retried here and never decides the startup verdict alone.
pub(crate) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    presentation: &TerminalPresentationId,
    binding: &AgentLaunchBinding,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Option<Published> {
    let request = PanelAgentRequest {
        protocol_version: snapshot.protocol_version,
        request_id: super::workbench_request_id().ok()?,
        session_id: snapshot.session_id.clone(),
        action: PanelAgentAction::States {
            presentation_ids: vec![presentation.clone()],
        },
    };
    let PanelAgentOutcome::States { presentations, .. } =
        client.panel_agent(request, deadline, wake).ok()?.outcome
    else {
        // Refusals and other variants leave this step without a published state.
        return None;
    };
    let mut rows = presentations
        .into_iter()
        .filter(|row| &row.presentation_id == presentation);
    let (Some(row), None) = (rows.next(), rows.next()) else {
        return None;
    };
    row.snapshot
        .filter(|checkpoint| &checkpoint.binding == binding)
        .map(|checkpoint| Published {
            agent: checkpoint.agent,
            state: checkpoint.state,
        })
}
