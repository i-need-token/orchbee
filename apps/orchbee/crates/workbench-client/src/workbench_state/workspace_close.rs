//! Confirm and retain one Workspace Close or explicit Project close-all. Membership owns removal.
use super::recovery_fence::RecoveryFence;
use super::{
    ClientContextTabs, ClientNotice, ClientPhase, OperationDeadlines, PaneKey,
    PendingRefreshFlight, WorkbenchClientState, context_removal::ContextRemovalPermit,
};
use crate::exchange_values::workbench::WorkbenchWorkspaceCloseAccepted;
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use crate::workbench_projection::{ClientContext, ClientContextLocation};
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, Snapshot, WorkbenchAction, WorkbenchCommand, WorkbenchProjectId,
    WorkbenchRequestId, WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus,
    WorkbenchTerminalContextId, WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceGroupCloseRequest, WorkbenchWorkspaceGroupCloseResult, WorkbenchWorkspaceId,
    WorkspaceGroupCloseAction, WorkspaceGroupCloseOutcome, WorkspaceGroupCloseStatus,
};
use std::time::{Duration, Instant};
mod sidebar;
pub(crate) use sidebar::{SidebarDeleteReview, WorkspaceDeleteRequest};

/// The next step after a close that a newer projection kept from admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StaleCloseReview {
    /// The result is not stale, or its own refresh still owns settlement.
    NotApplicable,
    /// The confirmed scope is unchanged: submit it again against the newer projection.
    Retry,
    /// The conditional target is unchanged: ask the Server again.
    Recheck(WorkbenchWorkspaceId),
    /// The scope changed, or the bounded retries kept meeting newer projections.
    Changed,
}

/// Identifies exactly one fully drawn destructive confirmation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceCloseToken(u64);
#[cfg(any(test, feature = "test-support"))]
impl WorkspaceCloseToken {
    pub const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceCloseTarget {
    Workspace {
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
    },
    Project {
        project_id: WorkbenchProjectId,
        member_count: usize,
    },
}
impl WorkspaceCloseTarget {
    fn matches_snapshot(&self, baseline: &ClientContextTabs) -> bool {
        let snapshot = baseline.canonical_snapshot();
        match self {
            Self::Workspace {
                workspace_id,
                terminal_context_id,
            } => {
                snapshot
                    .navigation
                    .workspace(workspace_id)
                    .is_some_and(|workspace| &workspace.terminal_context_id == terminal_context_id)
                    && snapshot
                        .terminal_contexts
                        .iter()
                        .any(|context| &context.terminal_context_id == terminal_context_id)
            }
            Self::Project {
                project_id,
                member_count,
            } => {
                snapshot.navigation.project(project_id).is_some()
                    && snapshot
                        .navigation
                        .workspaces_in_project(Some(project_id))
                        .count()
                        == *member_count
            }
        }
    }
    /// Unrelated snapshot changes keep a frozen scope valid; a change to the scope itself does
    /// not. A Workspace scope speaks for its Workspace and whole terminal Context; a Project scope
    /// for the Project and every member Workspace.
    fn unchanged_between(&self, before: &ClientContextTabs, after: &ClientContextTabs) -> bool {
        before.session_id() == after.session_id()
            && self.matches_snapshot(before)
            && self.matches_snapshot(after)
            && match self {
                Self::Workspace { workspace_id, .. } => {
                    sidebar::target_unchanged(before, after, workspace_id)
                }
                Self::Project { project_id, .. } => {
                    project_facts(before, project_id) == project_facts(after, project_id)
                }
            }
    }
    fn project_member_count(&self) -> Option<usize> {
        match self {
            Self::Project { member_count, .. } => Some(*member_count),
            Self::Workspace { .. } => None,
        }
    }
    fn workspace_id(&self) -> Option<&WorkbenchWorkspaceId> {
        match self {
            Self::Workspace { workspace_id, .. } => Some(workspace_id),
            Self::Project { .. } => None,
        }
    }
}

type ProjectFacts<'a> = (
    &'a orchbee_workbench_protocol::Project,
    Vec<&'a orchbee_workbench_protocol::Workspace>,
);
fn project_facts<'a>(
    cache: &'a ClientContextTabs,
    project_id: &'a WorkbenchProjectId,
) -> Option<ProjectFacts<'a>> {
    let navigation = &cache.canonical_snapshot().navigation;
    Some((
        navigation.project(project_id)?,
        navigation.workspaces_in_project(Some(project_id)).collect(),
    ))
}

/// Retries of a request that only a newer projection kept from admission, while its scope stays
/// unchanged. Each takes one read round trip, so the bound stays within a few seconds.
pub(crate) const STALE_RETRY_LIMIT: u8 = 4;

