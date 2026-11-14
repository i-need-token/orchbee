//! Routes the input port's pane, list, and surface vocabulary to the feature composition that
//! owns each target. Each owner still revalidates its painted or captured witness; native
//! retirement settlement stays with `WorkbenchClient`.

use super::WorkbenchClientUi;
use crate::client_view::ClientActionDisposition;
use crate::tui_hit_map::ListScrollTarget;
use crate::workbench_client::input::{
    BoardAction, CloseDecision, CopyAction, FindAction, LayoutAction, MenuAction, PaneAdmission,
    PaneInput, RenameAction, SettingsAction, SetupAction, SurfaceAction, SurfaceInput, TabAction,
    TextEntry, WorktreeAction,
};

impl WorkbenchClientUi {
    /// Selection and link steps report their own admission; history and scrollbar steps report
    /// whether the focused viewport consumed them.
    pub(in crate::workbench_client) fn pane_input(
        &mut self,
        input: PaneInput<'_>,
    ) -> PaneAdmission {
        let consumed = match input {
            PaneInput::Selection { pane, intent } => return self.pointer_selection(pane, intent),
            PaneInput::Link { source, cell } => return self.pointer_link(*source, cell),
            PaneInput::HistoryKey(event) => self.history_key(event),
            PaneInput::HistoryWheel { pane, up } => self.history_wheel(pane, up),
            PaneInput::Scrollbar {
                pane,
                source,
                offset,
            } => self.scrollbar_scroll(pane, source, offset),
        };
        if consumed {
            PaneAdmission::Admitted
        } else {
            PaneAdmission::Rejected
        }
    }

    /// Moves one painted list and returns its next painted target. Help, the navigator, and the
    /// machine picker are view-owned lists that the pointer layer scrolls without the port.
    pub(in crate::workbench_client) fn scroll_list(
        &mut self,
        target: &ListScrollTarget,
        up: bool,
        lines: usize,
    ) -> Option<ListScrollTarget> {
        match target {
            ListScrollTarget::Setup(token) => self
                .scroll_setup_projects(*token, up, lines)
                .map(ListScrollTarget::Setup),
            ListScrollTarget::Worktree(token) => self
                .scroll_worktree(*token, up, lines)
                .map(ListScrollTarget::Worktree),
            ListScrollTarget::Copy(token) => self
                .scroll_copy_mode(*token, up, lines)
                .map(ListScrollTarget::Copy),
            ListScrollTarget::AgentBoard(token) => self
                .scroll_agent_board(token, up, lines)
                .map(ListScrollTarget::AgentBoard),
            ListScrollTarget::Settings(token) => self
                .scroll_settings(*token, up, lines)
                .map(ListScrollTarget::Settings),
            ListScrollTarget::Menu(token) => self
                .scroll_action_menu(*token, up, lines)
                .map(ListScrollTarget::Menu),
            ListScrollTarget::Help(_)
            | ListScrollTarget::Navigator(_)
            | ListScrollTarget::MachinePicker(_)
            | ListScrollTarget::TabList { .. } => None,
        }
    }

    /// Delivers one local input to the surface that holds input. Popup program input always
    /// reports consumed because the popup lane owns its outcome.
    pub(in crate::workbench_client) fn surface_input(&mut self, input: SurfaceInput<'_>) -> bool {
        match input {
            SurfaceInput::Settings {
                expected,
                entry: TextEntry::Key(event),
            } => self.capture_settings_key_for(expected, event),
            SurfaceInput::Settings {
                expected,
                entry: TextEntry::Paste(text),
            } => self.capture_settings_paste_for(expected, text),
            SurfaceInput::ActionMenu { expected, key } => {
                self.capture_action_menu_key_for(expected, key)
            }
            SurfaceInput::ActionMenuHover { source, token, row } => {
                self.hover_action_menu_row(source, token, row)
            }
            SurfaceInput::CopyMode(TextEntry::Key(event)) => self.capture_copy_mode_key(&event),
            SurfaceInput::CopyMode(TextEntry::Paste(text)) => self.capture_copy_mode_paste(text),
            SurfaceInput::CopyModeCell { token, cell, start } => {
                self.copy_pointer_cell(token, cell, start)
            }
            SurfaceInput::TabClose { token, key } => self.capture_tab_confirmation_key(token, key),
            SurfaceInput::PanelClose { token, key } => {
                self.capture_panel_confirmation_key(token, key)
            }
            SurfaceInput::WorkspaceClose { token, key } => {
                self.capture_workspace_close_key(token, key)
            }
            SurfaceInput::AgentBoard { episode, key } => {
                self.capture_agent_board_key(episode, &key)
            }
            SurfaceInput::TabStripPage { source, start } => {
                self.page_drawn_tab_strip(source, start);
                true
            }
            SurfaceInput::TabStripFollow { source, tab } => {
                self.follow_drawn_tab_strip(source, tab);
                true
            }
            SurfaceInput::Popup { id, item } => {
                self.popup_input(id, item);
                true
            }
            SurfaceInput::PopupPointer { id, content, event } => {
                self.popup_mouse(id, content, event);
                true
            }
            SurfaceInput::Rename { token, edit } => self.edit_rename_draft(token, edit),
            SurfaceInput::Setup { token, edit } => self.edit_setup_draft(token, edit),
            SurfaceInput::Worktree { token, edit } => self.edit_worktree(token, edit),
        }
    }

