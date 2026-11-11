//! The `WorkbenchClientUi` side of the Agent board: the toggle, input capture,
//! snapshot-index maintenance, the read lane's lifetime, the per-tick drive
//! (read settlement, intent issue, jump stages), and the staged cross-context
//! jump that reuses the existing context-selection, switch, and local
//! panel-focus admissions.
//!
//! Every stage re-resolves stable identities against live state: the
//! attachment's `presentation_id` against the freshest index, the owning
//! context against the navigation list, the tab against the projected
//! cache, and the panel against the live pane records. Nothing moves on
//! cached positions, and no stage ever mints a panel.

use super::{
    ActionBuildError, Arc, CarrierFailure, ClientAction, ClientActionDisposition, ClientNotice,
    ClientPhaseKind, OperationDeadlines, WorkbenchClientUi, WorkbenchTabId, build_switch_for_panel,
    finish_action,
};
#[cfg(test)]
use crate::agent_board::AgentBoardInputContext;
use crate::agent_board::picker::{PickedStart, PickerOutcome};
use crate::agent_board::start::StartPlace;
use crate::agent_board::start::jobs::Place;
use crate::agent_board::{
    AgentBoardControlTarget, AgentBoardDraw, AgentBoardEpisodeToken, AgentBoardKeyAction,
    AgentBoardRowTarget, AgentBoardToken, AgentBoardView, JumpStage, JumpStep, board_key_action,
    plan_jump_step,
};
use crate::client_view::{KeyCode, KeyEvent, Modifiers, PanelDirection};
use crate::tui_composition::{WorkbenchCaptureOwner, project_native_capture};
use crate::workbench_projection::ClientContextLocation;
use orchbee_agent_observation_protocol::AgentPresentationAttachment;
use std::time::Instant;

