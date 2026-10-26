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

#[cfg(test)]
use super::super::agent_board::AgentBoardInputContext;
use super::super::agent_board::{
    AgentBoardControlTarget, AgentBoardDraw, AgentBoardEpisodeToken, AgentBoardKeyAction,
    AgentBoardRowTarget, AgentBoardToken, JumpStage, JumpStep, NativeAgentBoardView,
    board_key_action, plan_jump_step,
};
#[cfg(windows)]
use super::super::agent_board::{AgentStartKeyAction, agent_start_key_action};
use super::{
    ActionBuildError, Arc, CarrierFailure, ClientAction, ClientActionDisposition, ClientNotice,
    ClientPhaseKind, NativeUi, OperationDeadlines, WorkbenchTabId, build_switch_for_panel,
    finish_action,
};
use crate::client_view::KeyEvent;
use crate::tui_composition::{NativeCaptureOwner, project_native_capture};
use crate::workbench_client::workbench_projection::NativeContextLocation;
use std::time::Instant;

impl NativeUi {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(in crate::workbench_client) fn set_agent_board_origin(
        &mut self,
        origin: crate::local_client::LocalClientOrigin,
    ) {
        #[cfg(windows)]
        self.launch_profiles.set_origin(&origin);
        self.attention.set_origin(origin.clone());
        #[cfg(windows)]
        self.reports.set_origin(origin.clone());
        #[cfg(windows)]
        self.graphics.set_origin(origin.clone());
        #[cfg(windows)]
        self.overlays.set_origin(origin.clone());
        #[cfg(windows)]
        self.links.set_origin(origin.clone());
        #[cfg(windows)]
        self.copy_mode.set_origin(origin.clone());
        #[cfg(windows)]
        self.popup.set_origin(origin.clone());
        #[cfg(windows)]
        self.worktrees.set_origin(origin.clone());
        self.settings.set_origin(origin.clone());
        self.board.set_origin(origin);
    }

    /// `Ctrl+B g`. Opening queues the first read and refreshes the
    /// Workbench index best-effort; `DismissNavigator` reports the open
    /// so the client clears whatever overlay held the slot before it.
    /// A modal surface or armed confirmation owning input refuses the
    /// open — the apply-layer check backs the dispatcher's capture order.
    pub(super) fn toggle_agent_board(&mut self) -> ClientActionDisposition {
        if self.retirement_failure.is_some() {
            return ClientActionDisposition::KeepNavigator;
        }
        if self.board.is_open() {
            self.board.close();
            return ClientActionDisposition::KeepNavigator;
        }
        if self.modal_owns_input() {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::ModalOwnsInput);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        #[cfg(windows)]
        self.board.open_with_start_catalog(std::env::var_os("PATH"));
        #[cfg(not(windows))]
        self.board.open();
        self.enqueue_board_snapshot_refresh();
        ClientActionDisposition::DismissNavigator
    }

    /// Whether a modal surface or armed confirmation currently owns the
    /// input layer — the same surfaces the dispatcher's confirmation and
    /// draft capture arms serve before the board layer is ever reached.
    pub(super) fn modal_owns_input(&mut self) -> bool {
        if self.settings_active() {
            return true;
        }
        #[cfg(windows)]
        if self.worktree_input_context().is_some() {
            return true;
        }
        if self.rename_input_context().is_some()
            || self.setup_input_context().is_some()
            || self.panel_close_confirmation_armed()
            || self.view().workspace_close_confirmation().is_some()
        {
            return true;
        }
        self.state
            .as_ref()
            .is_some_and(|state| state.view().close_confirmation().is_some())
    }

    #[cfg(test)]
    pub(in crate::workbench_client) fn agent_board_input_context(
        &self,
    ) -> Option<AgentBoardInputContext> {
        self.board.input_context()
    }

    /// Close the lane's thread before the mailbox retires — the wake
    /// write interrupts any in-flight exchange so the join stays bounded.
    /// On non-Linux there is no lane; the call is a no-op.
    pub(in crate::workbench_client) fn release_agent_lane(&mut self) {
        self.board.release_lane();
        self.settings.close();
        #[cfg(windows)]
        if !self.worktrees.close() {
            self.worktrees
                .launcher
                .entry_notice(super::WorktreeNotice::Unknown);
        }
        self.attention.close();
        #[cfg(windows)]
        self.reports.close();
        #[cfg(windows)]
        self.graphics.close();
        #[cfg(windows)]
        self.copy_mode.close();
    }

