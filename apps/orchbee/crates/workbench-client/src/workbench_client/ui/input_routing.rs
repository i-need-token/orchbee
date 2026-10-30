//! Routes the input port's pane, list, and surface vocabulary to the feature composition that
//! owns each target. Each owner still revalidates its painted or captured witness; native
//! retirement settlement stays with `WorkbenchClient`.

use super::WorkbenchClientUi;
use crate::tui_pointer::hit_map::ListScrollTarget;
use crate::workbench_client::input::{PaneAdmission, PaneInput, SurfaceInput, TextEntry};

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
            | ListScrollTarget::MachinePicker(_) => None,
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
}
