//! Orchbee answering an Agent's routine confirmations, as this Client shows and changes it. The
//! Server owns every value: rows of its `confirmations` streams say the choice, an open offer, why
//! a confirmation waits and what was last answered. This owner keeps those rows, sends one change
//! at a time and says an answered confirmation once. A choice made here shows only as the Server's
//! answer or its next row, never as what was asked for.

use super::{NoticeContext, WorkbenchClientNotice, WorkbenchClientUi};
use crate::{
    action_menu::{MenuAction, MenuTarget},
    local_client::{LocalClientOrigin, subscription::Request},
    localization::ConfirmationAnswer,
    workbench_projection::ClientTerminalBinding,
    workbench_state::WorkbenchClientState,
};
use orchbee_workbench_protocol::{
    AgentConfirmationChoice, AgentConfirmationView, AgentConfirmationWait, PanelAgentAction,
    PanelAgentOutcome, PanelAgentRequest, PresentationBinding, PresentationConfirmationRow,
    ProtocolVersion, TerminalPresentationId, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// How long one change waits for its reply. The Server applies it under one lock and answers at
/// once, so a later answer means the request was lost.
const REPLY: Duration = Duration::from_secs(10);
/// How long an answered confirmation, a stop or a failed change stays on the status line.
const NOTICE: Duration = Duration::from_secs(4);

#[derive(Default)]
pub(super) struct AgentConfirmations {
    origin: Option<LocalClientOrigin>,
    /// The Presentations whose terminal runs an Agent the Server can answer for.
    rows: HashMap<TerminalPresentationId, Entry>,
    flight: Option<Flight>,
    changed: bool,
}

struct Entry {
    binding: PresentationBinding,
    view: AgentConfirmationView,
    /// The serial of the last answer this Client said, or found already given when it first read
    /// this terminal's row.
    said: u32,
    /// The stop was said, or found already made.
    stop_said: bool,
}

struct Flight {
    request_id: WorkbenchRequestId,
    request: Request,
    presentation: TerminalPresentationId,
    deadline: Instant,
}

impl AgentConfirmations {
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.origin = Some(origin);
    }

    /// When the flight needs a turn without a reply: its deadline.
    pub(super) fn wake(&self) -> Option<Instant> {
        self.flight.as_ref().map(|flight| flight.deadline)
    }

    /// Why the confirmation shown by the Agent in this terminal waits for the person.
    pub(crate) fn wait(
        &self,
        presentation: &TerminalPresentationId,
        terminal: &PresentationBinding,
    ) -> Option<AgentConfirmationWait> {
        self.rows
            .get(presentation)
            .filter(|entry| &entry.binding == terminal)?
            .view
            .wait
    }
}

/// The rows of the menu that answers for one Agent: an offer's three answers, else the three
/// choices.
pub(super) fn menu_actions(offer: bool) -> Vec<MenuAction> {
    let answers = if offer {
        [
            ConfirmationAnswer::Run,
            ConfirmationAnswer::AgentType,
            ConfirmationAnswer::No,
        ]
    } else {
        [
            ConfirmationAnswer::Off,
            ConfirmationAnswer::Run,
            ConfirmationAnswer::AgentType,
        ]
    };
    answers.map(MenuAction::AgentConfirmation).to_vec()
}

/// The answer that names the choice in force.
pub(super) const fn applied(choice: AgentConfirmationChoice) -> ConfirmationAnswer {
    match choice {
        AgentConfirmationChoice::Off => ConfirmationAnswer::Off,
        AgentConfirmationChoice::Run => ConfirmationAnswer::Run,
        AgentConfirmationChoice::AgentType => ConfirmationAnswer::AgentType,
    }
}

/// The Presentation each Panel of the active Tab shows, each with whether its Panel is focused.
fn shown(
    state: &WorkbenchClientState,
) -> impl Iterator<Item = (bool, &TerminalPresentationId)> + '_ {
    let tab = state
        .active()
        .and_then(|active| state.cache().tabs().get(active.index()));
    let focus = state.focus_panel_id();
    tab.into_iter().flat_map(move |tab| {
        (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .map(move |panel| {
                (
                    Some(panel.panel_id()) == focus,
                    state.selected_presentation(panel).presentation_id(),
                )
            })
    })
}

