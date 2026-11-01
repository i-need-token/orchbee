//! Port vocabulary for surfaces and components that own input. [`SurfaceInput`] is local input
//! that stays with its owner: keys and pastes for a capture owner, editor mutations, hover,
//! provisional selection, cursor placement, strip paging, and the program input of an independent
//! popup. [`SurfaceAction`] is a committed gesture result, grouped by the feature that names and
//! owns it. A value aimed at a painted or captured incarnation carries the witness it was resolved
//! against, and the owner refuses it once that incarnation is gone; the witness-free `Cancel`,
//! `Exit`, and `Dismiss` end whichever episode is current.

use crate::workbench_state::workspace_setup::{SetupEdit, SetupToken};
use crate::workbench_state::worktree_launch::{WorktreeEdit, WorktreeToken};

use super::{DrawnPresentationTarget, RenameDraftToken, RenameEdit};
use crate::action_menu::{DrawnTabCreate, MenuTarget, MenuToken};
use crate::client_view::{ClientKey, KeyEvent};
use orchbee_workbench_protocol::WorkbenchTabId;

/// One text entry for a capturing editor: a key, or a bracketed paste. `Paste(None)` reports a
/// paste the decoder refused, so the owner can end its paste state without inserting text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEntry<'a> {
    Key(KeyEvent),
    Paste(Option<&'a str>),
}

/// One local input for the current input owner. None of these commits a product result by
/// itself; a captured key may still lead the owner to its own committed transition, exactly as
/// the owner's keymap defines.
pub enum SurfaceInput<'a> {
    /// Settings text entry for the exact painted Settings input snapshot.
    Settings {
        expected: crate::native_settings::SettingsInputContext,
        entry: TextEntry<'a>,
    },
    /// A key for the exact retained action-menu input snapshot.
    ActionMenu {
        expected: crate::action_menu::MenuInputContext,
        key: KeyEvent,
    },
    /// Idle pointer motion over a painted menu row; the owner reports whether the highlight
    /// moved.
    ActionMenuHover {
        source: &'a MenuTarget,
        token: MenuToken,
        row: usize,
    },
    /// Copy-mode text entry for the active copy episode.
    CopyMode(TextEntry<'a>),
    /// Copy cursor placement on the exact painted content surface: `start` at the press, then
    /// each drag step.
    CopyModeCell {
        token: crate::terminal_copy_mode::CopySurfaceToken,
        cell: crate::terminal_copy_mode::CopyModeCursor,
        start: bool,
    },
    /// A key for the exact captured Tab close confirmation.
    TabClose {
        token: crate::workbench_state::CloseConfirmationToken,
        key: ClientKey,
    },
    /// A key for the exact captured Panel close confirmation.
    PanelClose {
        token: crate::workbench_state::PanelCloseConfirmationToken,
        key: ClientKey,
    },
    /// A key for the exact captured Workspace or Project close confirmation.
    WorkspaceClose {
        token: crate::workbench_state::workspace_close::WorkspaceCloseToken,
        key: ClientKey,
    },
    /// A key for the exact Agent board capture episode.
    AgentBoard {
        episode: crate::agent_board::AgentBoardEpisodeToken,
        key: KeyEvent,
    },
    /// A press on a painted board row: provisional selection by stable session identity, never
    /// by position.
    AgentBoardSelect(&'a crate::agent_board::AgentBoardRowTarget),
    /// Pages the painted Tab strip window; a stale source changes nothing.
    TabStripPage {
        source: &'a DrawnTabCreate,
        start: &'a crate::workbench_state::TabStripStart,
    },
    /// Keeps a paged strip window across a pointer activation of a Tab painted in it.
    TabStripFollow {
        source: &'a DrawnTabCreate,
        tab: &'a WorkbenchTabId,
    },
    /// Decoded input for the program of an interactive independent popup.
    Popup {
        id: &'a orchbee_workbench_protocol::WorkbenchRequestId,
        item: &'a crate::terminal_events::DecodedItem,
    },
    /// Pointer input inside the painted content of an independent popup.
    PopupPointer {
        id: &'a orchbee_workbench_protocol::WorkbenchRequestId,
        content: ratatui::layout::Rect,
        event: crate::terminal_events::MouseEvent,
    },
    /// One edit of the exact rename draft.
    Rename {
        token: RenameDraftToken,
        edit: RenameEdit<'a>,
    },
    /// One edit of the exact Workspace setup draft.
    Setup {
        token: SetupToken,
        edit: SetupEdit<'a>,
    },
    /// One edit of the exact worktree launch draft.
    Worktree {
        token: WorktreeToken,
        edit: WorktreeEdit<'a>,
    },
}

/// One committed gesture result, named by the feature that owns it. Composition executes it; the
/// owner revalidates the witness and settles the outcome, so a stale witness changes nothing.
pub enum SurfaceAction<'a> {
    Settings(SettingsAction),
    ActionMenu(MenuAction<'a>),
    CopyMode(CopyAction),
    AgentBoard(BoardAction<'a>),
    Close(CloseDecision),
    Rename(RenameAction),
    Setup(SetupAction),
    Worktree(WorktreeAction),
    Tabs(TabAction<'a>),
    Layout(LayoutAction<'a>),
    /// Activates one painted presentation target.
    Presentation(&'a DrawnPresentationTarget),
    /// Closes an independent popup through its painted close control.
    PopupClose(&'a crate::independent_popup::DrawnPopupClose),
}

/// Committed Settings gestures on the exact painted Settings surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsAction {
    Row {
        token: crate::native_settings::Token,
        row: crate::native_settings::Row,
    },
    Control {
        token: crate::native_settings::Token,
        control: crate::native_settings::Control,
    },
}

/// Committed action-menu gestures on the exact painted menu.
pub enum MenuAction<'a> {
    Row {
        source: &'a MenuTarget,
        token: MenuToken,
        row: usize,
    },
    Control {
        source: &'a MenuTarget,
        token: MenuToken,
        control: crate::action_menu::MenuControl,
    },
    /// Ends the open menu episode without a selection.
    Cancel,
}

/// Committed copy-mode gestures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyAction {
    Control {
        token: crate::terminal_copy_mode::CopyModeToken,
        control: crate::terminal_copy_mode::CopyControl,
    },
    /// Leaves copy mode; the owner reports whether an episode ended.
    Exit,
}

/// Committed Agent board gestures.
pub enum BoardAction<'a> {
    Control(&'a crate::agent_board::AgentBoardControlTarget),
    /// An outside press dismissed the board's capturing overlay.
    Dismiss,
}

/// A completed click on an exact, fully painted close decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseDecision {
    Tab {
        token: crate::workbench_state::CloseConfirmationToken,
        confirm: bool,
    },
    Panel {
        token: crate::workbench_state::PanelCloseConfirmationToken,
        confirm: bool,
    },
    Workspace {
        token: crate::workbench_state::workspace_close::WorkspaceCloseToken,
        confirm: bool,
    },
}

/// Committed results of the exact rename draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenameAction {
    Submit(RenameDraftToken),
    Cancel(RenameDraftToken),
}

/// Committed results of the exact Workspace setup draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupAction {
    Submit(SetupToken),
    /// `Esc` and the cancel button: a follow-up step returns to the step before it.
    Cancel(SetupToken),
    /// The frame's close control: ends the whole dialog.
    Close(SetupToken),
    /// Selects and submits one exact Project row authorized by the last draw.
    Choose {
        token: SetupToken,
        index: usize,
    },
}

