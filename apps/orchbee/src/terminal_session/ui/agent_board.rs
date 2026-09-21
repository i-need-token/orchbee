//! The `NativeUi` side of the Agent board: the toggle, input capture,
//! snapshot-index maintenance, the per-tick drive (lane drain, intent
//! issue, jump stages), and the staged cross-context jump that reuses the
//! existing context-selection, switch, and local panel-focus admissions.
//!
//! Every stage re-resolves stable identities against live state: the
//! attachment's `presentation_id` against the freshest index, the owning
//! context against the navigation list, the tab against the projected
//! cache, and the panel against the live pane records. Nothing moves on
//! cached positions, and no stage ever mints a panel.

use super::super::agent_board::{
    AgentBoardDraw, AgentBoardKeyAction, AgentBoardSummary, JumpStage, JumpStep,
    NativeAgentBoardView, board_key_action, plan_jump_step,
};
use super::*;
use crate::client_view::KeyEvent;
use crate::terminal_session::workbench_projection::NativeContextLocation;
use orchbee_agent_observation_protocol::AgentSessionId;
use std::time::Instant;

impl NativeUi {
    #[cfg(target_os = "linux")]
    pub(in crate::terminal_session) fn set_agent_board_origin(
        &mut self,
        origin: crate::local_client::LocalClientOrigin,
    ) {
        self.board.set_origin(origin);
    }

