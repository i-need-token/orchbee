//! A double-click on a finished Agent's sidebar row tells the Session the person saw it finish, as
//! typing into its terminal does. The Server owns the change: the Agent's next state row shows it,
//! so this owner only keeps each request until its reply or deadline.

use super::WorkbenchClientUi;
use crate::{
    local_client::{LocalClientOrigin, subscription::Request},
    session_capabilities::{SessionAction, serves},
};
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentRequest, PresentationBinding, ProtocolVersion,
    TerminalPresentationId, WorkbenchRequest,
};
use std::time::{Duration, Instant};

/// How long one request waits for its reply. The Server answers at once, so a later answer means
/// the request was lost.
const REPLY: Duration = Duration::from_secs(10);

#[derive(Default)]
pub(super) struct CompletionAcknowledgements {
    origin: Option<LocalClientOrigin>,
    /// Each request still waiting for its reply, with its deadline.
    flights: Vec<(Request, Instant)>,
}

impl CompletionAcknowledgements {
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.origin = Some(origin);
    }

    /// When a request needs a turn without a reply: the earliest deadline.
    pub(super) fn wake(&self) -> Option<Instant> {
        self.flights.iter().map(|(_, deadline)| *deadline).min()
    }

    /// Lets go of each request that has its reply or passed its deadline.
    pub(super) fn drive(&mut self, now: Instant) {
        self.flights
            .retain(|(request, deadline)| request.poll().is_none() && now < *deadline);
    }
}

impl WorkbenchClientUi {
    /// Asks the Session to acknowledge the Agent in `terminal`, which the Server refuses once that
    /// Presentation shows another terminal. Reports whether the request went out.
    pub(in crate::workbench_client) fn acknowledge_agent_completion(
        &mut self,
        presentation: &TerminalPresentationId,
        terminal: &PresentationBinding,
    ) -> bool {
        let (Some(state), Some(origin), Ok(request_id)) = (
            self.state.as_ref(),
            self.completion_acknowledgements.origin.as_ref(),
            crate::machine_observation::request_id(),
        ) else {
            return false;
        };
        if !serves(
            origin.session_capabilities().as_ref(),
            SessionAction::AcknowledgeAgentCompletion,
        ) {
            return false;
        }
        let request = PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: state.cache().session_id().clone(),
            action: PanelAgentAction::CompletionAcknowledge {
                presentation_id: presentation.clone(),
                expected_binding: terminal.clone(),
            },
        };
        let deadline = self.clock.now() + REPLY;
        let Ok(request) = origin.request(
            &WorkbenchRequest::PanelAgent(request),
            deadline,
            self.clock.clone(),
        ) else {
            return false;
        };
        self.completion_acknowledgements
            .flights
            .push((request, deadline));
        true
    }
}