mod reads;
pub(super) use reads::BoardReads;

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn set_agent_board_origin(
        &mut self,
        origin: crate::local_client::LocalClientOrigin,
    ) {
        // Snapshot publications only: output-only commits must not keep a refresh in flight.
        self.publications = origin.watch_publications().ok();
        // A change made outside this Client shows at once, not when the shell's wait next ends.
        if let (Some(publications), Some(mailbox)) = (&self.publications, &self.mailbox) {
            let mailbox = Arc::downgrade(mailbox);
            publications.on_change(move || {
                if let Some(mailbox) = mailbox.upgrade() {
                    mailbox.wake_ui();
                }
            });
        }
        self.launch_profiles.set_origin(&origin);
        self.attention.set_origin(origin.clone());
        self.reports.set_origin(origin.clone());
        self.graphics.set_origin(origin.clone());
        self.overlays.set_origin(origin.clone());
        self.links.set_origin(origin.clone());
        self.copy_query.set_origin(origin.clone());
        self.popup.set_origin(origin.clone());
        self.worktrees.set_origin(origin.clone());
        self.settings.set_origin(origin.clone());
        self.duplication.set_origin(origin.clone());
        self.held_restore.set_origin(origin.clone());
        self.earlier_layouts.set_origin(origin.clone());
        self.start_jobs.set_origin(origin.clone());
        self.board.set_origin_locality(origin.is_local());
        self.board_reads.set_origin(origin);
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
            self.board_reads.close_board(&mut self.board);
            return ClientActionDisposition::KeepNavigator;
        }
        if self.modal_owns_input() {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::ModalOwnsInput);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        self.board.open();
        self.enqueue_board_snapshot_refresh();
        ClientActionDisposition::DismissNavigator
    }

    /// `Ctrl+B G`: the Start Agent picker, opened over whatever the Panel shows. From an open
    /// Board it replaces the Board's content; a second press closes it.
    pub(super) fn open_agent_picker(&mut self) -> ClientActionDisposition {
        if self.retirement_failure.is_some() {
            return ClientActionDisposition::KeepNavigator;
        }
        let focus = self.focused_panel();
        let worktree = self.worktree_place_available();
        if self.board.is_open() {
            if self.board.picker().is_some() {
                self.board_reads.close_board(&mut self.board);
            } else {
                self.board
                    .enter_picker(std::env::var_os("PATH"), focus.as_ref(), worktree);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        if self.modal_owns_input() {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::ModalOwnsInput);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        self.board
            .open_picker(std::env::var_os("PATH"), focus.as_ref(), worktree);
        ClientActionDisposition::DismissNavigator
    }

    /// The new-Tab menu's "More Agents…": the picker as `Ctrl+B G` opens it, with a new Tab as
    /// its place. The menu closed first, so nothing else owns input.
    pub(super) fn open_agent_picker_in_new_tab(&mut self) {
        if self.retirement_failure.is_some() || self.board.is_open() || self.modal_owns_input() {
            return;
        }
        let focus = self.focused_panel();
        let worktree = self.worktree_place_available();
        self.board
            .open_picker_in_new_tab(std::env::var_os("PATH"), focus.as_ref(), worktree);
    }

    /// The new-Tab menu's Agents, fixed when it opens, or `None` where the picker's new-Tab start
    /// would be hidden or refused, so the section is absent rather than disabled.
    pub(super) fn new_tab_menu_agents(&mut self) -> Option<Vec<&'static str>> {
        if self.retirement_failure.is_some()
            || !self
                .capabilities
                .supports(crate::tui_capabilities::Capability::AgentLaunch)
            || !self.board.can_start_agent()
            || self.view().no_current_workspace()
        {
            return None;
        }
        self.board.want_start_catalog(std::env::var_os("PATH"));
        Some(self.board.menu_agents())
    }

    /// The Session's Agents as the sidebar last observed them.
    pub(in crate::workbench_client) fn observe_session_agents(
        &mut self,
        agents: crate::agent_board::SessionAgents,
    ) {
        self.board.observe_session_agents(agents);
    }

    fn focused_panel(&self) -> Option<orchbee_workbench_protocol::WorkbenchPanelId> {
        self.state
            .as_ref()
            .and_then(|state| state.focus_panel_id().cloned())
    }

    /// Whether a modal surface or armed confirmation currently owns the
    /// input layer — the same surfaces the dispatcher's confirmation and
    /// draft capture arms serve before the board layer is ever reached.
    pub(super) fn modal_owns_input(&mut self) -> bool {
        if self.settings_active() {
            return true;
        }
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

    /// Close the lane threads before the mailbox retires — the wake
    /// write interrupts any in-flight exchange so each join stays bounded.
    pub(in crate::workbench_client) fn release_agent_lane(&mut self) {
        self.start_jobs.close();
        self.board.retire_start_catalog();
        self.board_reads.release();
        self.settings.close();
        if !self.worktrees.close() {
            self.worktrees
                .launcher
                .entry_notice(super::WorktreeNotice::Unknown);
        }
        self.attention.close();
        self.reports.close();
        self.graphics.close();
        self.copy_query.close();
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
        if self.board.picker().is_some() {
            // Esc unwinds one level at a time: the query, then the picker, then the owner.
            if event.code == KeyCode::Esc && event.mods == Modifiers::NONE {
                self.board.picker_escape();
                self.board_reads.follow(self.board.open_episode());
                return true;
            }
            if !context.interactive() {
                return true;
            }
            if let PickerOutcome::Start(picked) = self.board.picker_key(*event) {
                self.board_reads.follow(self.board.open_episode());
                self.run_picked_start(picked);
            }
            return true;
        }
        if board_key_action(*event) == Some(AgentBoardKeyAction::Close) {
            self.board_reads.close_board(&mut self.board);
            return true;
        }
        if !context.interactive() {
            return true;
        }
        match board_key_action(*event) {
            Some(AgentBoardKeyAction::SelectUp) => self.board.move_selection(-1),
            Some(AgentBoardKeyAction::SelectDown) => self.board.move_selection(1),
            Some(AgentBoardKeyAction::Refresh) => self.board.request_refresh(self.clock.now()),
            Some(AgentBoardKeyAction::StartAgent) => {
                self.enter_agent_picker();
            }
            Some(AgentBoardKeyAction::Jump) if self.board.start_row_selected() => {
                self.enter_agent_picker();
            }
            Some(AgentBoardKeyAction::Jump) if self.board.selected_is_shown() => {
                self.board.request_jump();
            }
            Some(AgentBoardKeyAction::Jump | AgentBoardKeyAction::Close) => {}
            None => {}
        }
        true
    }

    fn enter_agent_picker(&mut self) {
        let focus = self.focused_panel();
        let worktree = self.worktree_place_available();
        self.board
            .enter_picker(std::env::var_os("PATH"), focus.as_ref(), worktree);
    }

    /// Runs a start the picker accepted in the place it showed.
    fn run_picked_start(&mut self, picked: PickedStart) {
        match picked.place {
            StartPlace::ThisPanel(panel) => {
                let clock = self.clock.clone();
                if !self.start_jobs.start(
                    self.board.start_state(),
                    picked.provider,
                    Place::Panel(panel),
                    clock,
                ) {
                    self.start_in_new_tab(picked.provider);
                }
            }
            StartPlace::NewTab => self.start_in_new_tab(picked.provider),
            StartPlace::SplitRight => self.start_in_split(picked.provider, PanelDirection::Right),
            StartPlace::SplitDown => self.start_in_split(picked.provider, PanelDirection::Down),
            StartPlace::NewWorktree => {
                let _disposition = self.request_worktree_starting(
                    super::worktree_launch::WorktreeMode::Create,
                    Some(picked.provider),
                );
            }
        }
    }

    /// Asks the Server for a split of the focused Panel whose new shell runs the Agent as its
    /// first command, through the split keys' own action, so their refusals and notices apply.
    fn start_in_split(&mut self, provider: &'static str, direction: PanelDirection) {
        let _disposition =
            self.structure_action(super::StructureAction::SplitAgent(direction, provider));
    }

    /// Asks the Server for a new Tab in the current Workspace whose shell runs the Agent as its
    /// first command, through the ordinary Tab creation; nothing is typed.
    pub(super) fn start_in_new_tab(&mut self, provider: &'static str) {
        if self.view().no_current_workspace() {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::NoWorkspace);
            }
            return;
        }
        let _disposition = self.structure_action(super::StructureAction::CreateAgentTab(provider));
    }

    /// Moves refused starts to new Tabs, and counts each Agent a created Tab's or split's shell
    /// runs as its first command among this Client's starts. The Server watches that start and
    /// reports one that did not happen on the Presentation.
    fn drive_start_places(&mut self) -> bool {
        let mut changed = false;
        while let Some(provider) = self.board.start_state().take_fallback() {
            self.start_in_new_tab(provider);
            changed = true;
        }
        let started = self
            .state
            .as_mut()
            .map(crate::workbench_state::WorkbenchClientState::take_started_agents)
            .unwrap_or_default();
        for provider in started {
            self.board.start_state().note_started(provider);
            changed = true;
        }
        changed
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
        if self.board.picker().is_some() {
            if !self.board.drawn(target) {
                return false;
            }
            let outcome = self.board.picker_control(target);
            self.board_reads.follow(self.board.open_episode());
            if let PickerOutcome::Start(picked) = outcome {
                self.run_picked_start(picked);
            }
            return true;
        }
        if target.control() == crate::agent_board::AgentBoardControl::StartRow {
            if !self.board.drawn(target) {
                return false;
            }
            self.enter_agent_picker();
            return true;
        }
        let changed = self.board.control_drawn(target, self.clock.now());
        self.board_reads.follow(self.board.open_episode());
        changed
    }

    /// Backdrop dismissal: a press outside the painted board surface.
    pub(in crate::workbench_client) fn agent_board_dismiss(&mut self) {
        self.board_reads.close_board(&mut self.board);
    }

    /// The renderer's read handle, `Some` only while the board is open.
    pub(super) fn agent_board_view(&self) -> Option<AgentBoardView<'_>> {
        self.board.is_open().then(|| {
            let view = AgentBoardView::new(&self.board, &self.agent_index);
            view.with_attention(&self.attention)
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
        self.board_reads.follow(self.board.open_episode());
    }

    /// A best-effort Workbench refresh at open time so the presentation
    /// index the jump resolves against is as fresh as the canonical
    /// snapshot machinery can make it. Silent finish — failures surface
    /// through the ordinary refresh-status path, never as board notices.
    fn enqueue_board_snapshot_refresh(&mut self) {
        let Some(deadline) = self
            .clock
            .now()
            .checked_add(crate::workbench_state::OPERATION_TOTAL)
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

    /// The per-tick board drive: expire a budgeted jump, settle arrived
    /// read events, issue the next queued intent, and advance the
    /// in-flight jump's stage. Runs inside the ordinary scheduler block,
    /// so an armed membership-recovery attempt still owns the mailbox.
    /// `allow_jump` is the total compositor's current top-owner verdict;
    /// covering Client surfaces block only focus-moving navigation while
    /// Board-owned domain work continues to settle.
    pub(super) fn drive_agent_board(&mut self, allow_jump: bool) -> bool {
        let now = self.clock.now();
        self.board_reads.follow(self.board.open_episode());
        let mut changed = self.board.expire_jump(now);
        // The Get path's deterministic pick asks which attachments already
        // resolve into the caller's active context+tab — read once here,
        // live at the moment the read answer is applied.
        let (location, tab) = self.agent_anchor();
        let mut prefer = |attachment: &AgentPresentationAttachment| {
            self.agent_index
                .placement(attachment)
                .is_some_and(|placement| {
                    location.as_ref() == Some(&placement.location)
                        && tab.as_ref() == Some(&placement.tab_id)
                })
        };
        while let Some(event) = self.board_reads.next_event() {
            changed = true;
            self.board.settle_read(event, now, &mut prefer);
        }
        changed |= self
            .board
            .drive_intent(now, |request| self.board_reads.issue(request));
        changed |= self.board.drain_start(now, |start, admission| {
            self.start_jobs.drain(start, admission)
        });
        changed |= self.drive_start_places();
        if let Some(notice) = self.board.start_state().take_notice()
            && let Some(state) = self.state.as_mut()
        {
            state.set_notice(ClientNotice::AgentStart(notice));
        }
        if allow_jump {
            changed |= self.drive_agent_jump(now);
        }
        changed
    }

    /// The jump's "current-tab usable" anchor: the active context's
    /// canonical location plus the active tab's stable identity.
    fn agent_anchor(&self) -> (Option<ClientContextLocation>, Option<WorkbenchTabId>) {
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
        if project_native_capture(self.capture_claims()) != Some(WorkbenchCaptureOwner::AgentBoard)
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
        let context_locations: Vec<ClientContextLocation> = (0..view.context_count())
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
                self.board_reads.follow(self.board.open_episode());
                true
            }
        }
    }
}

