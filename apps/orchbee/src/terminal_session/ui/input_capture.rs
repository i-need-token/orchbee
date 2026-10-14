//! Read-only native input composition projected from the feature owners.

use super::{NativeSessionView, NativeUi};
use crate::{
    terminal_session::{
        NativeCloseConfirmationToken, NativePanelCloseConfirmationToken, NativeRenameDraftToken,
        NativeSetupStage, NativeSetupToken, WorkspaceCloseToken,
    },
    tui_composition::{NativeCaptureClaims, NativeCaptureOwner},
    tui_presentation::PresentationPhase,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptureReadiness {
    Recovery,
    Interactive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RenameCaptureContext {
    pub(crate) token: NativeRenameDraftToken,
    pub(crate) phase: PresentationPhase,
    pub(crate) readiness: CaptureReadiness,
    pub(crate) remaining_scalars: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SetupCaptureContext {
    pub(crate) token: NativeSetupToken,
    pub(crate) phase: PresentationPhase,
    pub(crate) readiness: CaptureReadiness,
    pub(crate) stage: NativeSetupStage,
    pub(crate) remaining_scalars: usize,
    pub(crate) remaining_bytes: usize,
}

#[cfg(windows)]
#[derive(Clone, Copy)]
pub(crate) struct WorktreeCaptureContext {
    pub(crate) token: super::WorktreeToken,
    pub(crate) readiness: CaptureReadiness,
    pub(crate) stage: super::WorktreeStage,
    pub(crate) remaining_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConfirmationCaptureContext<Token> {
    pub(crate) token: Token,
    pub(crate) phase: PresentationPhase,
    pub(crate) readiness: CaptureReadiness,
}

#[derive(Clone)]
pub(crate) enum NativeCaptureContext {
    Settings(crate::native_settings::SettingsInputContext),
    #[cfg(windows)]
    ActionMenu(crate::action_menu::MenuInputContext),
    #[cfg(windows)]
    Popup(super::super::independent_popup::PopupInputContext),
    #[cfg(windows)]
    Worktree(WorktreeCaptureContext),
    Setup(SetupCaptureContext),
    Rename(RenameCaptureContext),
    PanelClose(ConfirmationCaptureContext<NativePanelCloseConfirmationToken>),
    TabClose(ConfirmationCaptureContext<NativeCloseConfirmationToken>),
    WorkspaceClose(ConfirmationCaptureContext<WorkspaceCloseToken>),
    AgentBoard(super::super::agent_board::AgentBoardInputContext),
}

impl NativeSessionView<'_> {
    /// All native capture intents, without assigning priority or inferring
    /// presentation readiness from render visibility. Copy mode is included
    /// as a lower owner beneath the exclusive native surfaces.
    pub(crate) fn capture_claims(&self) -> NativeCaptureClaims {
        let claims = NativeCaptureClaims::default()
            .with_settings(self.settings_input_context().is_some())
            .with_setup(self.setup_capture_claim())
            .with_rename(self.rename_draft().is_some())
            .with_panel_close(self.panel_close_confirmation().is_some())
            .with_tab_close(self.close_confirmation().is_some())
            .with_workspace_close(self.workspace_close_confirmation().is_some())
            .with_agent_board(self.agent_board().is_some());
        #[cfg(windows)]
        let claims = claims
            .with_action_menu(self.action_menu_input_context().is_some())
            .with_popup(self.popup_capture_claim())
            .with_worktree(self.worktree_capture_claim())
            .with_copy(self.copy_mode().is_some());
        claims
    }

    /// Exact current context for the already-selected top owner.
    ///
    /// `None` is a stale/incomplete owner snapshot and must be consumed by
    /// the caller. It never authorizes falling through to a lower surface.
    pub(crate) fn capture_context(
        &self,
        owner: NativeCaptureOwner,
    ) -> Option<NativeCaptureContext> {
        match owner {
            NativeCaptureOwner::Settings => self
                .settings_input_context()
                .map(NativeCaptureContext::Settings),
            #[cfg(windows)]
            NativeCaptureOwner::ActionMenu => self
                .action_menu_input_context()
                .map(NativeCaptureContext::ActionMenu),
            #[cfg(windows)]
            NativeCaptureOwner::Popup => {
                self.popup_input_context().map(NativeCaptureContext::Popup)
            }
            #[cfg(windows)]
            NativeCaptureOwner::Worktree => {
                let view = self.worktree()?;
                let token = view.token();
                let input = self
                    .worktree_input_context()
                    .filter(|input| input.token == token);
                Some(NativeCaptureContext::Worktree(WorktreeCaptureContext {
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
            NativeCaptureOwner::Setup => {
                let view = self.setup_draft()?;
                let token = view.token();
                let input = self
                    .setup_input_context()
                    .filter(|input| input.token() == token);
                Some(NativeCaptureContext::Setup(SetupCaptureContext {
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
            NativeCaptureOwner::Rename => {
                let view = self.rename_draft()?;
                Some(NativeCaptureContext::Rename(RenameCaptureContext {
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
            NativeCaptureOwner::PanelClose => {
                let view = self.panel_close_confirmation()?;
                Some(NativeCaptureContext::PanelClose(
                    ConfirmationCaptureContext {
                        token: view.token(),
                        phase: view.presentation(),
                        readiness: if view.interactive() {
                            CaptureReadiness::Interactive
                        } else {
                            CaptureReadiness::Recovery
                        },
                    },
                ))
            }
            NativeCaptureOwner::TabClose => {
                let view = self.close_confirmation()?;
                Some(NativeCaptureContext::TabClose(ConfirmationCaptureContext {
                    token: view.token(),
                    phase: view.presentation(),
                    readiness: if view.interactive() {
                        CaptureReadiness::Interactive
                    } else {
                        CaptureReadiness::Recovery
                    },
                }))
            }
            NativeCaptureOwner::WorkspaceClose => {
                let view = self.workspace_close_confirmation()?;
                Some(NativeCaptureContext::WorkspaceClose(
                    ConfirmationCaptureContext {
                        token: view.token(),
                        phase: view.presentation(),
                        readiness: if view.interactive() {
                            CaptureReadiness::Interactive
                        } else {
                            CaptureReadiness::Recovery
                        },
                    },
                ))
            }
            NativeCaptureOwner::AgentBoard => Some(NativeCaptureContext::AgentBoard(
                self.agent_board()?.input_context(),
            )),
        }
    }
}

impl NativeUi {
    pub(in crate::terminal_session) fn capture_claims(&self) -> NativeCaptureClaims {
        self.view().capture_claims()
    }

    pub(in crate::terminal_session) fn capture_context(
        &self,
        owner: NativeCaptureOwner,
    ) -> Option<NativeCaptureContext> {
        self.view().capture_context(owner)
    }
}