/// The pause between settlement reads. Physical Close settlement has no complete progress
/// notification in this protocol, so an accepted close is sampled while it stays pending.
const FOLLOW_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCloseOperation {
    conditional: bool,
    destructive: bool,
    /// An automatic follow-up: the single one after the target changed under a conditional
    /// reply, or one of the bounded ones after a newer projection kept the Server from admitting
    /// the previous request.
    recheck: bool,
    /// How many earlier requests a newer projection kept from admission, every one over an
    /// unchanged scope. Commits elsewhere in the Workbench advance the revision continuously
    /// while other Workspaces work, so one retry is not enough; the bound keeps it finite.
    pub(crate) stale_retries: u8,
    pub(crate) target: WorkspaceCloseTarget,
    pub(crate) request_id: WorkbenchRequestId,
    pub(crate) generation: u64,
    pub(crate) deadlines: OperationDeadlines,
    pub(crate) baseline: ClientContextTabs,
    desired: Option<PaneKey>,
    label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceCloseResult {
    NotSubmitted,
    Refused,
    /// Nothing was committed: the projection advanced past the operation's baseline before the
    /// Server admitted it, so the request names an outdated revision rather than a refused target.
    Stale,
    ConfirmationRequired,
    Unknown,
    WorkspaceAccepted(WorkbenchWorkspaceCloseAccepted),
    WorkspaceObserved {
        accepted: WorkbenchWorkspaceCloseAccepted,
        outcome: Box<WorkbenchWorkspaceCloseReadOutcome>,
    },
    ProjectObserved(Box<WorkbenchWorkspaceGroupCloseResult>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCloseEvidence {
    pub(crate) operation: WorkspaceCloseOperation,
    pub(crate) result: WorkspaceCloseResult,
}
impl WorkspaceCloseResult {
    pub(crate) fn is_closed(&self) -> bool {
        matches!(self,
        Self::WorkspaceObserved { outcome, .. }
            if matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closed { .. }, ..
            }))
            || matches!(self, Self::ProjectObserved(reply) if matches!(reply.outcome,
                WorkspaceGroupCloseOutcome::Available { status: WorkspaceGroupCloseStatus::Closed {}, .. }))
    }
    pub(crate) fn is_pending(&self) -> bool {
        matches!(self, Self::WorkspaceAccepted(_))
            || matches!(self,
            Self::WorkspaceObserved { outcome, .. }
                if matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                    status: WorkbenchTabCloseStatus::Closing { .. }
                        | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                            reason: WorkbenchTabClosePublicationReason::Preparing
                                | WorkbenchTabClosePublicationReason::TargetCleanupPending,
                            resumable: false,
                        }, ..
                }))
            || matches!(self, Self::ProjectObserved(reply) if matches!(reply.outcome,
                WorkspaceGroupCloseOutcome::Available { status: WorkspaceGroupCloseStatus::Pending {}, .. }))
    }
    fn has_admission(&self) -> bool {
        matches!(
            self,
            Self::WorkspaceAccepted(_) | Self::WorkspaceObserved { .. } | Self::ProjectObserved(_)
        )
    }
    fn unresolved(&self) -> bool {
        !matches!(
            self,
            Self::NotSubmitted | Self::Refused | Self::Stale | Self::ConfirmationRequired
        ) && !self.is_closed()
    }
    fn status(&self) -> WorkspaceCloseStatus {
        if self.is_closed() {
            return WorkspaceCloseStatus::Closed;
        }
        if self.is_pending() {
            return WorkspaceCloseStatus::Closing;
        }
        match self {
            Self::NotSubmitted | Self::Refused | Self::Stale => WorkspaceCloseStatus::Refused,
            Self::ConfirmationRequired => WorkspaceCloseStatus::ConfirmationRequired,
            Self::Unknown => WorkspaceCloseStatus::Unknown,
            Self::WorkspaceObserved { outcome, .. }
                if matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                    status: WorkbenchTabCloseStatus::OutcomeUnknown { .. }, ..
                }) => WorkspaceCloseStatus::Unknown,
            Self::ProjectObserved(reply)
                if matches!(reply.outcome, WorkspaceGroupCloseOutcome::Available {
                    status: WorkspaceGroupCloseStatus::Failed {
                        reason: orchbee_workbench_protocol::WorkspaceGroupCloseFailure::OutcomeUnknown, ..
                    }, ..
                }) => WorkspaceCloseStatus::Unknown,
            // This Close continuation requires the corresponding published receipt; other child states cannot prove final disposal.
            _ => WorkspaceCloseStatus::Failed,
        }
    }
}
impl WorkspaceCloseOperation {
    pub(crate) fn workspace_command(&self) -> Option<WorkbenchCommand> {
        let WorkspaceCloseTarget::Workspace { workspace_id, .. } = &self.target else {
            return None;
        };
        Some(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: self.request_id.clone(),
            session_id: self.baseline.session_id().clone(),
            expected_revision: self.baseline.revision(),
            action: if self.conditional {
                WorkbenchAction::WorkspaceCloseIfIdle {
                    workspace_id: workspace_id.clone(),
                }
            } else {
                WorkbenchAction::WorkspaceClose {
                    workspace_id: workspace_id.clone(),
                }
            },
        })
    }
    pub(crate) fn project_request(&self) -> Option<WorkbenchWorkspaceGroupCloseRequest> {
        let WorkspaceCloseTarget::Project { project_id, .. } = &self.target else {
            return None;
        };
        Some(WorkbenchWorkspaceGroupCloseRequest {
            protocol_version: ProtocolVersion,
            request_id: self.request_id.clone(),
            session_id: self.baseline.session_id().clone(),
            action: WorkspaceGroupCloseAction::Start {
                operation_id: self.request_id.clone(),
                project_id: project_id.clone(),
            },
        })
    }
    fn admits_accepted(&self, accepted: &WorkbenchWorkspaceCloseAccepted) -> bool {
        matches!(&self.target, WorkspaceCloseTarget::Workspace { workspace_id, terminal_context_id }
            if accepted.workspace_id == *workspace_id
                && accepted.terminal_context_id == *terminal_context_id
                && accepted.accepted_revision.value() != u64::MAX
                && accepted.accepted_revision <= self.baseline.revision()
                && (accepted.close_request_id != self.request_id
                    || accepted.accepted_revision == self.baseline.revision()))
    }
    pub(crate) fn admits(&self, result: &WorkspaceCloseResult) -> bool {
        if !self.target.matches_snapshot(&self.baseline) {
            return false;
        }
        match result {
            WorkspaceCloseResult::NotSubmitted
            | WorkspaceCloseResult::Refused
            | WorkspaceCloseResult::Stale
            | WorkspaceCloseResult::Unknown => true,
            WorkspaceCloseResult::ConfirmationRequired => self.conditional,
            WorkspaceCloseResult::WorkspaceAccepted(accepted) => self.admits_accepted(accepted),
            WorkspaceCloseResult::WorkspaceObserved { accepted, outcome } => {
                self.admits_accepted(accepted)
                    && matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                        close_request_id, workspace_id, terminal_context_id, status, ..
                    } if close_request_id == &accepted.close_request_id
                        && workspace_id == &accepted.workspace_id
                        && terminal_context_id == &accepted.terminal_context_id
                        && (!matches!(status, WorkbenchTabCloseStatus::Closed { revision }
                            if revision <= &accepted.accepted_revision)))
            }
            WorkspaceCloseResult::ProjectObserved(reply) => {
                matches!(&self.target, WorkspaceCloseTarget::Project { project_id, .. }
                    if reply.operation_id == self.request_id
                        && reply.validate().is_ok()
                        && matches!(&reply.outcome, WorkspaceGroupCloseOutcome::Available {
                            project_id: actual, ..
                        } if actual == project_id))
            }
        }
    }
}
struct Confirmation {
    destructive: bool,
    token: WorkspaceCloseToken,
    target: WorkspaceCloseTarget,
    baseline: ClientContextTabs,
    desired: Option<PaneKey>,
    label: String,
    presentation: PresentationPhase,
    shown: bool,
}
#[derive(Default)]
pub(super) struct WorkspaceCloseState {
    next_token: u64,
    confirmation: Option<Confirmation>,
    pub(super) pending: Option<WorkspaceCloseOperation>,
    evidence: Option<WorkspaceCloseEvidence>,
    /// Present while settlement reads follow the retained accepted close after its submission
    /// lease ended, and while a close that settled Closed still waits for its landing. Accepted
    /// evidence without it can no longer settle here and reads as unknown.
    follow: Option<CloseFollow>,
    automatic_landing: Option<ClosedLanding>,
    show_pending: bool,
    /// The Closed result is a one-shot notice; retiring it keeps the evidence.
    closed_notice_retired: bool,
}