    /// Executes one committed gesture result through the feature composition that owns it.
    /// Results whose owner reports only acceptance answer `KeepNavigator` when accepted. Layout
    /// resizing needs the client-control lane, so `WorkbenchClient` executes it instead.
    pub(in crate::workbench_client) fn surface_action(
        &mut self,
        action: SurfaceAction<'_>,
    ) -> Option<ClientActionDisposition> {
        let accepted = |accepted: bool| accepted.then_some(ClientActionDisposition::KeepNavigator);
        match action {
            SurfaceAction::Settings(SettingsAction::Row { token, row }) => {
                self.click_settings(token, row);
                None
            }
            SurfaceAction::Settings(SettingsAction::Control { token, control }) => {
                self.click_settings_control(token, control);
                None
            }
            SurfaceAction::ActionMenu(MenuAction::Row { source, token, row }) => {
                self.activate_action_menu_row(source, token, row);
                None
            }
            SurfaceAction::ActionMenu(MenuAction::Control {
                source,
                token,
                control,
            }) => {
                self.activate_action_menu_control(source, token, control);
                None
            }
            SurfaceAction::ActionMenu(MenuAction::Rest { source, token, row }) => {
                accepted(self.rest_action_menu_row(source, token, row))
            }
            SurfaceAction::ActionMenu(MenuAction::Cancel) => {
                self.cancel_action_menu();
                None
            }
            SurfaceAction::CopyMode(CopyAction::Control { token, control }) => {
                self.click_copy_control(token, control);
                None
            }
            SurfaceAction::CopyMode(CopyAction::Exit) => accepted(self.cancel_copy_mode()),
            SurfaceAction::Find(FindAction::Control { token, control }) => {
                self.click_find_control(token, control);
                None
            }
            SurfaceAction::Find(FindAction::Scroll { token, up }) => {
                accepted(self.scroll_find(token, up))
            }
            SurfaceAction::Find(FindAction::Close(token)) => accepted(self.close_find_at(token)),
            SurfaceAction::AgentBoard(BoardAction::Control(target)) => {
                accepted(self.click_agent_board_control(target))
            }
            SurfaceAction::AgentBoard(BoardAction::Dismiss) => {
                self.agent_board_dismiss();
                None
            }
            SurfaceAction::Close(CloseDecision::Tab { token, confirm }) => {
                self.click_tab_close(token, confirm);
                None
            }
            SurfaceAction::Close(CloseDecision::Panel { token, confirm }) => {
                self.click_panel_close(token, confirm);
                None
            }
            SurfaceAction::Close(CloseDecision::Workspace { token, confirm }) => {
                self.click_workspace_close(token, confirm);
                None
            }
            SurfaceAction::Rename(RenameAction::Submit(token)) => {
                Some(self.submit_rename_draft(token))
            }
            SurfaceAction::Rename(RenameAction::Cancel(token)) => {
                self.cancel_rename_draft(token);
                None
            }
            SurfaceAction::Setup(SetupAction::Submit(token)) => {
                Some(self.submit_setup_draft(token))
            }
            SurfaceAction::Setup(SetupAction::Cancel(token)) => {
                self.cancel_setup_draft(token);
                None
            }
            SurfaceAction::Setup(SetupAction::Close(token)) => {
                self.close_setup_draft(token);
                None
            }
            SurfaceAction::Setup(SetupAction::Choose { token, index }) => {
                self.choose_setup_project(token, index);
                None
            }
            SurfaceAction::Worktree(WorktreeAction::Submit(token)) => {
                self.submit_worktree(token);
                None
            }
            SurfaceAction::Worktree(WorktreeAction::Dismiss(token)) => {
                self.dismiss_worktree(token);
                None
            }
            SurfaceAction::Worktree(WorktreeAction::Choose { token, index }) => {
                self.choose_worktree(token, index);
                None
            }
            SurfaceAction::Tabs(TabAction::RequestClose { source, tab }) => {
                self.request_drawn_tab_close(source, tab);
                None
            }
            SurfaceAction::Tabs(TabAction::RequestRename { source, tab }) => {
                self.request_drawn_tab_rename(source, tab)
            }
            SurfaceAction::Tabs(TabAction::Drag { source, operation }) => {
                self.commit_tab_drag(source, operation);
                None
            }
            SurfaceAction::Layout(LayoutAction::SidebarReorder { source, operation }) => {
                self.commit_sidebar_reorder(source, operation);
                None
            }
            SurfaceAction::Layout(LayoutAction::Divider { .. }) => None,
            SurfaceAction::Presentation(target) => Some(self.activate_drawn_presentation(target)),
            SurfaceAction::HeldRestore(target) => Some(self.activate_held_restore(target)),
            SurfaceAction::PopupClose(target) => accepted(self.activate_drawn_popup_close(target)),
        }
    }
}
