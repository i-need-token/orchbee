//! One read of the Server's identity result for the startup Terminal, on the admitted Client.
use crate::local_client::LocalClient;
use orchbee_agent_observation_protocol::AgentProcessIdentity;
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, Snapshot, TerminalPresentationId,
};
use std::time::Instant;

/// Read the identity of the presentation's current Terminal generation once.
///
/// `None` means no usable result: an exchange failure, a Server refusal, or a reply for another
/// Host Terminal. Startup then keeps its own per-sample evidence; this read is never retried
/// here and never decides the startup verdict alone.
pub(crate) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    presentation: &TerminalPresentationId,
    host_terminal: &str,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Option<AgentProcessIdentity> {
    let request = PanelAgentRequest {
        protocol_version: snapshot.protocol_version,
        request_id: super::workbench_request_id().ok()?,
        session_id: snapshot.session_id.clone(),
        action: PanelAgentAction::Identify {
            presentation_id: presentation.clone(),
        },
    };
    match client.panel_agent(request, deadline, wake).ok()?.outcome {
        PanelAgentOutcome::Identified {
            binding, identity, ..
        } if binding.host_terminal_id.as_str() == host_terminal => Some(identity),
        // Refusals and other variants leave the per-sample evidence in charge.
        _ => None,
    }
}