/// Paces the settlement reads of one accepted close, one at a time, each in its own lease.
struct CloseFollow {
    /// When the next read may be admitted.
    due: Instant,
    /// The operation's original forward cutoff. No read is admitted after it, and a close still
    /// pending then ends unsettled.
    until: Instant,
    /// The close read Closed while the Workspace it removes is still current, because no other
    /// Workspace was shown to land on. No read follows; the follow only keeps the Closing status
    /// until publication lands the Client elsewhere or the cutoff passes.
    settled: bool,
}

/// One repeatable status read of the retained accepted close. It names the Server's admission and
/// never carries the close request, so no read can submit the close again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCloseSettlementRead {
    generation: u64,
    session_id: SessionId,
    target: SettlementTarget,
    deadline: Instant,
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum SettlementTarget {
    Workspace(WorkbenchWorkspaceCloseAccepted),
    /// The group operation, whose identity is the Start request's.
    Project(WorkbenchRequestId),
}
impl WorkspaceCloseSettlementRead {
    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }
    pub(crate) const fn deadline(&self) -> Instant {
        self.deadline
    }
    /// The Workspace status read and the canonical close identity it must answer for, or `None`
    /// for a Project close.
    pub(crate) fn workspace_request(
        &self,
        request_id: WorkbenchRequestId,
    ) -> Option<(WorkbenchWorkspaceCloseRead, &WorkbenchRequestId)> {
        let SettlementTarget::Workspace(accepted) = &self.target else {
            return None;
        };
        Some((
            WorkbenchWorkspaceCloseRead {
                protocol_version: ProtocolVersion,
                request_id,
                session_id: self.session_id.clone(),
                workspace_id: accepted.workspace_id.clone(),
            },
            &accepted.close_request_id,
        ))
    }
    /// The group status read, or `None` for a Workspace close.
    pub(crate) fn project_request(
        &self,
        request_id: WorkbenchRequestId,
    ) -> Option<WorkbenchWorkspaceGroupCloseRequest> {
        let SettlementTarget::Project(operation_id) = &self.target else {
            return None;
        };
        Some(WorkbenchWorkspaceGroupCloseRequest {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: self.session_id.clone(),
            action: WorkspaceGroupCloseAction::Read {
                operation_id: operation_id.clone(),
            },
        })
    }
    /// What a Workspace read observed, or `None` when the Server no longer answers for the close.
    pub(crate) fn workspace_observation(
        &self,
        outcome: WorkbenchWorkspaceCloseReadOutcome,
    ) -> Option<WorkspaceCloseResult> {
        let SettlementTarget::Workspace(accepted) = &self.target else {
            return None;
        };
        matches!(
            outcome,
            WorkbenchWorkspaceCloseReadOutcome::Available { .. }
        )
        .then(|| WorkspaceCloseResult::WorkspaceObserved {
            accepted: accepted.clone(),
            outcome: Box::new(outcome),
        })
    }
    /// What a group read observed, or `None` when the Server no longer answers for the close.
    pub(crate) fn project_observation(
        reply: WorkbenchWorkspaceGroupCloseResult,
    ) -> Option<WorkspaceCloseResult> {
        matches!(reply.outcome, WorkspaceGroupCloseOutcome::Available { .. })
            .then(|| WorkspaceCloseResult::ProjectObserved(Box::new(reply)))
    }
}

/// Display continuity for one validated removal install, never navigation or input authority.
struct ClosedLanding {
    request_id: WorkbenchRequestId,
    generation: u64,
    location: ClientContextLocation,
}
/// Read-only scope and draw identity of one pending confirmation.
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceCloseConfirmationView<'a> {
    inner: &'a Confirmation,
}
impl<'a> WorkspaceCloseConfirmationView<'a> {
    #[cfg(test)]
    pub(crate) fn destructive(self) -> bool {
        self.inner.destructive
    }
    pub(crate) const fn token(self) -> WorkspaceCloseToken {
        self.inner.token
    }
    pub(crate) fn label(self) -> &'a str {
        &self.inner.label
    }
    /// A Project confirmation names its captured member count, including zero.
    pub(crate) fn project_member_count(self) -> Option<usize> {
        self.inner.target.project_member_count()
    }
    pub(crate) const fn interactive(self) -> bool {
        self.inner.shown && self.inner.presentation.interactive()
    }
    pub(crate) const fn presentation(self) -> PresentationPhase {
        self.inner.presentation
    }
}
pub(crate) use crate::localization::WorkspaceCloseStatus;
/// Scope and recovery identity retained independently of the current selection.
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceCloseStatusView<'a> {
    operation: &'a WorkspaceCloseOperation,
    operation_id: &'a WorkbenchRequestId,
    status: WorkspaceCloseStatus,
}
impl<'a> WorkspaceCloseStatusView<'a> {
    pub(crate) fn label(self) -> &'a str {
        &self.operation.label
    }
    pub(crate) const fn status(self) -> WorkspaceCloseStatus {
        self.status
    }
    pub(crate) fn operation_id(self) -> Option<&'a WorkbenchRequestId> {
        (!matches!(
            self.status,
            WorkspaceCloseStatus::Refused | WorkspaceCloseStatus::ConfirmationRequired
        ))
        .then_some(self.operation_id)
    }
    /// Ordinary recovery is scoped by stable Workspace identity, independent of its Project.
    pub(crate) fn workspace_id(self) -> Option<&'a WorkbenchWorkspaceId> {
        self.operation.target.workspace_id()
    }
}
impl<'a> From<WorkspaceCloseStatusView<'a>> for crate::localization::WorkspaceCloseText<'a> {
    fn from(view: WorkspaceCloseStatusView<'a>) -> Self {
        Self {
            label: view.label(),
            status: view.status(),
            operation_id: view.operation_id(),
            workspace_id: view.workspace_id(),
        }
    }
}