impl crate::agent_board::LivePlacementNames for super::WorkbenchClientView<'_> {
    /// The strip's own names for a Tab of the current context: the Tab's display name and, for
    /// the Panel's selected Presentation, the same automatic name the strip derives from it.
    fn placement_names(
        &self,
        presentation_id: &str,
        placement: &crate::agent_board::PresentationPlacement,
        locale: crate::localization::Locale,
    ) -> Option<(String, String)> {
        if self.state.as_ref()?.location() != &placement.location {
            return None;
        }
        let tab = self.tab_by_id(&placement.tab_id)?;
        let panel = (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .find(|panel| panel.panel_id() == &placement.panel_id)?;
        let member = panel
            .presentations()
            .find(|member| member.presentation_id().as_str() == presentation_id)?;
        let tab_name = tab.display_label(locale).into_owned();
        let title = if std::ptr::eq(self.selected_presentation(panel), member) {
            self.automatic_name(panel)
                .map(|name| locale.chrome_name(name).into_owned())
        } else {
            let facts = crate::agent_board::NameFacts::of(member);
            facts
                .chrome(Some(self))
                .map(|name| locale.chrome_name(name).into_owned())
        };
        Some((title.unwrap_or_else(|| tab_name.clone()), tab_name))
    }

    fn terminal_title(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
        binding: Option<&crate::workbench_projection::ClientTerminalBinding>,
    ) -> Option<crate::terminal_title::ChromeName<'_>> {
        self.terminal_title_name(presentation, binding, None)
    }
}
