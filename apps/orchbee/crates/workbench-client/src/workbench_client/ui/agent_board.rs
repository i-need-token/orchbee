//! The `WorkbenchClientUi` side of the Start Agent picker: opening and closing it, its input
//! capture, the Session's Agent catalog stream, the startup jobs it hands picked starts to, and
//! the places a picked start runs in.

use super::{
    Arc, CarrierFailure, ClientAction, ClientActionDisposition, ClientNotice, OperationDeadlines,
    WorkbenchClientUi, build_switch_for_panel, finish_action,
};
#[cfg(test)]
use crate::agent_board::AgentBoardInputContext;
use crate::agent_board::picker::{PickedStart, PickerOutcome};
use crate::agent_board::start::StartPlace;
use crate::agent_board::start::jobs::Place;
use crate::agent_board::{
    AgentBoardControlTarget, AgentBoardDraw, AgentBoardEpisodeToken, AgentBoardToken,
    AgentBoardView,
};
use crate::client_view::{KeyCode, KeyEvent, Modifiers, PanelDirection};

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
        self.start_jobs.set_origin(origin.clone());
        self.agent_catalog.set_origin(origin);
    }

    /// `Ctrl+B G`: the Start Agent picker, opened over whatever the Panel shows; a second press
    /// closes it.
    pub(super) fn open_agent_picker(&mut self) -> ClientActionDisposition {
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
        let focus = self.focused_panel();
        let worktree = self.worktree_place_available();
        self.board.open_picker(focus.as_ref(), worktree);
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
        self.board.open_picker_in_new_tab(focus.as_ref(), worktree);
    }

    /// The new-Tab menu's Agents from the catalog already held, fixed when it opens, or `None`
    /// where the picker's new-Tab start would be hidden or refused, so the section is absent
    /// rather than disabled. Opening the menu never asks for a new lookup.
    pub(super) fn new_tab_menu_agents(&self) -> Option<Vec<&'static str>> {
        if self.retirement_failure.is_some()
            || !self
                .capabilities
                .supports(crate::tui_capabilities::Capability::AgentLaunch)
            || !self.board.can_start_agent()
            || self.view().no_current_workspace()
        {
            return None;
        }
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
    /// draft capture arms serve before the picker layer is ever reached.
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
        self.agent_catalog.close();
        self.board.retire_start_catalog();
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

    /// One key under picker capture. Consumed regardless — the capture
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
        // Esc clears the query first, then closes the picker.
        if event.code == KeyCode::Esc && event.mods == Modifiers::NONE {
            self.board.picker_escape();
            return true;
        }
        if !context.interactive() {
            return true;
        }
        if let PickerOutcome::Start(picked) = self.board.picker_key(*event) {
            self.run_picked_start(picked);
        }
        true
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

    /// Moves refused starts to new Tabs. The Server delivers and watches each Agent a created
    /// Tab's or split's shell runs as its first command, reports one that did not happen on the
    /// Presentation, and lists it among the Session's recent providers.
    fn drive_start_places(&mut self) -> bool {
        let mut changed = false;
        while let Some(provider) = self.board.start_state().take_fallback() {
            self.start_in_new_tab(provider);
            changed = true;
        }
        changed
    }

    /// Settles the Session's catalog stream into the start state, then sends the new lookup an
    /// opened picker asked for. Nothing is polled: the stream pushes every change.
    fn drive_agent_catalog(&mut self) -> bool {
        let session = self
            .state
            .as_ref()
            .map(|state| state.cache().session_id().clone());
        let mut changed = false;
        for event in self.agent_catalog.poll(session.as_ref(), &self.clock) {
            changed |= self.board.receive_agent_catalog(event);
        }
        if self.board.take_catalog_refresh() {
            self.agent_catalog.refresh(&self.clock);
        }
        changed
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
        if !self.board.drawn(target) {
            return false;
        }
        if let PickerOutcome::Start(picked) = self.board.picker_control(target) {
            self.run_picked_start(picked);
        }
        true
    }

    /// Backdrop dismissal: a press outside the painted picker surface.
    pub(in crate::workbench_client) fn agent_board_dismiss(&mut self) {
        self.board.close();
    }

    /// The renderer's read handle, `Some` only while the picker is open.
    pub(super) fn agent_board_view(&self) -> Option<AgentBoardView<'_>> {
        self.board
            .is_open()
            .then(|| AgentBoardView::new(&self.board))
    }

    /// Settles the renderer's exact episode verdict. A geometry-blocked first
    /// draw refuses the new picker; an admitted picker suspends and retains its
    /// state until a complete frame restores current draw proof.
    pub(in crate::workbench_client) fn after_agent_board_draw(
        &mut self,
        draw: Option<AgentBoardDraw>,
    ) {
        self.board.settle_draw(draw);
    }

    /// `select_tab` carrying the stable panel identity to focus after the
    /// switch lands — identical admission machinery, only the builder
    /// resolves `panel_id` instead of `panel(0)`.
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

    /// The per-tick picker drive: settle the catalog stream and the startup jobs, move refused
    /// starts to new Tabs and show a start notice. Runs inside the ordinary scheduler block.
    pub(super) fn drive_agent_board(&mut self) -> bool {
        let now = self.clock.now();
        let mut changed = self.drive_agent_catalog();
        changed |= self
            .board
            .drain_start(now, |start| self.start_jobs.drain(start));
        changed |= self.drive_start_places();
        if let Some(notice) = self.board.start_state().take_notice()
            && let Some(state) = self.state.as_mut()
        {
            state.set_notice(ClientNotice::AgentStart(notice));
        }
        changed
    }
}

impl crate::agent_board::LivePlacementNames for super::WorkbenchClientView<'_> {
    fn terminal_title(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
        binding: Option<&crate::workbench_projection::ClientTerminalBinding>,
    ) -> Option<crate::terminal_title::ChromeName<'_>> {
        self.terminal_title_name(presentation, binding, None)
    }
}
