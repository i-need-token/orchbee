//! Protocol-owned reply admission for the two opaque layout/terminal creation operations.
use super::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome as Outcome,
    WorkbenchCommandUnavailableReason as Unavailable, WorkbenchPanelTerminalTarget,
    WorkbenchRejectionReason as Rejection, WorkbenchTerminalOutcome,
};

impl WorkbenchCommand {
    /// Admits only layout/terminal creation receipts, including revision and target echoes.
    #[must_use]
    pub fn admits_creation_outcome(&self, outcome: &Outcome) -> bool {
        let structure = match &self.action {
            WorkbenchAction::TabCreateLayout { .. } => true,
            WorkbenchAction::PanelCreateTerminal { .. } => false,
            WorkbenchAction::ProjectCreate { .. }
            | WorkbenchAction::ProjectRename { .. }
            | WorkbenchAction::ProjectDissolve { .. }
            | WorkbenchAction::ProjectDissolveIfIdle { .. }
            | WorkbenchAction::WorkspaceCreate { .. }
            | WorkbenchAction::WorkspaceLaunch { .. }
            | WorkbenchAction::WorkspaceRename { .. }
            | WorkbenchAction::WorkspaceSetProject { .. }
            | WorkbenchAction::WorkspaceCreateProject { .. }
            | WorkbenchAction::WorkspaceMove { .. }
            | WorkbenchAction::WorkspaceMoveBlock { .. }
            | WorkbenchAction::WorkspaceClose { .. }
            | WorkbenchAction::WorkspaceCloseIfIdle { .. }
            | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
            | WorkbenchAction::TabRename { .. }
            | WorkbenchAction::NavigationConfigure { .. }
            | WorkbenchAction::TabsConfigure { .. }
            | WorkbenchAction::PanelRename { .. }
            | WorkbenchAction::PanelInputSet { .. }
            | WorkbenchAction::SplitRatioSet { .. }
            | WorkbenchAction::TabMove { .. }
            | WorkbenchAction::TabMoveIndex { .. }
            | WorkbenchAction::TabClose { .. }
            | WorkbenchAction::TabCreate { .. }
            | WorkbenchAction::PanelSplit { .. }
            | WorkbenchAction::PanelClose { .. }
            | WorkbenchAction::PanelMove { .. }
            | WorkbenchAction::PanelSwap { .. }
            | WorkbenchAction::PanelRelocate { .. }
            | WorkbenchAction::LaunchProfileCreate { .. }
            | WorkbenchAction::LaunchProfileDelete { .. } => return false,
        };
        match outcome {
            Outcome::Denied {} => return true,
            Outcome::LaunchProfileUnavailable {} | Outcome::LaunchProfileAmbiguous { .. } => {
                return matches!(
                    self.action,
                    WorkbenchAction::PanelCreateTerminal {
                        profile: Some(_),
                        ..
                    }
                );
            }
            Outcome::Rejected { reason, .. } => {
                return match reason {
                    Rejection::SessionMismatch
                    | Rejection::RevisionMismatch
                    | Rejection::TerminalContextUnavailable
                    | Rejection::ResourceLimit
                    | Rejection::RevisionExhausted
                    | Rejection::RequestIdentityConflict => true,
                    Rejection::IdentityExhausted => {
                        structure
                            || matches!(
                                self.action,
                                WorkbenchAction::PanelCreateTerminal {
                                    target: WorkbenchPanelTerminalTarget::Child { .. },
                                    ..
                                }
                            )
                    }
                    Rejection::TabUnavailable
                    | Rejection::PanelUnavailable
                    | Rejection::WorkspaceDirectoryUnavailable => !structure,
                    Rejection::ProjectUnavailable
                    | Rejection::WorkspaceUnavailable
                    | Rejection::WorkspaceMoveIndexOutOfBounds
                    | Rejection::WorkspaceMoveTargetInBlock
                    | Rejection::InvalidNavigationConfiguration
                    | Rejection::WorkspaceGroupCloseRequired
                    | Rejection::WorkspaceDirectoryConflict
                    | Rejection::TabGroupUnavailable
                    | Rejection::TabMoveIndexOutOfBounds
                    | Rejection::TabMoveOrderConflict
                    | Rejection::SplitUnavailable
                    | Rejection::ConfirmationRequired
                    | Rejection::SameTab => false,
                };
            }
            Outcome::Unavailable { reason } => {
                return match reason {
                    Unavailable::RequestLedgerFull
                    | Unavailable::ServiceUnavailable
                    | Unavailable::TerminalBusy => true,
                    Unavailable::NoCompatibleWorker
                    | Unavailable::ServerBusy
                    | Unavailable::TerminalRuntimeUnavailable
                    | Unavailable::DirectoryUnavailable => !structure,
                };
            }
            // Failure classes above are shared; success receipts still require exact action echoes.
            _ => {}
        }
        let Some(next) = self.expected_revision.value().checked_add(1) else {
            return false;
        };
        match (&self.action, outcome) {
            (
                WorkbenchAction::TabCreateLayout {
                    terminal_context_id: expected,
                    ..
                },
                Outcome::TabLayoutCreated {
                    revision,
                    terminal_context_id,
                    ..
                },
            ) => revision.value() == next && terminal_context_id == expected,
            (
                WorkbenchAction::PanelCreateTerminal {
                    terminal_context_id: context,
                    tab_id: tab,
                    panel_id: panel,
                    target,
                    geometry,
                    ..
                },
                Outcome::PanelTerminalPending {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id,
                }
                | Outcome::PanelTerminalCreated {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id,
                    ..
                },
            ) => {
                revision.value() == next
                    && terminal_context_id == context
                    && tab_id == tab
                    && panel_id == panel
                    && match target {
                        WorkbenchPanelTerminalTarget::Unbound {
                            presentation_id: expected,
                        } => presentation_id == expected,
                        WorkbenchPanelTerminalTarget::Child { .. } => true,
                    }
                    && !matches!(outcome,
                    Outcome::PanelTerminalCreated { terminal_outcome: WorkbenchTerminalOutcome::Started { geometry: actual }, .. } if actual != geometry)
            }
            // Creation admits only the two success classes above; other result classes are invalid.
            _ => false,
        }
    }
}