impl WorkbenchClientUi {
    /// The Server's view of the Agent in `presentation`, while this Client's Workbench binds the
    /// Presentation to the terminal the row was read for.
    pub(super) fn agent_confirmation(
        &self,
        presentation: &TerminalPresentationId,
    ) -> Option<&AgentConfirmationView> {
        let entry = self.agent_confirmations.rows.get(presentation)?;
        let current = self.agent_index.binding(presentation).flatten()?;
        (current == &ClientTerminalBinding::from(&entry.binding)).then_some(&entry.view)
    }

    /// The Agent the confirmations key answers for: one in the active Tab with an open offer, else
    /// the focused Panel's. The flag says whether its offer is open.
    pub(super) fn confirmation_subject(&self) -> Option<(TerminalPresentationId, bool)> {
        if self.retirement.is_some() {
            return None;
        }
        let state = self.state.as_ref()?;
        let offering = |id: &TerminalPresentationId| {
            self.agent_confirmation(id).is_some_and(|view| view.offer)
        };
        // The focused Panel's own offer comes before another Panel's.
        let mut panels: Vec<_> = shown(state).collect();
        panels.sort_by_key(|(focused, _)| !focused);
        panels
            .iter()
            .find(|(_, id)| offering(id))
            .map(|(_, id)| ((*id).clone(), true))
            .or_else(|| {
                let (_, id) = panels.first().filter(|(focused, _)| *focused)?;
                self.agent_confirmation(id)?;
                Some(((*id).clone(), false))
            })
    }

    /// The report label of the Agent whose offer the status line states.
    pub(super) fn confirmation_offer(&self) -> Option<&str> {
        if self.retirement.is_some() {
            return None;
        }
        let (presentation, offer) = self.confirmation_subject()?;
        self.agent_confirmation(&presentation)
            .filter(|_| offer)
            .map(|view| view.agent.as_str())
    }

    /// The menu entry that answers for `presentation`, or for the key's subject without one.
    pub(super) fn confirmation_menu(
        &self,
        presentation: Option<TerminalPresentationId>,
        offer: bool,
    ) -> Option<MenuTarget> {
        let (presentation, offer) = match presentation {
            Some(presentation) => (presentation, offer),
            None => self.confirmation_subject()?,
        };
        self.agent_confirmation(&presentation)?;
        Some(MenuTarget::AgentConfirmations {
            presentation: Some(presentation),
            offer,
        })
    }

    /// Stops following: another Session's rows, or a Server that no longer serves them.
    pub(in crate::workbench_client) fn forget_agent_confirmations(&mut self) {
        let confirmations = &mut self.agent_confirmations;
        confirmations.changed |= !confirmations.rows.is_empty();
        confirmations.rows.clear();
        confirmations.flight = None;
    }

    /// Rows of the Server's `confirmations` streams. Reports whether anything shown changed.
    pub(in crate::workbench_client) fn receive_agent_confirmations(
        &mut self,
        rows: Vec<PresentationConfirmationRow>,
    ) -> bool {
        if rows.is_empty() || self.retirement.is_some() {
            return false;
        }
        let mut changed = false;
        let mut said = None;
        for row in rows {
            let (Some(binding), Some(view)) = (row.binding, row.confirmation) else {
                changed |= self
                    .agent_confirmations
                    .rows
                    .remove(&row.presentation_id)
                    .is_some();
                continue;
            };
            let prior = self
                .agent_confirmations
                .rows
                .get(&row.presentation_id)
                .filter(|entry| entry.binding == binding);
            if prior.is_some_and(|entry| entry.view == view) {
                continue;
            }
            let serial = view.pressed.as_ref().map_or(0, |pressed| pressed.serial);
            // What the Server did before this Client first read the terminal's row is not news.
            let (answered, stopped) = prior.map_or((false, false), |entry| {
                (serial > entry.said, view.stopped && !entry.stop_said)
            });
            if stopped {
                said = Some((WorkbenchClientNotice::ConfirmationStopped, None));
            } else if answered
                && let Some(pressed) = &view.pressed
                && self.state.as_ref().is_some_and(|state| {
                    self.agent_index
                        .binding(&row.presentation_id)
                        .flatten()
                        .is_some_and(|current| current == &ClientTerminalBinding::from(&binding))
                        && shown(state).any(|(_, id)| id == &row.presentation_id)
                })
            {
                said = Some((
                    WorkbenchClientNotice::ConfirmationAllowed(pressed.kind),
                    Some(pressed.text.clone()),
                ));
            }
            let stop_said = view.stopped;
            self.agent_confirmations.rows.insert(
                row.presentation_id,
                Entry {
                    binding,
                    view,
                    said: serial,
                    stop_said,
                },
            );
            changed = true;
        }
        // The line holds one notice; a batch's last answer stands for it. A newer answer replaces
        // an older one, and nothing else the person has not had time to read.
        let now = self.clock.now();
        if let Some((notice, text)) = said
            && self
                .command_notice
                .current(now)
                .is_none_or(|shown| matches!(shown, WorkbenchClientNotice::ConfirmationAllowed(_)))
        {
            if let Some(text) = text {
                self.command_notice
                    .set_context(NoticeContext::Confirmation(text));
            }
            self.command_notice.set(Some((notice, now + NOTICE)));
        }
        changed
    }