/// Committed results of the exact worktree launch picker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeAction {
    Submit(WorktreeToken),
    Dismiss(WorktreeToken),
    /// Submits an exact checkout from the current successfully painted picker.
    Choose {
        token: WorktreeToken,
        index: usize,
    },
}

/// Committed gestures on the painted Tab strip.
pub enum TabAction<'a> {
    RequestClose {
        source: &'a DrawnTabCreate,
        tab: &'a WorkbenchTabId,
    },
    RequestRename {
        source: &'a DrawnTabCreate,
        tab: &'a WorkbenchTabId,
    },
    /// Commits a released Tab drag.
    Drag {
        source: &'a DrawnTabCreate,
        operation: orchbee_workbench_protocol::TabConfiguration,
    },
}

/// Committed layout gestures. Resize and divider releases hand off to the asynchronous local
/// owner, which reports its eventual result.
pub enum LayoutAction<'a> {
    ResizeControl {
        target: &'a crate::tui_layout::DrawnResize,
        direction: crate::client_view::PanelDirection,
        fine: bool,
    },
    Divider {
        target: &'a crate::tui_layout::DrawnDivider,
        ratio: orchbee_workbench_protocol::ClientSplitRatio,
    },
    /// Commits a released sidebar reorder.
    SidebarReorder {
        source: &'a super::SidebarReorderSource,
        operation: orchbee_workbench_protocol::NavigationConfiguration,
    },
}
