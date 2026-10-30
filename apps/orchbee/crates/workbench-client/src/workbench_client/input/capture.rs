//! Capture contexts and typed edit vocabulary that the native owners project for input dispatch.

use crate::localization::SetupStage;
use crate::tui_presentation::PresentationPhase;
use crate::workbench_client::state::rename_draft;
use crate::workbench_client::state::workspace_close::WorkspaceCloseToken;
use crate::workbench_client::state::workspace_setup::SetupToken;
use crate::workbench_client::state::{CloseConfirmationToken, PanelCloseConfirmationToken};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenameDraftToken(pub(in crate::workbench_client) rename_draft::RenameDraftToken);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptureReadiness {
    Recovery,
    Interactive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenameCaptureContext {
    pub(crate) token: RenameDraftToken,
    pub(crate) phase: PresentationPhase,
    pub(crate) readiness: CaptureReadiness,
    pub(crate) remaining_scalars: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetupCaptureContext {
    pub(crate) token: SetupToken,
    pub(crate) phase: PresentationPhase,
    pub(crate) readiness: CaptureReadiness,
    pub(crate) stage: SetupStage,
    pub(crate) remaining_scalars: usize,
    pub(crate) remaining_bytes: usize,
}

#[derive(Clone, Copy)]
pub struct WorktreeCaptureContext {
    pub(crate) token: crate::workbench_client::state::worktree_launch::WorktreeToken,
    pub(crate) readiness: CaptureReadiness,
    pub(crate) stage: crate::workbench_client::state::worktree_launch::WorktreeStage,
    pub(crate) remaining_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfirmationCaptureContext<Token> {
    pub(crate) token: Token,
    pub(crate) phase: PresentationPhase,
    pub(crate) readiness: CaptureReadiness,
}

#[derive(Clone)]
pub enum CaptureContext {
    Settings(crate::native_settings::SettingsInputContext),
    ActionMenu(crate::action_menu::MenuInputContext),
    Popup(crate::workbench_client::independent_popup::PopupInputContext),
    Worktree(WorktreeCaptureContext),
    Setup(SetupCaptureContext),
    Rename(RenameCaptureContext),
    PanelClose(ConfirmationCaptureContext<PanelCloseConfirmationToken>),
    TabClose(ConfirmationCaptureContext<CloseConfirmationToken>),
    WorkspaceClose(ConfirmationCaptureContext<WorkspaceCloseToken>),
    AgentBoard(crate::workbench_client::agent_board::AgentBoardInputContext),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenameInputContext {
    pub(in crate::workbench_client) token: RenameDraftToken,
}
impl RenameInputContext {
    #[cfg(test)]
    pub(crate) const fn token(self) -> RenameDraftToken {
        self.token
    }
}

/// The typed edit vocabulary forwarded to the rename draft — the same
/// contract U5's typed-input bridge emits for text fields.
pub enum RenameEdit<'a> {
    Append(&'a str),
    Paste(&'a str),
    Backspace,
    Clear,
    Left,
    Right,
    Home,
    End,
    Delete,
    WordBackspace,
    Fault,
    Completeness(bool),
}