    /// `Ctrl+B g`. Opening queues the first read and refreshes the
    /// Workbench index best-effort; `DismissNavigator` reports the open
    /// so the client clears whatever overlay held the slot before it.
    /// A modal surface or armed confirmation owning input refuses the
    /// open — the apply-layer check backs the dispatcher's capture order.
    pub(super) fn toggle_agent_board(&mut self) -> SessionActionDisposition {
        if self.retirement_failure.is_some() {
            return SessionActionDisposition::KeepNavigator;
        }
        if self.board.is_open() {
            self.board.close();
            return SessionActionDisposition::KeepNavigator;
        }
        if self.modal_owns_input() {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(SessionNotice::ModalOwnsInput);
            }
            return SessionActionDisposition::KeepNavigator;
        }
        self.board.open();
        self.enqueue_board_snapshot_refresh();
        SessionActionDisposition::DismissNavigator
    }

    /// Whether a modal surface or armed confirmation currently owns the
    /// input layer — the same surfaces the dispatcher's confirmation and
    /// draft capture arms serve before the board layer is ever reached.
    fn modal_owns_input(&mut self) -> bool {
        if self.rename_input_context().is_some()
            || self.setup_input_context().is_some()
            || self.panel_close_confirmation_armed()
        {
            return true;
        }
        self.state
            .as_ref()
            .is_some_and(|state| state.view().close_confirmation().is_some())
    }

    /// Input ownership: only an actually-presented board captures keys —
    /// the draw receipt's verdict, so an unpainted overlay never eats
    /// input for a surface the user cannot see.
    pub(in crate::terminal_session) fn agent_board_input(&self) -> bool {
        self.board.owns_input()
    }

    /// Close the lane's thread before the mailbox retires — the wake
    /// write interrupts any in-flight exchange so the join stays bounded.
    /// On non-Linux there is no lane; the call is a no-op.
    pub(in crate::terminal_session) fn release_agent_lane(&mut self) {
        self.board.release_lane();
    }

    /// One key under board capture. Consumed regardless — the capture
    /// layer guarantees nothing typed here ever reaches the pane.
    pub(in crate::terminal_session) fn capture_agent_board_key(
        &mut self,
        event: &KeyEvent,
    ) -> bool {
        if !self.agent_board_input() {
            return false;
        }
        if event.kind == crate::client_view::KeyKind::Release {
            return true;
        }
        match board_key_action(*event) {
            Some(AgentBoardKeyAction::SelectUp) => self.board.move_selection(-1),
            Some(AgentBoardKeyAction::SelectDown) => self.board.move_selection(1),
            Some(AgentBoardKeyAction::Refresh) => self.board.request_refresh(),
            Some(AgentBoardKeyAction::Jump) => self.board.request_jump(),
            Some(AgentBoardKeyAction::Close) => self.board.close(),
            None => {}
        }
        true
    }

    /// Pointer row select — resolves by stable session identity, so a
    /// stale click can only ever pick a row that still exists.
    pub(in crate::terminal_session) fn agent_board_select(
        &mut self,
        session: &AgentSessionId,
    ) -> bool {
        self.board.is_open() && self.board.select_session(session)
    }

    /// Wheel over the board — scrolls the row window, three rows a notch.
    pub(in crate::terminal_session) fn agent_board_scroll(&mut self, up: bool) -> bool {
        if !self.board.is_open() {
            return false;
        }
        self.board.scroll_by(if up { -3 } else { 3 });
        true
    }

    /// Backdrop dismissal: a press outside the painted board surface.
    pub(in crate::terminal_session) fn agent_board_dismiss(&mut self) {
        self.board.close();
    }

    /// The renderer's read handle, `Some` only while the board is open.
    pub(super) fn agent_board_view(&self) -> Option<NativeAgentBoardView<'_>> {
        self.board
            .is_open()
            .then(|| NativeAgentBoardView::new(&self.board, &self.agent_index))
    }

    /// The sidebar's summary of the last completed read — survives the
    /// board closing, absent entirely when no read ever completed.
    pub(super) fn agent_board_summary(&self) -> Option<AgentBoardSummary> {
        self.board.summary()
    }

    /// The draw receipt's verdict: only an actually-painted board may own
    /// input, and the painted viewport height bounds selection scrolling.
    pub(in crate::terminal_session) fn after_agent_board_draw(
        &mut self,
        draw: Option<AgentBoardDraw>,
    ) {
        self.board.settle_draw(draw);
    }

    /// A best-effort Workbench refresh at open time so the presentation
    /// index the jump resolves against is as fresh as the canonical
    /// snapshot machinery can make it. Silent finish — failures surface
    /// through the ordinary refresh-status path, never as board notices.
    fn enqueue_board_snapshot_refresh(&mut self) {
        let Some(deadline) = self
            .clock
            .now()
            .checked_add(crate::terminal_session::state::OPERATION_TOTAL)
        else {
            return;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return;
        };
        let result = mailbox.try_enqueue_refresh_for(
            state,
            deadline,
            |state, generation| {
                state
                    .start_pending_refresh(generation)
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
            |_state, _result| {},
        );
        if let Err(error) = result {
            self.retire_for_admission(&error);
        }
    }

    /// `select_tab` carrying the stable panel identity the jump wants
    /// focused after the switch lands — identical admission machinery,
    /// only the builder resolves `panel_id` instead of `panel(0)`.
    fn select_tab_for_panel(
        &mut self,
        position: u8,
        panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
    ) -> SessionActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_switch_for(
            state,
            |state, generation| {
                build_switch_for_panel(
                    state,
                    ClientAction::SelectTab(position),
                    &panel_id,
                    generation,
                    deadlines,
                )
            },
            |state, result, facts| {
                if result.is_ok() {
                    state.finish_switch_membership_admission(
                        facts.discarded(),
                        facts.inherited(),
                        facts.authorized_cleanup(),
                    );
                }
                finish_action(state, result);
            },
        );
        match result {
            Ok(_generation) => {
                self.pending_draws.clear();
                self.ordinary_poll_serial = None;
                SessionActionDisposition::DismissNavigator
            }
            Err(error) => {
                self.retire_for_admission(&error);
                SessionActionDisposition::KeepNavigator
            }
        }
    }

    /// The per-tick board drive: expire a budgeted jump, drain arrived
    /// lane outcomes, issue the next queued intent, and advance the
    /// in-flight jump's stage. Runs inside the ordinary scheduler block,
    /// so an armed membership-recovery attempt still owns the mailbox.
    pub(super) fn drive_agent_board(&mut self) -> bool {
        let now = self.clock.now();
        let mut changed = self.board.expire_jump(now);
        // The Get path's deterministic pick asks which attachments already
        // resolve into the caller's active context+tab — read once here,
        // live at the moment the lane outcome is applied.
        let (location, tab) = self.agent_anchor();
        changed |= self.board.drain_lane(now, |attachment| {
            self.agent_index
                .placement(attachment)
                .is_some_and(|placement| {
                    location.as_ref() == Some(&placement.location)
                        && tab.as_ref() == Some(&placement.tab_id)
                })
        });
        changed |= self.board.drive_intent(now);
        changed |= self.drive_agent_jump(now);
        changed
    }

    /// The jump's "current-tab usable" anchor: the active context's
    /// canonical location plus the active tab's stable identity.
    fn agent_anchor(&self) -> (Option<NativeContextLocation>, Option<WorkbenchTabId>) {
        let Some(state) = self.state.as_ref() else {
            return (None, None);
        };
        let location = Some(state.cache().location().clone());
        let tab = state
            .active()
            .and_then(|active| state.cache().tabs().get(active.index()))
            .map(|tab| tab.tab_id().clone());
        (location, tab)
    }

    /// One stage attempt of an accepted jump. The pure planner decides
    /// the next step from live facts; each arm reuses an existing
    /// admission (`select_context_at`, `select_tab`, the local
    /// panel-focus path) and the next tick re-plans — stages never cache
    /// positions and never block.
    fn drive_agent_jump(&mut self, _now: Instant) -> bool {
        let Some(stage) = self.board.jump().map(|jump| jump.stage.clone()) else {
            return false;
        };
        let JumpStage::Navigating { candidates, index } = stage else {
            return false;
        };
        let Some(attachment) = candidates.get(index) else {
            return self.board.advance_jump_candidate();
        };
        let Some(placement) = self.agent_index.placement(attachment).cloned() else {
            // This attachment's presentation identity no longer resolves
            // to any live panel — try the next candidate, never a guess.
            return self.board.advance_jump_candidate();
        };
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let view = state.view();
        let context_locations: Vec<NativeContextLocation> = (0..view.context_count())
            .filter_map(|index| view.context_at(index))
            .map(|context| context.location().clone())
            .collect();
        let active_location = view
            .active_context_position()
            .and_then(|position| position.checked_sub(1))
            .and_then(|index| view.context_at(index))
            .map(|context| context.location().clone());
        let target_tab_index = state.cache().tab_index(&placement.tab_id);
        let active_tab = state
            .active()
            .and_then(|active| state.cache().tabs().get(active.index()))
            .map(|tab| tab.tab_id().clone());
        let focus = state.focus_panel_id().cloned();
        let context_selecting = state.context_selecting();
        let switching = state.phase_kind() == SessionPhaseKind::Switching;
        match plan_jump_step(
            &placement,
            active_location.as_ref(),
            &context_locations,
            target_tab_index,
            active_tab.as_ref(),
            focus.as_ref(),
            context_selecting,
            switching,
        ) {
            JumpStep::Wait => false,
            JumpStep::Advance => self.board.advance_jump_candidate(),
            JumpStep::SelectContext(index) => {
                let _disposition = self.select_context_at(index);
                true
            }
            JumpStep::SelectTab(position) => {
                // The jump's switch carries the target panel's stable
                // identity — the landing pane re-resolves inside the
                // admission, never the `panel(0)` fallback.
                let _disposition = self.select_tab_for_panel(position, placement.panel_id.clone());
                true
            }
            JumpStep::FocusPanel(panel) => {
                let _disposition = self.move_panel_focus_to(&panel);
                true
            }
            JumpStep::Landed => {
                self.board.jump_landed();
                true
            }
        }
    }
}