    /// Sends one answer about the Agent in `presentation`. Its outcome arrives on a later turn.
    pub(super) fn answer_agent_confirmation(
        &mut self,
        presentation: &TerminalPresentationId,
        answer: ConfirmationAnswer,
    ) {
        let now = self.clock.now();
        if !self.send_agent_confirmation(presentation, answer, now) {
            self.note_confirmation_not_changed(now);
        }
    }

    fn send_agent_confirmation(
        &mut self,
        presentation: &TerminalPresentationId,
        answer: ConfirmationAnswer,
        now: Instant,
    ) -> bool {
        if self.agent_confirmations.flight.is_some()
            || self.agent_confirmation(presentation).is_none()
        {
            return false;
        }
        let (Some(state), Some(origin), Some(entry), Ok(request_id)) = (
            self.state.as_ref(),
            self.agent_confirmations.origin.as_ref(),
            self.agent_confirmations.rows.get(presentation),
            crate::machine_observation::request_id(),
        ) else {
            return false;
        };
        // The terminal the person saw the choice for; the Server refuses any other.
        let expected_binding = entry.binding.clone();
        let presentation_id = presentation.clone();
        let choose = |choice| PanelAgentAction::ConfirmationChoose {
            presentation_id: presentation_id.clone(),
            expected_binding: expected_binding.clone(),
            choice,
        };
        let action = match answer {
            ConfirmationAnswer::Off => choose(AgentConfirmationChoice::Off),
            ConfirmationAnswer::Run => choose(AgentConfirmationChoice::Run),
            ConfirmationAnswer::AgentType => choose(AgentConfirmationChoice::AgentType),
            ConfirmationAnswer::No => PanelAgentAction::ConfirmationDecline {
                presentation_id: presentation_id.clone(),
                expected_binding: expected_binding.clone(),
            },
        };
        let request = PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            session_id: state.cache().session_id().clone(),
            action,
        };
        let deadline = now + REPLY;
        let Ok(request) = origin.request(
            &WorkbenchRequest::PanelAgent(request),
            deadline,
            self.clock.clone(),
        ) else {
            return false;
        };
        self.agent_confirmations.flight = Some(Flight {
            request_id,
            request,
            presentation: presentation.clone(),
            deadline,
        });
        true
    }

    /// Settles the flight's reply and reports whether anything shown changed.
    pub(super) fn drive_agent_confirmations(&mut self) -> bool {
        let mut changed = std::mem::take(&mut self.agent_confirmations.changed);
        if !self.agent_confirmations.rows.is_empty() {
            // A Presentation that left the Workbench has no row to say so.
            let index = &self.agent_index;
            let before = self.agent_confirmations.rows.len();
            self.agent_confirmations
                .rows
                .retain(|presentation, _| index.binding(presentation).is_some());
            changed |= self.agent_confirmations.rows.len() != before;
        }
        let Some(reply) = self
            .agent_confirmations
            .flight
            .as_ref()
            .and_then(|flight| flight.request.poll())
        else {
            return changed;
        };
        let Some(flight) = self.agent_confirmations.flight.take() else {
            return changed;
        };
        match reply {
            Ok(WorkbenchResponse::PanelAgent(result))
                if result.request_id == flight.request_id
                    && matches!(
                        &result.outcome,
                        PanelAgentOutcome::ConfirmationChosen { presentation }
                            if presentation.presentation_id == flight.presentation
                    ) =>
            {
                if let PanelAgentOutcome::ConfirmationChosen { presentation } = result.outcome {
                    self.receive_agent_confirmations(vec![*presentation]);
                }
            }
            Ok(_) | Err(_) => self.note_confirmation_not_changed(self.clock.now()),
        }
        true
    }

    fn note_confirmation_not_changed(&mut self, now: Instant) {
        self.command_notice.set(Some((
            WorkbenchClientNotice::ConfirmationNotChanged,
            now + NOTICE,
        )));
    }
}

#[cfg(test)]
mod tests;
