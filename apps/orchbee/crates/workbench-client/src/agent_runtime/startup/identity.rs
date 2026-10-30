//! One read of the Server's identity result for the startup Terminal, on the admitted Client.
use super::ServerIdentity;
use crate::local_client::LocalClient;
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, Snapshot, TerminalPresentationId,
};
use std::time::Instant;

/// Read the identity of the presentation's current Terminal generation once.
///
/// `None` means no usable result: an exchange failure, a Server refusal, or a reply for another
/// Host Terminal. Startup then selects no foreground for that step; this read is never retried
/// here and never decides the startup verdict alone.
pub(crate) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    presentation: &TerminalPresentationId,
    host_terminal: &str,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Option<ServerIdentity> {
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
            binding,
            identity,
            anchor,
            ..
        } if binding.host_terminal_id.as_str() == host_terminal => {
            Some(ServerIdentity { identity, anchor })
        }
        // Refusals and other variants leave this step without a Server foreground.
        _ => None,
    }
}