    /// One key under board capture. Consumed regardless — the capture
    /// layer guarantees nothing typed here ever reaches the pane.
    pub(in crate::workbench_client) fn capture_agent_board_key(
        &mut self,
        expected: AgentBoardEpisodeToken,
        event: &KeyEvent,
    ) -> bool {
        let Some(context) = self.board.input_context() else {
            return false;
        };
        if context.episode() != expected {
            return false;
        }
        if event.kind == crate::client_view::KeyKind::Release {
            return true;
        }
        if board_key_action(*event) == Some(AgentBoardKeyAction::Close) {
            self.board.close();
            return true;
        }
        if !context.interactive() {
            return true;
        }
        #[cfg(windows)]
        if self.board.start_form_active() {
            match agent_start_key_action(*event, self.board.start_running()) {
                Some(AgentStartKeyAction::Start) => {
                    if let Some(panel) = self
                        .state
                        .as_ref()
                        .and_then(|state| state.focus_panel_id().cloned())
                        && self.board.start_agent(panel)
                    {
                        self.accepted_agent_start();
                    }
                }
                Some(AgentStartKeyAction::PreviousProvider) => {
                    self.board.move_start_provider(-1);
                }
                Some(AgentStartKeyAction::NextProvider) => {
                    self.board.move_start_provider(1);
                }
                Some(AgentStartKeyAction::Cancel) => {
                    self.board.cancel_start_drawn();
                }
                None => {
                    self.board.edit_start_draft(*event);
                }
            }
            return true;
        }
        match board_key_action(*event) {
            Some(AgentBoardKeyAction::SelectUp) => self.board.move_selection(-1),
            Some(AgentBoardKeyAction::SelectDown) => self.board.move_selection(1),
            Some(AgentBoardKeyAction::Refresh) => self.board.request_refresh(),
            Some(AgentBoardKeyAction::Jump) if self.board.selected_is_shown() => {
                self.board.request_jump();
            }
            Some(AgentBoardKeyAction::Jump | AgentBoardKeyAction::Close) => {}
            None => {}
        }
        true
    }

    /// Pointer row selection commits on press against the exact successful
    /// Board draw that published the target.
    pub(in crate::workbench_client) fn agent_board_select(
        &mut self,
        target: &AgentBoardRowTarget,
    ) -> bool {
        self.board.select_drawn_row(target)
    }

    pub(in crate::workbench_client) fn scroll_agent_board(
        &mut self,
        token: &AgentBoardToken,
        up: bool,
        lines: usize,
    ) -> Option<AgentBoardToken> {
        self.board.scroll_painted(token, up, lines)
    }

    pub(in crate::workbench_client) fn click_agent_board_control(
        &mut self,
        target: &AgentBoardControlTarget,
    ) -> bool {
        #[cfg(windows)]
        if target.control() == super::super::agent_board::AgentBoardControl::Start {
            let Some(panel) = self
                .state
                .as_ref()
                .and_then(|state| state.focus_panel_id().cloned())
            else {
                return false;
            };
            let accepted = self.board.start_agent_drawn(target, panel);
            if accepted {
                self.accepted_agent_start();
            }
            return accepted;
        }
        self.board.control_drawn(target)
    }

    #[cfg(windows)]
    fn accepted_agent_start(&mut self) {
        self.board.close();
        if let Some(phase) = self.board.start_phase()
            && let Some(state) = self.state.as_mut()
        {
            state.set_notice(ClientNotice::AgentStart(phase));
        }
    }

    /// Backdrop dismissal: a press outside the painted board surface.
    pub(in crate::workbench_client) fn agent_board_dismiss(&mut self) {
        self.board.close();
    }