impl WorkbenchClientState {
    /// A known guarded-signal refusal can be reviewed explicitly without
    /// discarding the retained operation or weakening unknown-outcome fences.
    pub(crate) fn can_confirm_guarded_workspace_close(
        &self,
        workspace: &WorkbenchWorkspaceId,
    ) -> bool {
        self.workspace_close_quiet()
            && self.invariant_holds()
            && self.workspace_close.confirmation.is_none()
            && self.workspace_close.pending.is_none()
            && self.workspace_close.evidence.as_ref().is_some_and(|evidence| {
                evidence.operation.conditional
                    && evidence.operation.destructive
                    && evidence.operation.target.workspace_id() == Some(workspace)
                    && evidence.operation.target.matches_snapshot(&self.cache)
                    && matches!(&evidence.result, WorkspaceCloseResult::WorkspaceObserved { outcome, .. }
                        if matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                            status: WorkbenchTabCloseStatus::CleanupBlocked {
                                reason: orchbee_workbench_protocol::WorkbenchTabCloseBlockedReason::HostRefused,
                                resumable: true,
                            }, ..
                        }))
            })
    }
    pub(super) fn workspace_close_invariant_holds(&self) -> bool {
        self.workspace_close
            .pending
            .as_ref()
            .is_none_or(|operation| {
                operation.baseline == self.cache
                    && operation.target.matches_snapshot(&operation.baseline)
                    && (self.pending_refresh_generation() == Some(operation.generation)
                        || matches!(self.phase, ClientPhase::Retired(_)))
            })
            && self
                .workspace_close
                .evidence
                .as_ref()
                .is_none_or(|evidence| evidence.operation.admits(&evidence.result))
            && self.workspace_close.follow.as_ref().is_none_or(|follow| {
                self.workspace_close.pending.is_none()
                    && self
                        .workspace_close
                        .evidence
                        .as_ref()
                        .is_some_and(|evidence| {
                            if follow.settled {
                                evidence.result.is_closed()
                            } else {
                                evidence.result.is_pending()
                            }
                        })
            })
    }
    /// Whether the retained result reads as closed: settled Closed, or accepted while settlement
    /// reads still follow it. Acceptance is announced at once; a later failure replaces it.
    fn reads_closed(&self, result: &WorkspaceCloseResult) -> bool {
        result.is_closed() || (result.is_pending() && self.workspace_close.follow.is_some())
    }
    /// A close owns the refresh slot or its confirmation, or its outcome stays unresolved with no
    /// read following it. A followed accepted close leaves refresh and navigation among the
    /// remaining Workspaces to the user.
    pub(crate) fn workspace_close_blocks_refresh(&self) -> bool {
        self.workspace_close_blocks_membership()
            || self
                .workspace_close
                .evidence
                .as_ref()
                .is_some_and(|evidence| {
                    evidence.result.unresolved() && !self.reads_closed(&evidence.result)
                })
    }
    /// Structure also waits while the current Workspace is the one leaving, so nothing selects
    /// from, attaches to, or changes it before its landing.
    pub(crate) fn workspace_close_blocks_structure(&self) -> bool {
        self.workspace_close_blocks_refresh() || self.current_workspace_leaves()
    }
    /// Whether the current location is a Workspace an accepted close takes out of navigation.
    pub(crate) fn current_workspace_leaves(&self) -> bool {
        current_leaves(self.cache.location(), &self.leaving_workspace_ids())
    }
    /// Finished evidence still fences effects, but canonical read-only membership repair may run.
    pub(crate) fn workspace_close_blocks_membership(&self) -> bool {
        self.workspace_close.confirmation.is_some() || self.workspace_close.pending.is_some()
    }
    fn workspace_close_quiet(&self) -> bool {
        !self.own_refresh_pending()
            && self.pending_mutation.is_none()
            && self.pending_membership.is_none()
            && !self.setup_blocks_structure()
            && !self.rename_draft_blocks_structure()
            && !self.recovery_fences(RecoveryFence::Removal)
            && self.close.permits_retired_membership()
            && self.panel_close.confirmation().is_none()
            && matches!(
                self.phase,
                ClientPhase::Empty
                    | ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
    }
    /// Shared availability for either Close scope; opening additionally resolves its exact target.
    /// A close whose outcome is unresolved, including one still followed, keeps its recovery
    /// identity, so no second close starts beside it.
    pub(crate) fn close_scope_available(&self) -> bool {
        self.workspace_close_quiet()
            && !self.workspace_close_blocks_structure()
            && !self
                .workspace_close
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence.result.unresolved())
            && self.invariant_holds()
    }
    pub(crate) fn open_workspace_close(&mut self) -> Option<WorkspaceCloseToken> {
        let ClientContextLocation::Workspace { workspace_id, .. } = self.cache.location() else {
            self.set_notice(ClientNotice::WorkspaceCloseUnavailable);
            return None;
        };
        self.open_workspace_close_at(&workspace_id.clone())
    }
    /// Freezes an addressed Workspace while retaining the current Context and attachment.
    pub(crate) fn open_workspace_close_at(
        &mut self,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> Option<WorkspaceCloseToken> {
        let Some(workspace) = self
            .cache
            .canonical_snapshot()
            .navigation
            .workspace(workspace_id)
        else {
            self.set_notice(ClientNotice::WorkspaceCloseUnavailable);
            return None;
        };
        self.open_workspace_scope_close_confirmation(
            WorkspaceCloseTarget::Workspace {
                workspace_id: workspace_id.clone(),
                terminal_context_id: workspace.terminal_context_id.clone(),
            },
            workspace.label.as_str().to_owned(),
        )
    }
    /// Opens explicit Project close-all without selecting or borrowing an anchor Workspace.
    pub(crate) fn open_project_close_all(
        &mut self,
        project_id: &WorkbenchProjectId,
    ) -> Option<WorkspaceCloseToken> {
        let navigation = &self.cache.canonical_snapshot().navigation;
        let Some(project) = navigation
            .projects
            .iter()
            .find(|project| &project.project_id == project_id)
        else {
            self.set_notice(ClientNotice::WorkspaceCloseUnavailable);
            return None;
        };
        let target = WorkspaceCloseTarget::Project {
            project_id: project_id.clone(),
            member_count: navigation.workspaces_in_project(Some(project_id)).count(),
        };
        self.open_workspace_scope_close_confirmation(target, project.label.as_str().to_owned())
    }
    fn open_workspace_scope_close_confirmation(
        &mut self,
        target: WorkspaceCloseTarget,
        label: String,
    ) -> Option<WorkspaceCloseToken> {
        if !self.close_scope_available()
            && !target
                .workspace_id()
                .is_some_and(|workspace| self.can_confirm_guarded_workspace_close(workspace))
        {
            self.set_notice(ClientNotice::LocalBusy);
            return None;
        }
        let next = self.workspace_close.next_token.checked_add(1)?;
        self.workspace_close.next_token = next;
        let token = WorkspaceCloseToken(next);
        self.workspace_close.confirmation = Some(Confirmation {
            destructive: false,
            token,
            target,
            baseline: self.cache.clone(),
            desired: self.desired.clone(),
            label,
            presentation: PresentationPhase::Pending,
            shown: false,
        });
        self.notice = None;
        Some(token)
    }
    pub(crate) fn workspace_close_confirmation(
        &self,
    ) -> Option<WorkspaceCloseConfirmationView<'_>> {
        self.workspace_close
            .confirmation
            .as_ref()
            .map(|inner| WorkspaceCloseConfirmationView { inner })
    }
    /// Ends the confirmation together with the conditional reply it continues, so a cancelled
    /// prompt leaves no confirmation-required status behind.
    pub(crate) fn cancel_workspace_close(&mut self) {
        self.workspace_close.confirmation = None;
        if self
            .workspace_close
            .evidence
            .as_ref()
            .is_some_and(|evidence| {
                matches!(evidence.result, WorkspaceCloseResult::ConfirmationRequired)
            })
        {
            self.workspace_close.evidence = None;
        }
    }
    pub(crate) fn acknowledge_workspace_close_draw(&mut self, token: Option<WorkspaceCloseToken>) {
        if let Some(prompt) = &mut self.workspace_close.confirmation {
            prompt.shown = token == Some(prompt.token);
            if prompt.shown {
                prompt.presentation.presented();
            }
        }
    }
    /// Refuses a new prompt whose first draw is geometry-blocked, or suspends
    /// an admitted prompt without discarding its frozen scope.
    pub(crate) fn settle_workspace_close_not_presented(&mut self) -> Option<GeometrySettlement> {
        let prompt = self.workspace_close.confirmation.as_mut()?;
        prompt.shown = false;
        let settlement = prompt.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.cancel_workspace_close();
            self.set_notice(ClientNotice::DialogRefused);
        }
        Some(settlement)
    }
    /// Keeps an open confirmation across unrelated publications by rebasing its frozen scope; a
    /// change to the scope itself ends it with the changed-target notice.
    pub(crate) fn settle_stale_workspace_close(&mut self) -> bool {
        let Some(prompt) = self.workspace_close.confirmation.as_ref() else {
            return false;
        };
        if prompt.desired != self.desired || !self.workspace_close_quiet() {
            self.cancel_workspace_close();
            self.set_notice(ClientNotice::ConfirmationCancelled);
            return true;
        }
        if prompt.baseline == self.cache && prompt.target.matches_snapshot(&prompt.baseline) {
            return false;
        }
        if prompt
            .target
            .unchanged_between(&prompt.baseline, &self.cache)
        {
            let cache = self.cache.clone();
            if let Some(prompt) = self.workspace_close.confirmation.as_mut() {
                prompt.baseline = cache;
            }
            return false;
        }
        self.cancel_workspace_close();
        self.set_notice(ClientNotice::WorkspaceChanged);
        true
    }
    pub(crate) fn plan_workspace_close(
        &self,
        token: WorkspaceCloseToken,
        request_id: WorkbenchRequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<WorkspaceCloseOperation> {
        let prompt = self.workspace_close.confirmation.as_ref()?;
        // An unrelated publication since the prompt was drawn rebases the frozen scope; a change
        // to the scope itself never reaches the Server.
        if !prompt.shown
            || prompt.token != token
            || !prompt.target.matches_snapshot(&prompt.baseline)
            || (prompt.baseline != self.cache
                && !prompt
                    .target
                    .unchanged_between(&prompt.baseline, &self.cache))
            || prompt.desired != self.desired
            || !self.workspace_close_quiet()
            || generation == 0
        {
            return None;
        }
        Some(WorkspaceCloseOperation {
            conditional: false,
            destructive: prompt.destructive,
            recheck: false,
            stale_retries: 0,
            target: prompt.target.clone(),
            request_id,
            generation,
            deadlines,
            baseline: self.cache.clone(),
            desired: prompt.desired.clone(),
            label: prompt.label.clone(),
        })
    }
    pub(crate) fn begin_workspace_close(&mut self, operation: WorkspaceCloseOperation) -> bool {
        if operation.conditional {
            if !self.sidebar_workspace_delete_current(&operation) {
                return false;
            }
            self.install_workspace_close(operation);
            return true;
        }
        if operation.recheck {
            let Some(prior) = self.stale_workspace_close().cloned() else {
                return false;
            };
            if self
                .plan_workspace_close_retry(
                    &prior,
                    operation.request_id.clone(),
                    operation.generation,
                    operation.deadlines,
                )
                .as_ref()
                != Some(&operation)
            {
                return false;
            }
            self.install_workspace_close(operation);
            return true;
        }
        let Some(token) = self
            .workspace_close
            .confirmation
            .as_ref()
            .map(|prompt| prompt.token)
        else {
            return false;
        };
        if self
            .plan_workspace_close(
                token,
                operation.request_id.clone(),
                operation.generation,
                operation.deadlines,
            )
            .as_ref()
            != Some(&operation)
        {
            return false;
        }
        self.cancel_workspace_close();
        self.install_workspace_close(operation);
        true
    }
    /// The operation of a retained close that a newer projection kept from admission.
    fn stale_workspace_close(&self) -> Option<&WorkspaceCloseOperation> {
        self.workspace_close
            .evidence
            .as_ref()
            .filter(|evidence| matches!(evidence.result, WorkspaceCloseResult::Stale))
            .map(|evidence| &evidence.operation)
    }
    /// Re-plans a confirmed close against the newer projection after its request named an
    /// outdated revision, up to the stale retry bound. The user already confirmed this exact
    /// unchanged scope.
    pub(crate) fn plan_workspace_close_retry(
        &self,
        prior: &WorkspaceCloseOperation,
        request_id: WorkbenchRequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<WorkspaceCloseOperation> {
        if prior.conditional
            || prior.stale_retries >= STALE_RETRY_LIMIT
            || generation == 0
            || self.stale_workspace_close() != Some(prior)
            || !self.close_scope_available()
            || !prior.target.unchanged_between(&prior.baseline, &self.cache)
        {
            return None;
        }
        Some(WorkspaceCloseOperation {
            conditional: false,
            destructive: prior.destructive,
            recheck: true,
            stale_retries: prior.stale_retries.saturating_add(1),
            target: prior.target.clone(),
            request_id,
            generation,
            deadlines,
            baseline: self.cache.clone(),
            desired: self.desired.clone(),
            label: prior.label.clone(),
        })
    }
    /// What follows a close that a newer projection kept from admission. No variant leaves the
    /// outdated request as a refusal status.
    pub(crate) fn review_stale_workspace_close(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
    ) -> StaleCloseReview {
        if !matches!(evidence.result, WorkspaceCloseResult::Stale)
            || self.pending_refresh_generation() == Some(evidence.operation.generation)
            || matches!(self.phase, ClientPhase::Retired(_))
        {
            return StaleCloseReview::NotApplicable;
        }
        if evidence.operation.stale_retries < STALE_RETRY_LIMIT
            && self.close_scope_available()
            && evidence
                .operation
                .target
                .unchanged_between(&evidence.operation.baseline, &self.cache)
        {
            return if evidence.operation.conditional {
                evidence
                    .operation
                    .target
                    .workspace_id()
                    .map_or(StaleCloseReview::NotApplicable, |workspace| {
                        StaleCloseReview::Recheck(workspace.clone())
                    })
            } else {
                StaleCloseReview::Retry
            };
        }
        self.settle_sidebar_workspace_delete_changed();
        StaleCloseReview::Changed
    }
    fn install_workspace_close(&mut self, operation: WorkspaceCloseOperation) {
        self.pending_refresh = Some(PendingRefreshFlight::recovery(operation.generation));
        self.workspace_close.pending = Some(operation);
        self.workspace_close.evidence = None;
        self.workspace_close.follow = None;
        self.workspace_close.automatic_landing = None;
        self.workspace_close.show_pending = false;
        self.workspace_close.closed_notice_retired = false;
        self.adopt_publication_debt();
    }
    pub(crate) fn tick_workspace_close_pending(&mut self, sampled: Instant) -> bool {
        if !self.workspace_close.show_pending
            && self
                .workspace_close
                .pending
                .as_ref()
                .is_some_and(|operation| {
                    sampled
                        >= operation.deadlines.total() - super::OPERATION_TOTAL
                            + Duration::from_millis(200)
                })
        {
            self.workspace_close.show_pending = true;
            return true;
        }
        false
    }
    pub(crate) fn retain_workspace_close_evidence(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
    ) -> bool {
        if !evidence.operation.admits(&evidence.result) {
            return false;
        }
        // Late completion may neither replace a later operation nor move its selection.
        if evidence.result.is_closed()
            && self
                .workspace_close
                .pending
                .as_ref()
                .is_some_and(|pending| pending.generation > evidence.operation.generation)
        {
            return true;
        }
        if let Some(prior) = &self.workspace_close.evidence {
            if prior.operation != evidence.operation {
                return evidence.result.is_closed()
                    && prior.operation.generation > evidence.operation.generation;
            }
            if prior.result.has_admission() && !evidence.result.has_admission() {
                return true;
            }
            if !retains_close_result(&prior.result, &evidence.result) {
                return false;
            }
        } else if self.workspace_close.pending.as_ref() != Some(&evidence.operation) {
            return false;
        }
        self.workspace_close.evidence = Some(evidence.clone());
        true
    }
    pub(crate) fn finish_workspace_close(&mut self, evidence: &WorkspaceCloseEvidence) -> bool {
        if !self.retain_workspace_close_evidence(evidence) {
            return false;
        }
        if self.workspace_close.pending.as_ref() == Some(&evidence.operation) {
            self.workspace_close.pending = None;
        }
        true
    }
    /// Retains the submission lease's published result. An accepted close that is still pending
    /// is then followed by settlement reads until the operation's original forward cutoff; a
    /// result recovered after worker loss goes through `finish_workspace_close` and is not.
    pub(crate) fn accept_workspace_close_event(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
        sampled: Instant,
    ) -> bool {
        if !self.finish_workspace_close(evidence) {
            return false;
        }
        if evidence.result.is_pending() && self.workspace_close.evidence.as_ref() == Some(evidence)
        {
            self.workspace_close.follow = Some(CloseFollow {
                due: sampled,
                until: evidence.operation.deadlines.forward_cutoff(),
                settled: false,
            });
        }
        true
    }
    /// The next settlement read, when one is due at `sampled`. The caller keeps at most one in
    /// flight.
    pub(crate) fn workspace_close_read_due(
        &self,
        sampled: Instant,
    ) -> Option<WorkspaceCloseSettlementRead> {
        self.workspace_close
            .follow
            .as_ref()
            .filter(|follow| !follow.settled && follow.due <= sampled && sampled < follow.until)?;
        let evidence = self.workspace_close.evidence.as_ref()?;
        let target = match &evidence.result {
            WorkspaceCloseResult::WorkspaceAccepted(accepted)
            | WorkspaceCloseResult::WorkspaceObserved { accepted, .. } => {
                SettlementTarget::Workspace(accepted.clone())
            }
            WorkspaceCloseResult::ProjectObserved(_) => {
                SettlementTarget::Project(evidence.operation.request_id.clone())
            }
            _ => return None,
        };
        Some(WorkspaceCloseSettlementRead {
            generation: evidence.operation.generation,
            session_id: evidence.operation.baseline.session_id().clone(),
            target,
            deadline: sampled.checked_add(super::OPERATION_TOTAL)?,
        })
    }
    /// Ends a follow whose cutoff passed before settlement. The caller asks only while no read is
    /// in flight, so a read already sent still decides. The close then reads as unknown and its
    /// rows return; the selection stays where it is.
    pub(crate) fn expire_workspace_close_follow(&mut self, sampled: Instant) -> bool {
        if self
            .workspace_close
            .follow
            .as_ref()
            .is_some_and(|follow| !follow.settled && sampled >= follow.until)
        {
            self.workspace_close.follow = None;
            return true;
        }
        false
    }
    /// Ends the wait of a close that settled Closed once publication landed the Client off the
    /// closed Workspace, or at the cutoff, after which Closed shows beside it. Returns whether
    /// the status can change.
    pub(crate) fn release_settled_workspace_close_follow(&mut self, sampled: Instant) -> bool {
        let release = self.workspace_close.follow.as_ref().is_some_and(|follow| {
            follow.settled && (sampled >= follow.until || !self.current_workspace_leaves())
        });
        if release {
            self.workspace_close.follow = None;
        }
        release
    }
    /// A lost Client Connection ends the follow without deciding anything: the retained evidence
    /// stays, and an accepted close that has not settled reads as unknown.
    pub(crate) fn end_workspace_close_follow(&mut self) {
        self.workspace_close.follow = None;
    }
    /// Applies one settlement read of the close with refresh `generation`. `None` says the Server
    /// no longer answers for the close, so it can no longer settle here and the follow ends. A
    /// settled result ends the follow as well, except that Closed keeps it, without reads, while
    /// the closed Workspace is still current. Returns whether the retained result changed, or
    /// `None` when the observation contradicts the retained admission.
    pub(crate) fn observe_workspace_close(
        &mut self,
        generation: u64,
        observation: Option<WorkspaceCloseResult>,
        sampled: Instant,
    ) -> Option<bool> {
        let followed = self.workspace_close.follow.is_some()
            && self
                .workspace_close
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence.operation.generation == generation);
        if !followed {
            // The follow this read answered already ended; its outcome no longer waits for it.
            return Some(false);
        }
        let Some(result) = observation else {
            self.workspace_close.follow = None;
            return Some(true);
        };
        let evidence = self.workspace_close.evidence.as_mut()?;
        if !evidence.operation.admits(&result) || !retains_close_result(&evidence.result, &result) {
            return None;
        }
        let changed = evidence.result != result;
        let pending = result.is_pending();
        let closed = result.is_closed();
        evidence.result = result;
        // With no other Workspace shown, the closed Workspace stays current until publication
        // lands the Client elsewhere; Closed is announced with that landing.
        let landing_awaited = closed && self.current_workspace_leaves();
        match self.workspace_close.follow.as_mut() {
            Some(follow) if pending => {
                // The next read waits one interval. None is admitted at the cutoff, where the
                // expiry check ends the follow instead.
                follow.due = sampled
                    .checked_add(FOLLOW_INTERVAL)
                    .map_or(follow.until, |next| next.min(follow.until));
                Some(changed)
            }
            Some(follow) if landing_awaited => {
                follow.settled = true;
                Some(true)
            }
            _ => {
                self.workspace_close.follow = None;
                Some(true)
            }
        }
    }
    pub(crate) fn workspace_close_deadlines(&self) -> Option<OperationDeadlines> {
        self.workspace_close
            .evidence
            .as_ref()
            .filter(|evidence| {
                self.pending_refresh_generation() == Some(evidence.operation.generation)
            })
            .map(|evidence| evidence.operation.deadlines)
    }
    pub(crate) fn workspace_close_status(&self) -> Option<WorkspaceCloseStatusView<'_>> {
        if let Some(evidence) = &self.workspace_close.evidence {
            let closed = self.reads_closed(&evidence.result);
            if closed && self.workspace_close.closed_notice_retired {
                return None;
            }
            if closed
                && self.cache.location() != evidence.operation.baseline.location()
                && !self
                    .workspace_close
                    .automatic_landing
                    .as_ref()
                    .is_some_and(|landing| {
                        landing.request_id == evidence.operation.request_id
                            && landing.generation == evidence.operation.generation
                            && &landing.location == self.cache.location()
                    })
            {
                return None;
            }
            let operation_id = match &evidence.result {
                WorkspaceCloseResult::WorkspaceAccepted(accepted)
                | WorkspaceCloseResult::WorkspaceObserved { accepted, .. } => {
                    &accepted.close_request_id
                }
                WorkspaceCloseResult::ProjectObserved(reply) => &reply.operation_id,
                _ => &evidence.operation.request_id,
            };
            // The closing Workspace stays installed and drawn as current until its landing, which
            // viewer cleanup can defer, or which waits for publication when no other Workspace is
            // shown; announce Closed with that landing, not before it.
            let landing_awaited = self
                .refresh_install_awaits_cleanup(evidence.operation.generation)
                || (self.workspace_close.follow.is_some() && self.current_workspace_leaves());
            let status = if closed && landing_awaited {
                WorkspaceCloseStatus::Closing
            } else if closed {
                WorkspaceCloseStatus::Closed
            } else if evidence.result.is_pending() {
                // Admitted and not settled: its own submission flight still observes it, or no
                // read follows it any more and its outcome is unknown here.
                if self.workspace_close.pending.as_ref() == Some(&evidence.operation) {
                    WorkspaceCloseStatus::Closing
                } else {
                    WorkspaceCloseStatus::Unknown
                }
            } else {
                evidence.result.status()
            };
            return Some(WorkspaceCloseStatusView {
                operation: &evidence.operation,
                operation_id,
                status,
            });
        }
        let pending = self
            .workspace_close
            .pending
            .as_ref()
            .filter(|_| self.workspace_close.show_pending)?;
        Some(WorkspaceCloseStatusView {
            operation: pending,
            operation_id: &pending.request_id,
            status: WorkspaceCloseStatus::Closing,
        })
    }

    /// Workspaces an accepted close has taken out of navigation before publication removes them
    /// from the canonical snapshot. A Project close names its canonical members. The retained
    /// receipt decides, while settlement reads follow it or once it reads Closed: a refusal,
    /// failure, unknown outcome or ended follow yields no leaving set, so the rows return without
    /// a restore step. The cache keeps every canonical Workspace.
    pub(crate) fn leaving_workspace_ids(&self) -> Vec<WorkbenchWorkspaceId> {
        let Some(evidence) = self.workspace_close.evidence.as_ref() else {
            return Vec::new();
        };
        if !self.reads_closed(&evidence.result) {
            return Vec::new();
        }
        match &evidence.operation.target {
            WorkspaceCloseTarget::Workspace { workspace_id, .. } => vec![workspace_id.clone()],
            WorkspaceCloseTarget::Project { project_id, .. } => self
                .cache
                .canonical_snapshot()
                .navigation
                .workspaces
                .iter()
                .filter(|workspace| workspace.project_id.as_ref() == Some(project_id))
                .map(|workspace| workspace.workspace_id.clone())
                .collect(),
        }
    }

    /// The Workspace an accepted close of the current Workspace lands on: the neighbour the
    /// Context strip already marks, so the selection moves once and to the same place. Any other
    /// removal keeps landing on the first surviving Workspace.
    pub(super) fn accepted_close_neighbour(
        &self,
        snapshot: &Snapshot,
    ) -> Option<ClientContextLocation> {
        let leaving = self.leaving_workspace_ids();
        if !current_leaves(self.cache.location(), &leaving) {
            return None;
        }
        let neighbour =
            neighbour_after_leaving(self.cache.navigation(), &leaving, self.cache.location())?;
        let ClientContextLocation::Workspace {
            workspace_id,
            terminal_context_id,
        } = &neighbour
        else {
            return None;
        };
        snapshot
            .navigation
            .workspaces
            .iter()
            .any(|workspace| {
                &workspace.workspace_id == workspace_id
                    && &workspace.terminal_context_id == terminal_context_id
            })
            .then_some(neighbour)
    }

    /// Identifies the visible Closed notice so its one-shot lifetime survives repaint and landing.
    pub(crate) fn closed_workspace_close_notice(&self) -> Option<u64> {
        self.workspace_close_status()
            .filter(|status| status.status() == WorkspaceCloseStatus::Closed)
            .map(|status| status.operation.generation)
    }
    /// Ends only the Closed notice; the receipt and its fences stay with the operation. A notice
    /// retired while the close was still followed stays retired once it settles Closed.
    pub(crate) fn retire_closed_workspace_close_notice(&mut self) {
        if self
            .workspace_close
            .evidence
            .as_ref()
            .is_some_and(|evidence| self.reads_closed(&evidence.result))
        {
            self.workspace_close.closed_notice_retired = true;
        }
    }

    pub(super) fn record_workspace_close_landing(
        &mut self,
        permit: Option<&ContextRemovalPermit>,
        generation: Option<u64>,
        candidate: &ClientContextTabs,
    ) {
        if candidate.location() == self.cache.location() {
            return;
        }
        // Every later cross-context install, including manual navigation, retires the
        // prior display witness and its Closed notice. Returning to that location cannot
        // revive either; only the validated automatic landing off a Workspace this close takes
        // out of navigation keeps the notice, whichever refresh flight installs it.
        let leaving = self.leaving_workspace_ids();
        self.workspace_close.automatic_landing = permit.and_then(|permit| {
            let evidence = self.workspace_close.evidence.as_ref().filter(|_| {
                generation == Some(permit.generation())
                    && current_leaves(permit.removed(), &leaving)
                    && permit.authorizes(&self.cache, candidate)
            })?;
            Some(ClosedLanding {
                request_id: evidence.operation.request_id.clone(),
                generation: evidence.operation.generation,
                location: candidate.location().clone(),
            })
        });
        if self.workspace_close.automatic_landing.is_none() {
            self.retire_closed_workspace_close_notice();
        }
    }
}

