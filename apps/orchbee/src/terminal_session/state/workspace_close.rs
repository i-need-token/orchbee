//! Client confirmation and one retained group Close; canonical removal stays with membership.
use super::{
    NativeContextTabs, NativeSessionState, OperationDeadlines, PaneKey, PendingRefreshFlight,
    SessionNotice, SessionPhase,
};
use crate::terminal_session::workbench_projection::NativeContextLocation;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchRequestId, WorkbenchWorkspaceGroupCloseRequest,
    WorkbenchWorkspaceGroupCloseResult, WorkspaceGroupCloseAction, WorkspaceGroupCloseOutcome,
    WorkspaceGroupCloseStatus,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCloseToken(u64);
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct WorkspaceCloseOperation {
    pub(in crate::terminal_session) request: WorkbenchWorkspaceGroupCloseRequest,
    pub(in crate::terminal_session) generation: u64,
    pub(in crate::terminal_session) deadlines: OperationDeadlines,
    pub(in crate::terminal_session) baseline: NativeContextTabs,
    desired: Option<PaneKey>,
    label: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum WorkspaceCloseResult {
    NotSubmitted,
    Refused,
    Unknown,
    Observed(Box<WorkbenchWorkspaceGroupCloseResult>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct WorkspaceCloseEvidence {
    pub(in crate::terminal_session) operation: WorkspaceCloseOperation,
    pub(in crate::terminal_session) result: WorkspaceCloseResult,
}
impl WorkspaceCloseResult {
    pub(in crate::terminal_session) fn is_closed(&self) -> bool {
        matches!(self, Self::Observed(reply) if matches!(reply.outcome, WorkspaceGroupCloseOutcome::Available { status: WorkspaceGroupCloseStatus::Closed {}, .. }))
    }
    pub(in crate::terminal_session) fn is_pending(&self) -> bool {
        matches!(self, Self::Observed(reply) if matches!(reply.outcome, WorkspaceGroupCloseOutcome::Available { status: WorkspaceGroupCloseStatus::Pending {}, .. }))
    }
}
impl WorkspaceCloseOperation {
    pub(in crate::terminal_session) fn admits(&self, result: &WorkspaceCloseResult) -> bool {
        let WorkspaceCloseResult::Observed(reply) = result else {
            return true;
        };
        let WorkspaceGroupCloseAction::Start {
            operation_id,
            project_id,
            workspace_id,
        } = &self.request.action
        else {
            return false;
        };
        reply.operation_id == *operation_id
            && reply.validate().is_ok()
            && matches!(&reply.outcome, WorkspaceGroupCloseOutcome::Available { project_id: project, workspace_id: workspace, attempt: 1, .. } if project == project_id && workspace == workspace_id)
    }
}
struct Confirmation {
    token: WorkspaceCloseToken,
    baseline: NativeContextTabs,
    desired: Option<PaneKey>,
    label: String,
    shown: bool,
}
#[derive(Default)]
pub(super) struct WorkspaceCloseState {
    next_token: u64,
    confirmation: Option<Confirmation>,
    pub(super) pending: Option<WorkspaceCloseOperation>,
    evidence: Option<WorkspaceCloseEvidence>,
    show_pending: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceCloseConfirmationView<'a> {
    inner: &'a Confirmation,
}
impl<'a> WorkspaceCloseConfirmationView<'a> {
    pub(crate) const fn token(self) -> WorkspaceCloseToken {
        self.inner.token
    }
    pub(crate) fn label(self) -> &'a str {
        &self.inner.label
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceCloseStatus {
    Closing,
    Closed,
    Failed,
    Unknown,
    Refused,
}
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceCloseStatusView<'a> {
    label: &'a str,
    status: WorkspaceCloseStatus,
    operation_id: Option<&'a WorkbenchRequestId>,
}
impl<'a> WorkspaceCloseStatusView<'a> {
    pub(crate) const fn label(self) -> &'a str {
        self.label
    }
    pub(crate) const fn status(self) -> WorkspaceCloseStatus {
        self.status
    }
    pub(crate) const fn operation_id(self) -> Option<&'a WorkbenchRequestId> {
        self.operation_id
    }
}
impl NativeSessionState {
    pub(super) fn workspace_close_invariant_holds(&self) -> bool {
        self.workspace_close
            .pending
            .as_ref()
            .is_none_or(|operation| {
                operation.baseline == self.cache
                    && (self.pending_refresh_generation() == Some(operation.generation)
                        || matches!(self.phase, SessionPhase::Retired(_)))
            })
            && self
                .workspace_close
                .evidence
                .as_ref()
                .is_none_or(|evidence| evidence.operation.admits(&evidence.result))
    }
    pub(in crate::terminal_session) fn workspace_close_blocks_structure(&self) -> bool {
        self.workspace_close.confirmation.is_some() || self.workspace_close.pending.is_some()
    }
    fn workspace_close_quiet(&self) -> bool {
        self.pending_refresh.is_none()
            && self.pending_mutation.is_none()
            && self.pending_membership.is_none()
            && !self.setup_blocks_structure()
            && !self.rename_draft_blocks_structure()
            && self.move_recovery.is_none()
            && self.rename_recovery.is_none()
            && self.rename_uncertain.is_none()
            && self.panel_recovery.is_none()
            && self.close.permits_retired_membership()
            && self.panel_close.confirmation().is_none()
            && matches!(
                self.phase,
                SessionPhase::Empty
                    | SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
    }
    pub(in crate::terminal_session) fn open_workspace_close(
        &mut self,
    ) -> Option<WorkspaceCloseToken> {
        if !matches!(
            self.cache.location(),
            NativeContextLocation::Workspace { .. }
        ) {
            self.set_notice(SessionNotice::WorkspaceCloseUnavailable);
            return None;
        }
        if !self.workspace_close_quiet()
            || self.workspace_close_blocks_structure()
            || self
                .workspace_close
                .evidence
                .as_ref()
                .is_some_and(|evidence| {
                    !matches!(
                        evidence.result,
                        WorkspaceCloseResult::NotSubmitted | WorkspaceCloseResult::Refused
                    ) && !evidence.result.is_closed()
                })
        {
            self.set_notice(SessionNotice::LocalBusy);
            return None;
        }
        let label = self
            .cache
            .navigation()
            .iter()
            .find(|entry| &entry.location == self.cache.location())?
            .workspace_label
            .as_ref()?
            .as_str()
            .to_owned();
        let next = self.workspace_close.next_token.checked_add(1)?;
        self.workspace_close.next_token = next;
        let token = WorkspaceCloseToken(next);
        self.workspace_close.confirmation = Some(Confirmation {
            token,
            baseline: self.cache.clone(),
            desired: self.desired.clone(),
            label,
            shown: false,
        });
        self.notice = None;
        Some(token)
    }
    pub(in crate::terminal_session) fn workspace_close_confirmation(
        &self,
    ) -> Option<WorkspaceCloseConfirmationView<'_>> {
        self.workspace_close
            .confirmation
            .as_ref()
            .map(|inner| WorkspaceCloseConfirmationView { inner })
    }
    pub(in crate::terminal_session) fn cancel_workspace_close(&mut self) {
        self.workspace_close.confirmation = None;
    }
    pub(in crate::terminal_session) fn acknowledge_workspace_close_draw(
        &mut self,
        token: Option<WorkspaceCloseToken>,
    ) {
        if let Some(prompt) = &mut self.workspace_close.confirmation {
            prompt.shown = token == Some(prompt.token);
        }
    }
    pub(in crate::terminal_session) fn settle_stale_workspace_close(&mut self) -> bool {
        if self
            .workspace_close
            .confirmation
            .as_ref()
            .is_some_and(|prompt| {
                prompt.baseline != self.cache
                    || prompt.desired != self.desired
                    || !self.workspace_close_quiet()
            })
        {
            self.cancel_workspace_close();
            self.set_notice(SessionNotice::ConfirmationCancelled);
            return true;
        }
        false
    }
    pub(in crate::terminal_session) fn plan_workspace_close(
        &self,
        token: WorkspaceCloseToken,
        request_id: WorkbenchRequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<WorkspaceCloseOperation> {
        let prompt = self.workspace_close.confirmation.as_ref()?;
        if !prompt.shown
            || prompt.token != token
            || prompt.baseline != self.cache
            || prompt.desired != self.desired
            || !self.workspace_close_quiet()
            || generation == 0
        {
            return None;
        }
        let NativeContextLocation::Workspace {
            project_id,
            workspace_id,
            ..
        } = self.cache.location()
        else {
            return None;
        };
        Some(WorkspaceCloseOperation {
            request: WorkbenchWorkspaceGroupCloseRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                site_id: self.cache.site_id().clone(),
                action: WorkspaceGroupCloseAction::Start {
                    operation_id: request_id,
                    project_id: project_id.clone(),
                    workspace_id: workspace_id.clone(),
                },
            },
            generation,
            deadlines,
            baseline: prompt.baseline.clone(),
            desired: prompt.desired.clone(),
            label: prompt.label.clone(),
        })
    }
    pub(in crate::terminal_session) fn begin_workspace_close(
        &mut self,
        operation: WorkspaceCloseOperation,
    ) -> bool {
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
                operation.request.request_id.clone(),
                operation.generation,
                operation.deadlines,
            )
            .as_ref()
            != Some(&operation)
        {
            return false;
        }
        self.cancel_workspace_close();
        self.pending_refresh = Some(PendingRefreshFlight::recovery(operation.generation));
        self.workspace_close.pending = Some(operation);
        self.workspace_close.evidence = None;
        self.workspace_close.show_pending = false;
        self.adopt_publication_debt();
        true
    }
    pub(in crate::terminal_session) fn tick_workspace_close_pending(
        &mut self,
        sampled: Instant,
    ) -> bool {
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
    pub(in crate::terminal_session) fn retain_workspace_close_evidence(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
    ) -> bool {
        if !evidence.operation.admits(&evidence.result) {
            return false;
        }
        // A completed older lease may return after the next operation was admitted.
        // It cannot overwrite that operation's recovery identity.
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
                return false;
            }
            if matches!(prior.result, WorkspaceCloseResult::Observed(_))
                && !matches!(evidence.result, WorkspaceCloseResult::Observed(_))
            {
                return true;
            }
            if let (WorkspaceCloseResult::Observed(before), WorkspaceCloseResult::Observed(after)) =
                (&prior.result, &evidence.result)
                && !retains_group_observation(before, after)
            {
                return false;
            }
        } else if self.workspace_close.pending.as_ref() != Some(&evidence.operation) {
            return false;
        }
        self.workspace_close.evidence = Some(evidence.clone());
        true
    }
    pub(in crate::terminal_session) fn finish_workspace_close(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
    ) -> bool {
        if !self.retain_workspace_close_evidence(evidence) {
            return false;
        }
        if self.workspace_close.pending.as_ref() == Some(&evidence.operation) {
            self.workspace_close.pending = None;
        }
        true
    }
    pub(in crate::terminal_session) fn workspace_close_deadlines(
        &self,
    ) -> Option<OperationDeadlines> {
        self.workspace_close
            .evidence
            .as_ref()
            .filter(|evidence| {
                self.pending_refresh_generation() == Some(evidence.operation.generation)
            })
            .map(|evidence| evidence.operation.deadlines)
    }
    pub(in crate::terminal_session) fn workspace_close_status(
        &self,
    ) -> Option<WorkspaceCloseStatusView<'_>> {
        if let Some(evidence) = &self.workspace_close.evidence {
            if evidence.result.is_closed()
                && self.cache.location() != evidence.operation.baseline.location()
                && !matches!(self.cache.location(), NativeContextLocation::Standalone(_))
            {
                return None;
            }
            let status = match &evidence.result {
                WorkspaceCloseResult::NotSubmitted | WorkspaceCloseResult::Refused => {
                    WorkspaceCloseStatus::Refused
                }
                WorkspaceCloseResult::Unknown => WorkspaceCloseStatus::Unknown,
                WorkspaceCloseResult::Observed(_) if evidence.result.is_closed() => {
                    WorkspaceCloseStatus::Closed
                }
                WorkspaceCloseResult::Observed(_) if evidence.result.is_pending() => {
                    WorkspaceCloseStatus::Closing
                }
                WorkspaceCloseResult::Observed(_) => WorkspaceCloseStatus::Failed,
            };
            return Some(WorkspaceCloseStatusView {
                label: &evidence.operation.label,
                status,
                operation_id: (status != WorkspaceCloseStatus::Refused)
                    .then(|| evidence.operation.request.action.operation_id()),
            });
        }
        let pending = self
            .workspace_close
            .pending
            .as_ref()
            .filter(|_| self.workspace_close.show_pending)?;
        Some(WorkspaceCloseStatusView {
            label: &pending.label,
            status: WorkspaceCloseStatus::Closing,
            operation_id: Some(pending.request.action.operation_id()),
        })
    }
}
pub(in crate::terminal_session) fn retains_group_observation(
    before: &WorkbenchWorkspaceGroupCloseResult,
    after: &WorkbenchWorkspaceGroupCloseResult,
) -> bool {
    let (
        WorkspaceGroupCloseOutcome::Available {
            project_id: bp,
            workspace_id: bw,
            attempt: ba,
            members: bm,
            ..
        },
        WorkspaceGroupCloseOutcome::Available {
            project_id: ap,
            workspace_id: aw,
            attempt: aa,
            members: am,
            ..
        },
    ) = (&before.outcome, &after.outcome)
    else {
        return false;
    };
    before.operation_id == after.operation_id
        && bp == ap
        && bw == aw
        && ba == aa
        && bm.len() == am.len()
        && (!matches!(
            before.outcome,
            WorkspaceGroupCloseOutcome::Available {
                status: WorkspaceGroupCloseStatus::Closed {},
                ..
            }
        ) || before.outcome == after.outcome)
        && bm.iter().zip(am).all(|(old, new)| {
            old.project_id == new.project_id
                && old.workspace_id == new.workspace_id
                && old.label == new.label
                && old.close.as_ref().is_none_or(|child| {
                    new.close.as_ref().is_some_and(|next| {
                        child.request_id == next.request_id
                            && (!child.is_closed() || child == next)
                            && child.observation.as_ref().is_none_or(|a| {
                                next.observation
                                    .as_ref()
                                    .is_some_and(|b| b.attempt_generation >= a.attempt_generation)
                            })
                    })
                })
        })
}
impl<'a> super::NativeSessionView<'a> {
    pub(in crate::terminal_session) fn workspace_close_confirmation(
        &self,
    ) -> Option<WorkspaceCloseConfirmationView<'a>> {
        self.state.workspace_close_confirmation()
    }
    pub(in crate::terminal_session) fn workspace_close_status(
        &self,
    ) -> Option<WorkspaceCloseStatusView<'a>> {
        self.state.workspace_close_status()
    }
}