    /// The renderer's read handle, `Some` only while the board is open.
    pub(super) fn agent_board_view(&self) -> Option<NativeAgentBoardView<'_>> {
        self.board.is_open().then(|| {
            let view = NativeAgentBoardView::new(&self.board, &self.agent_index);
            #[cfg(windows)]
            let view = view.with_attention(&self.attention);
            view
        })
    }

    /// Settles the renderer's exact episode verdict. A geometry-blocked first
    /// draw refuses the new Board; an admitted Board suspends and retains its
    /// state until a complete frame restores current draw proof.
    pub(in crate::workbench_client) fn after_agent_board_draw(
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
            .checked_add(crate::workbench_client::state::OPERATION_TOTAL)
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
    pub(super) fn select_tab_for_panel(
        &mut self,
        position: u8,
        panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
    ) -> ClientActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
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
                ClientActionDisposition::DismissNavigator
            }
            Err(error) => {
                self.retire_for_admission(&error);
                ClientActionDisposition::KeepNavigator
            }
        }
    }

    /// The per-tick board drive: expire a budgeted jump, drain arrived
    /// lane outcomes, issue the next queued intent, and advance the
    /// in-flight jump's stage. Runs inside the ordinary scheduler block,
    /// so an armed membership-recovery attempt still owns the mailbox.
    /// `allow_jump` is the total compositor's current top-owner verdict;
    /// covering Client surfaces block only focus-moving navigation while
    /// Board-owned domain work continues to settle.
    pub(super) fn drive_agent_board(&mut self, allow_jump: bool) -> bool {
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
        #[cfg(windows)]
        {
            let before = self.board.start_phase();
            changed |= self.board.drain_start();
            let after = self.board.start_phase();
            if before != after
                && let Some(phase) = after
                && let Some(state) = self.state.as_mut()
            {
                state.set_notice(ClientNotice::AgentStart(phase));
            }
        }
        if allow_jump {
            changed |= self.drive_agent_jump(now);
        }
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
        // The accepted lane may settle while a child covers the Board or
        // geometry suspends it. Keep that result in this episode, but do not
        // move context, tab, panel, or presentation unless the compositor
        // still names the Board as the top native owner and its successful
        // frame visibility lease remains live.
        if project_native_capture(self.capture_claims()) != Some(NativeCaptureOwner::AgentBoard)
            || self
                .board
                .input_context()
                .is_none_or(|context| !context.interactive())
        {
            return false;
        }
        let Some(stage) = self.board.jump().map(|jump| jump.stage.clone()) else {
            return false;
        };
        let JumpStage::Navigating { candidates, index } = stage else {
            return false;
        };
        if !self.agent_index.ready() {
            // No canonical snapshot has fed the index yet — resolving
            // anything now could only guess, and guessing reports `Gone`
            // for sessions that exist. The jump waits inside its own
            // deadline while the refresh machinery keeps trying; expiry
            // reports `Expired` honestly through `expire_jump`.
            self.enqueue_board_snapshot_refresh();
            return false;
        }
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
        let switching = state.phase_kind() == ClientPhaseKind::Switching;
        // `Landed` must name a live viewer: resolved placement plus an
        // attached, projected, non-departing record under the current
        // anchor — a dead viewer routes to the same-tab reattach instead.
        let target_live = state.panel_viewer_live(&placement.panel_id);
        match plan_jump_step(
            &placement,
            active_location.as_ref(),
            &context_locations,
            target_tab_index,
            active_tab.as_ref(),
            focus.as_ref(),
            target_live,
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
            JumpStep::Reattach { position, panel_id } => {
                // Same tab, dead viewer: `SelectTab` on the current
                // position is a NoOp to the ordinary planner, so the
                // builder's dedicated arm mints the fresh-generation
                // reattach only for a genuinely dead panel.
                let _disposition = self.select_tab_for_panel(position, panel_id);
                true
            }
            JumpStep::FocusPanel(panel) => {
                let _disposition = self.move_panel_focus_to(&panel);
                true
            }
            JumpStep::Landed => {
                let Some(index) = target_tab_index else {
                    return false;
                };
                let Some(panel) = state.cache().tabs()[index]
                    .panel_index(&placement.panel_id)
                    .and_then(|panel_index| state.cache().tabs()[index].panel(panel_index))
                else {
                    return false;
                };
                let Ok(presentation) = orchbee_workbench_protocol::TerminalPresentationId::parse(
                    attachment.presentation_id.as_str(),
                ) else {
                    return self.board.advance_jump_candidate();
                };
                if state.selected_presentation(panel).presentation_id() != &presentation {
                    return self.select_presentation(index, &placement.panel_id, &presentation);
                }
                self.board.jump_landed();
                true
            }
        }
    }
}