/// Whether the current location is a Workspace that the leaving set takes out of navigation.
pub(super) fn current_leaves(
    current: &ClientContextLocation,
    leaving: &[WorkbenchWorkspaceId],
) -> bool {
    matches!(
        current,
        ClientContextLocation::Workspace { workspace_id, .. } if leaving.contains(workspace_id)
    )
}

/// The neighbour an accepted Workspace close selects, in navigation order: the next shown
/// Workspace after the closing one, else the previous shown one. `None` means no other
/// Workspace is shown, so the Workbench becomes empty.
pub(super) fn neighbour_after_leaving(
    navigation: &[ClientContext],
    leaving: &[WorkbenchWorkspaceId],
    closing: &ClientContextLocation,
) -> Option<ClientContextLocation> {
    let shown = |entry: &&ClientContext| match &entry.location {
        ClientContextLocation::Workspace { workspace_id, .. } => !leaving.contains(workspace_id),
        ClientContextLocation::NoWorkspace => true,
    };
    let index = navigation
        .iter()
        .position(|entry| &entry.location == closing)?;
    navigation[index + 1..]
        .iter()
        .find(|entry| shown(entry))
        .or_else(|| navigation[..index].iter().rev().find(|entry| shown(entry)))
        .map(|entry| entry.location.clone())
}

