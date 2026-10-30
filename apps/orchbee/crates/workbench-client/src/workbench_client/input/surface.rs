//! Port vocabulary for the surface that currently holds input. Local input stays with the surface
//! owner: keys and pastes for a capture owner, editor mutations, hover, cursor placement, and the
//! program input of an independent popup. Every value names the painted or captured incarnation
//! it was resolved against, and the owner refuses it once that incarnation is gone.

use crate::workbench_client::state::workspace_setup::{SetupEdit, SetupToken};
use crate::workbench_client::state::worktree_launch::{WorktreeEdit, WorktreeToken};

use super::{RenameDraftToken, RenameEdit};
use crate::client_view::{ClientKey, KeyEvent};

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
        source: &'a crate::action_menu::MenuTarget,
        token: crate::action_menu::MenuToken,
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
        token: crate::workbench_client::CloseConfirmationToken,
        key: ClientKey,
    },
    /// A key for the exact captured Panel close confirmation.
    PanelClose {
        token: crate::workbench_client::PanelCloseConfirmationToken,
        key: ClientKey,
    },
    /// A key for the exact captured Workspace or Project close confirmation.
    WorkspaceClose {
        token: crate::workbench_client::WorkspaceCloseToken,
        key: ClientKey,
    },
    /// A key for the exact Agent board capture episode.
    AgentBoard {
        episode: crate::workbench_client::AgentBoardEpisodeToken,
        key: KeyEvent,
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
