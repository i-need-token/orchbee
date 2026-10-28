//! Read-only native input composition projected from the feature owners.

use super::{WorkbenchClientUi, WorkbenchClientView};
use crate::{
    tui_composition::{CaptureClaims, WorkbenchCaptureOwner},
    tui_presentation::PresentationPhase,
    workbench_client::{
        CloseConfirmationToken, PanelCloseConfirmationToken, RenameDraftToken, SetupStage,
        SetupToken, WorkspaceCloseToken,
    },
};

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
    pub(crate) token: super::WorktreeToken,
    pub(crate) readiness: CaptureReadiness,
    pub(crate) stage: super::WorktreeStage,
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
    Popup(super::super::independent_popup::PopupInputContext),
    Worktree(WorktreeCaptureContext),
    Setup(SetupCaptureContext),
    Rename(RenameCaptureContext),
    PanelClose(ConfirmationCaptureContext<PanelCloseConfirmationToken>),
    TabClose(ConfirmationCaptureContext<CloseConfirmationToken>),
    WorkspaceClose(ConfirmationCaptureContext<WorkspaceCloseToken>),
    AgentBoard(super::super::agent_board::AgentBoardInputContext),
}

impl WorkbenchClientView<'_> {
    /// All native capture intents, without assigning priority or inferring
    /// presentation readiness from render visibility. Copy mode is included
    /// as a lower owner beneath the exclusive native surfaces.
    pub fn capture_claims(&self) -> CaptureClaims {
        let claims = CaptureClaims::default()
            .with_settings(self.settings_input_context().is_some())
            .with_setup(self.setup_capture_claim())
            .with_rename(self.rename_draft().is_some())
            .with_panel_close(self.panel_close_confirmation().is_some())
            .with_tab_close(self.close_confirmation().is_some())
            .with_workspace_close(self.workspace_close_confirmation().is_some())
            .with_agent_board(self.agent_board().is_some());
        claims
            .with_action_menu(self.action_menu_input_context().is_some())
            .with_popup(self.popup_capture_claim())
            .with_worktree(self.worktree_capture_claim())
            .with_copy(self.copy_mode().is_some())
    }

    /// Exact current context for the already-selected top owner.
    ///
    /// `None` is a stale/incomplete owner snapshot and must be consumed by
    /// the caller. It never authorizes falling through to a lower surface.
    pub(crate) fn capture_context(&self, owner: WorkbenchCaptureOwner) -> Option<CaptureContext> {
        match owner {
            WorkbenchCaptureOwner::Settings => {
                self.settings_input_context().map(CaptureContext::Settings)
            }
            WorkbenchCaptureOwner::ActionMenu => self
                .action_menu_input_context()
                .map(CaptureContext::ActionMenu),
            WorkbenchCaptureOwner::Popup => self.popup_input_context().map(CaptureContext::Popup),
            WorkbenchCaptureOwner::Worktree => {
                let view = self.worktree()?;
                let token = view.token();
                let input = self
                    .worktree_input_context()
                    .filter(|input| input.token == token);
                Some(CaptureContext::Worktree(WorktreeCaptureContext {
                    token,
                    readiness: if input.is_some() {
                        CaptureReadiness::Interactive
                    } else {
                        CaptureReadiness::Recovery
                    },
                    stage: input.map_or_else(|| view.stage(), |input| input.stage),
                    remaining_bytes: input.map_or(0, |input| input.remaining_bytes),
                }))
            }
            WorkbenchCaptureOwner::Setup => {
                let view = self.setup_draft()?;
                let token = view.token();
                let input = self
                    .setup_input_context()
                    .filter(|input| input.token() == token);
                Some(CaptureContext::Setup(SetupCaptureContext {
                    token,
                    phase: view.presentation(),
                    readiness: if input.is_some() {
                        CaptureReadiness::Interactive
                    } else {
                        CaptureReadiness::Recovery
                    },
                    stage: input.map_or_else(|| view.stage(), |input| input.stage()),
                    remaining_scalars: input.map_or(0, |input| input.remaining_scalars()),
                    remaining_bytes: input.map_or(0, |input| input.remaining_bytes()),
                }))
            }
            WorkbenchCaptureOwner::Rename => {
                let view = self.rename_draft()?;
                Some(CaptureContext::Rename(RenameCaptureContext {
                    token: view.token(),
                    phase: view.presentation(),
                    readiness: if view.interactive() {
                        CaptureReadiness::Interactive
                    } else {
                        CaptureReadiness::Recovery
                    },
                    remaining_scalars: view.remaining_scalar_capacity(),
                }))
            }
            WorkbenchCaptureOwner::PanelClose => {
                let view = self.panel_close_confirmation()?;
                Some(CaptureContext::PanelClose(ConfirmationCaptureContext {
                    token: view.token(),
                    phase: view.presentation(),
                    readiness: if view.interactive() {
                        CaptureReadiness::Interactive
                    } else {
                        CaptureReadiness::Recovery
                    },
                }))
            }
            WorkbenchCaptureOwner::TabClose => {
                let view = self.close_confirmation()?;
                Some(CaptureContext::TabClose(ConfirmationCaptureContext {
                    token: view.token(),
                    phase: view.presentation(),
                    readiness: if view.interactive() {
                        CaptureReadiness::Interactive
                    } else {
                        CaptureReadiness::Recovery
                    },
                }))
            }
            WorkbenchCaptureOwner::WorkspaceClose => {
                let view = self.workspace_close_confirmation()?;
                Some(CaptureContext::WorkspaceClose(ConfirmationCaptureContext {
                    token: view.token(),
                    phase: view.presentation(),
                    readiness: if view.interactive() {
                        CaptureReadiness::Interactive
                    } else {
                        CaptureReadiness::Recovery
                    },
                }))
            }
            WorkbenchCaptureOwner::AgentBoard => Some(CaptureContext::AgentBoard(
                self.agent_board()?.input_context(),
            )),
        }
    }
}

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn capture_claims(&self) -> CaptureClaims {
        self.view().capture_claims()
    }

    pub(in crate::workbench_client) fn capture_context(
        &self,
        owner: WorkbenchCaptureOwner,
    ) -> Option<CaptureContext> {
        self.view().capture_context(owner)
    }
}