/// Every consumer retains the same monotone evidence, including admission before the first read.
pub(crate) fn retains_close_result(
    before: &WorkspaceCloseResult,
    after: &WorkspaceCloseResult,
) -> bool {
    match (before, after) {
        (
            WorkspaceCloseResult::ConfirmationRequired,
            WorkspaceCloseResult::ConfirmationRequired,
        ) => true,
        (
            WorkspaceCloseResult::WorkspaceAccepted(a),
            WorkspaceCloseResult::WorkspaceAccepted(b),
        ) => a == b,
        (
            WorkspaceCloseResult::WorkspaceAccepted(a),
            WorkspaceCloseResult::WorkspaceObserved { accepted, .. },
        ) => a == accepted,
        (
            WorkspaceCloseResult::WorkspaceObserved {
                accepted: a,
                outcome: before,
            },
            WorkspaceCloseResult::WorkspaceObserved {
                accepted: b,
                outcome: after,
            },
        ) => a == b && retains_workspace_observation(before, after),
        (
            WorkspaceCloseResult::ProjectObserved(before),
            WorkspaceCloseResult::ProjectObserved(after),
        ) => retains_group_observation(before, after),
        (
            WorkspaceCloseResult::NotSubmitted
            | WorkspaceCloseResult::Refused
            | WorkspaceCloseResult::Stale
            | WorkspaceCloseResult::Unknown,
            _,
        ) => true,
        _ => false,
    }
}
fn retains_workspace_observation(
    before: &WorkbenchWorkspaceCloseReadOutcome,
    after: &WorkbenchWorkspaceCloseReadOutcome,
) -> bool {
    let (
        WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: br,
            workspace_id: bw,
            terminal_context_id: bc,
            attempt_generation: ba,
            status: bs,
            members: bm,
            ..
        },
        WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: ar,
            workspace_id: aw,
            terminal_context_id: ac,
            attempt_generation: aa,
            status: ast,
            members: am,
            ..
        },
    ) = (before, after)
    else {
        return false;
    };
    br == ar && bw == aw && bc == ac && aa >= ba
        && (!matches!(bs, WorkbenchTabCloseStatus::Closed { .. }) || before == after)
        && (!matches!(bs, WorkbenchTabCloseStatus::OutcomeUnknown { .. }) || (ba == aa && bs == ast))
        && retains_disposal(bs, ast)
        && bm.len() == am.len()
        && bm.iter().zip(am).all(|(old, new)| {
            old.tab_id == new.tab_id && old.panel_id == new.panel_id && old.presentation_id == new.presentation_id
                && (!matches!(old.status, orchbee_workbench_protocol::WorkbenchWorkspaceCloseMemberStatus::OutcomeUnknown { .. })
                    || (ba == aa && old.status == new.status))
                && (!matches!(old.status, orchbee_workbench_protocol::WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication { .. })
                    || matches!(new.status, orchbee_workbench_protocol::WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication { .. }))
        })
}
fn retains_disposal(before: &WorkbenchTabCloseStatus, after: &WorkbenchTabCloseStatus) -> bool {
    !matches!(
        before,
        WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }
            | WorkbenchTabCloseStatus::Closed { .. }
    ) || matches!(
        after,
        WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }
            | WorkbenchTabCloseStatus::Closed { .. }
    )
}
fn retains_group_observation(
    before: &WorkbenchWorkspaceGroupCloseResult,
    after: &WorkbenchWorkspaceGroupCloseResult,
) -> bool {
    let (
        WorkspaceGroupCloseOutcome::Available {
            project_id: bp,
            attempt: ba,
            status: bs,
            members: bm,
        },
        WorkspaceGroupCloseOutcome::Available {
            project_id: ap,
            attempt: aa,
            status: ast,
            members: am,
        },
    ) = (&before.outcome, &after.outcome)
    else {
        return false;
    };
    before.operation_id == after.operation_id
        && bp == ap
        && aa >= ba
        && bm.len() == am.len()
        && (!matches!(bs, WorkspaceGroupCloseStatus::Closed {}) || before.outcome == after.outcome)
        && (aa > ba || !matches!(bs, WorkspaceGroupCloseStatus::Failed { .. }) || bs == ast)
        && (!matches!(
            bs,
            WorkspaceGroupCloseStatus::Failed {
                resumable: false,
                ..
            }
        ) || aa == ba)
        && bm.iter().zip(am).all(|(old, new)| {
            old.workspace_id == new.workspace_id
                && old.label == new.label
                && old.close.as_ref().is_none_or(|child| {
                    new.close.as_ref().is_some_and(|next| {
                        child.request_id == next.request_id
                            && (!child.is_closed() || child == next)
                            && child.observation.as_ref().is_none_or(|a| {
                                next.observation.as_ref().is_some_and(|b| {
                                    b.attempt_generation >= a.attempt_generation
                                        && (!matches!(
                                            a.status,
                                            WorkbenchTabCloseStatus::OutcomeUnknown { .. }
                                        ) || a == b)
                                        && retains_disposal(&a.status, &b.status)
                                })
                            })
                    })
                })
        })
}
impl<'a> super::WorkbenchClientView<'a> {
    pub(crate) fn workspace_close_confirmation(
        &self,
    ) -> Option<WorkspaceCloseConfirmationView<'a>> {
        self.state.workspace_close_confirmation()
    }
    pub(crate) fn workspace_close_status(&self) -> Option<WorkspaceCloseStatusView<'a>> {
        self.state.workspace_close_status()
    }
}
