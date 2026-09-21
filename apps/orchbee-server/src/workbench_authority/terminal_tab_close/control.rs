use std::time::Instant;

use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseRead,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseResume,
    WorkbenchTabCloseResumeOutcome, WorkbenchTabCloseResumeRejectionReason,
    WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus, WorkbenchTabCloseUnavailableReason,
    WorkbenchWorkspaceCloseCounts, WorkbenchWorkspaceCloseMember,
    WorkbenchWorkspaceCloseMemberStatus, WorkbenchWorkspaceCloseRead,
    WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceCloseReadResult,
    WorkbenchWorkspaceCloseResume, WorkbenchWorkspaceCloseResumeOutcome,
    WorkbenchWorkspaceCloseResumeResult,
};

use super::super::{RetainedRequest, WorkbenchAuthority, WorkbenchGrant};
use super::continuation::PreparedCloseContinuationResume;
use super::{AcceptedResumeReceipt, CloseRecord, WorkbenchTabCloseTarget};

pub enum WorkbenchTabCloseResumeBegin<'a> {
    Ready(WorkbenchTabCloseResumeResult),
    Admitted(Box<WorkbenchTabCloseResumeAdmission<'a>>),
}

pub struct WorkbenchTabCloseResumeAdmission<'a> {
    record: &'a mut CloseRecord,
    request: WorkbenchTabCloseResume,
}

pub enum WorkbenchWorkspaceCloseResumeBegin<'a> {
    Ready(WorkbenchWorkspaceCloseResumeResult),
    Admitted(Box<WorkbenchWorkspaceCloseResumeAdmission<'a>>),
}

pub struct WorkbenchWorkspaceCloseResumeAdmission<'a> {
    record: &'a mut CloseRecord,
    request: WorkbenchWorkspaceCloseResume,
}

pub enum WorkbenchWorkspaceCloseResumePreparation<'a> {
    Ready(WorkbenchWorkspaceCloseResumeResult),
    Prepared(Box<PreparedWorkbenchWorkspaceCloseResume<'a>>),
}

pub struct PreparedWorkbenchWorkspaceCloseResume<'a> {
    record: &'a mut CloseRecord,
    continuations: Vec<(usize, PreparedCloseContinuationResume)>,
    generation: WorkbenchTabCloseAttemptGeneration,
    receipt: AcceptedResumeReceipt,
    response: WorkbenchWorkspaceCloseResumeResult,
}

pub enum WorkbenchTabCloseResumePreparation<'a> {
    Ready(WorkbenchTabCloseResumeResult),
    Prepared(Box<PreparedWorkbenchTabCloseResume<'a>>),
}

pub struct PreparedWorkbenchTabCloseResume<'a> {
    record: &'a mut CloseRecord,
    continuations: Vec<(usize, PreparedCloseContinuationResume)>,
    generation: WorkbenchTabCloseAttemptGeneration,
    receipt: AcceptedResumeReceipt,
    response: WorkbenchTabCloseResumeResult,
}

impl<'a> WorkbenchTabCloseResumeAdmission<'a> {
    #[must_use]
    pub fn target(&self) -> Option<&WorkbenchTabCloseTarget> {
        self.record.targets().next()
    }

    pub fn targets(&self) -> impl Iterator<Item = &WorkbenchTabCloseTarget> {
        self.record.targets()
    }

    #[must_use]
    pub fn runtime_targets(&self) -> Vec<WorkbenchTabCloseTarget> {
        self.record
            .targets()
            .enumerate()
            .filter(|(index, _target)| self.record.target_requires_runtime(*index))
            .map(|(_index, target)| target.clone())
            .collect()
    }

    #[must_use]
    pub fn needs_local_cleanup(&self) -> bool {
        self.record.has_target_cleanup_pending()
    }

    #[must_use]
    pub fn status(&self) -> orchbee_workbench_protocol::WorkbenchTabCloseStatus {
        self.record.status()
    }

    #[must_use]
    pub fn runtime_unavailable(self) -> WorkbenchTabCloseResumeResult {
        resume_result(&self.request, busy())
    }

    /// The Site first validates the exact dependencies required by the retained phase.
    #[must_use]
    pub fn finish(self, now: Instant) -> WorkbenchTabCloseResumePreparation<'a> {
        let Some(generation) = self.record.generation.checked_next() else {
            self.record.generation_exhausted();
            return WorkbenchTabCloseResumePreparation::Ready(resume_result(
                &self.request,
                rejected(WorkbenchTabCloseResumeRejectionReason::GenerationExhausted),
            ));
        };
        let Ok(continuations) = self.record.prepare_resumes(now) else {
            return WorkbenchTabCloseResumePreparation::Ready(resume_result(&self.request, busy()));
        };
        let response = resume_result(
            &self.request,
            WorkbenchTabCloseResumeOutcome::Resumed {
                attempt_generation: generation,
            },
        );
        // Measure the real candidate receipt before any state changes: it
        // must fit its reserved payload slot, and the retained record —
        // existing receipt included — plus the candidate must stay inside
        // the original reservation. Over budget is busy, never malformed.
        let Some(candidate_payload) =
            super::reservation::tab_receipt_owned_bytes(&self.request, &response)
        else {
            return WorkbenchTabCloseResumePreparation::Ready(resume_result(&self.request, busy()));
        };
        if candidate_payload > self.record.receipt_slot_bytes {
            return WorkbenchTabCloseResumePreparation::Ready(resume_result(&self.request, busy()));
        }
        let within_reservation = super::reservation::record_controlled_heap(self.record)
            .and_then(|retained| retained.checked_add(candidate_payload))
            .is_some_and(|peak| peak <= self.record.reserved_bytes);
        if !within_reservation {
            return WorkbenchTabCloseResumePreparation::Ready(resume_result(&self.request, busy()));
        }
        let receipt = AcceptedResumeReceipt::Tab {
            request: self.request,
            result: response.clone(),
        };
        WorkbenchTabCloseResumePreparation::Prepared(Box::new(PreparedWorkbenchTabCloseResume {
            record: self.record,
            continuations,
            generation,
            receipt,
            response,
        }))
    }
}

impl PreparedWorkbenchTabCloseResume<'_> {
    #[must_use]
    pub fn result(&self) -> &WorkbenchTabCloseResumeResult {
        &self.response
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchTabCloseResumeResult {
        self.record.commit_resumes(self.continuations);
        self.record.generation = self.generation;
        self.record.resume_receipt = Some(self.receipt);
        self.response
    }
}

impl<'a> WorkbenchWorkspaceCloseResumeAdmission<'a> {
    #[must_use]
    pub fn runtime_targets(&self) -> Vec<WorkbenchTabCloseTarget> {
        self.record
            .targets()
            .enumerate()
            .filter(|(index, _target)| self.record.target_requires_runtime(*index))
            .map(|(_index, target)| target.clone())
            .collect()
    }

    #[must_use]
    pub fn needs_local_cleanup(&self) -> bool {
        self.record.has_target_cleanup_pending()
    }

    #[must_use]
    pub fn runtime_unavailable(self) -> WorkbenchWorkspaceCloseResumeResult {
        workspace_resume_result(&self.request, workspace_busy())
    }

    /// The Site first validates the exact dependencies required by the retained phase.
    #[must_use]
    pub fn finish(self, now: Instant) -> WorkbenchWorkspaceCloseResumePreparation<'a> {
        let Some(generation) = self.record.generation.checked_next() else {
            self.record.generation_exhausted();
            return WorkbenchWorkspaceCloseResumePreparation::Ready(workspace_resume_result(
                &self.request,
                workspace_rejected(WorkbenchTabCloseResumeRejectionReason::GenerationExhausted),
            ));
        };
        let Ok(continuations) = self.record.prepare_resumes(now) else {
            return WorkbenchWorkspaceCloseResumePreparation::Ready(workspace_resume_result(
                &self.request,
                workspace_busy(),
            ));
        };
        let response = workspace_resume_result(
            &self.request,
            WorkbenchWorkspaceCloseResumeOutcome::Resumed {
                attempt_generation: generation,
            },
        );
        // Measure the real candidate receipt before any state changes: it
        // must fit its reserved payload slot, and the retained record —
        // existing receipt included — plus the candidate must stay inside
        // the original reservation. Over budget is busy, never malformed.
        let Some(candidate_payload) =
            super::reservation::workspace_receipt_owned_bytes(&self.request, &response)
        else {
            return WorkbenchWorkspaceCloseResumePreparation::Ready(workspace_resume_result(
                &self.request,
                workspace_busy(),
            ));
        };
        if candidate_payload > self.record.receipt_slot_bytes {
            return WorkbenchWorkspaceCloseResumePreparation::Ready(workspace_resume_result(
                &self.request,
                workspace_busy(),
            ));
        }
        let within_reservation = super::reservation::record_controlled_heap(self.record)
            .and_then(|retained| retained.checked_add(candidate_payload))
            .is_some_and(|peak| peak <= self.record.reserved_bytes);
        if !within_reservation {
            return WorkbenchWorkspaceCloseResumePreparation::Ready(workspace_resume_result(
                &self.request,
                workspace_busy(),
            ));
        }
        let receipt = AcceptedResumeReceipt::Workspace {
            request: self.request,
            result: response.clone(),
        };
        WorkbenchWorkspaceCloseResumePreparation::Prepared(Box::new(
            PreparedWorkbenchWorkspaceCloseResume {
                record: self.record,
                continuations,
                generation,
                receipt,
                response,
            },
        ))
    }
}

impl PreparedWorkbenchWorkspaceCloseResume<'_> {
    #[must_use]
    pub fn result(&self) -> &WorkbenchWorkspaceCloseResumeResult {
        &self.response
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchWorkspaceCloseResumeResult {
        self.record.commit_resumes(self.continuations);
        self.record.generation = self.generation;
        self.record.resume_receipt = Some(self.receipt);
        self.response
    }
}

impl WorkbenchAuthority {
    #[must_use]
    pub fn read_terminal_tab_close(
        &self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchTabCloseRead,
    ) -> WorkbenchTabCloseReadResult {
        let authorized = grant.is_some_and(|grant| {
            grant.site_id == *self.site_id() && request.site_id == *self.site_id()
        });
        let outcome = if !authorized {
            WorkbenchTabCloseReadOutcome::Denied {}
        } else {
            self.ledger
                .values()
                .find_map(|entry| match entry {
                    RetainedRequest::TabClose(record)
                        if record
                            .matches_context_tab(&request.terminal_context_id, &request.tab_id) =>
                    {
                        Some(record)
                    }
                    _ => None,
                })
                .map_or(WorkbenchTabCloseReadOutcome::NotClosing {}, |record| {
                    let Some(target) = record.targets().next() else {
                        return WorkbenchTabCloseReadOutcome::NotClosing {};
                    };
                    WorkbenchTabCloseReadOutcome::Available {
                        close_request_id: record.command.request_id.clone(),
                        terminal_context_id: target.terminal_context_id.clone(),
                        tab_id: target.member.tab_id().clone(),
                        attempt_generation: record.generation,
                        status: record.status(),
                    }
                })
        };
        WorkbenchTabCloseReadResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome,
        }
    }

    pub fn begin_terminal_tab_close_resume(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchTabCloseResume,
    ) -> WorkbenchTabCloseResumeBegin<'_> {
        let authorized =
            grant.is_some_and(|grant| grant.site_id == *self.site_id() && grant.close_terminal);
        if !authorized {
            return WorkbenchTabCloseResumeBegin::Ready(resume_result(
                &request,
                WorkbenchTabCloseResumeOutcome::Denied {},
            ));
        }
        let Some(record) = self.close_record_mut(&request.close_request_id) else {
            return WorkbenchTabCloseResumeBegin::Ready(resume_result(
                &request,
                rejected(WorkbenchTabCloseResumeRejectionReason::OperationUnavailable),
            ));
        };
        if request.site_id != record.command.site_id
            || !record.matches_context_tab(&request.terminal_context_id, &request.tab_id)
        {
            return WorkbenchTabCloseResumeBegin::Ready(resume_result(
                &request,
                rejected(WorkbenchTabCloseResumeRejectionReason::OperationMismatch),
            ));
        }
        if let Some(AcceptedResumeReceipt::Tab {
            request: receipt_request,
            result: receipt_result,
        }) = &record.resume_receipt
            && receipt_request.request_id == request.request_id
        {
            return WorkbenchTabCloseResumeBegin::Ready(if *receipt_request == request {
                receipt_result.clone()
            } else {
                resume_result(
                    &request,
                    rejected(WorkbenchTabCloseResumeRejectionReason::RequestIdentityConflict),
                )
            });
        }
        if request.expected_attempt_generation < record.generation {
            return WorkbenchTabCloseResumeBegin::Ready(resume_result(
                &request,
                WorkbenchTabCloseResumeOutcome::Stale {
                    attempt_generation: record.generation,
                },
            ));
        }
        if request.expected_attempt_generation > record.generation {
            return WorkbenchTabCloseResumeBegin::Ready(resume_result(
                &request,
                rejected(WorkbenchTabCloseResumeRejectionReason::GenerationMismatch),
            ));
        }
        if !record.is_resumable() || record.has_outstanding() {
            return WorkbenchTabCloseResumeBegin::Ready(resume_result(
                &request,
                rejected(WorkbenchTabCloseResumeRejectionReason::NotResumable),
            ));
        }
        WorkbenchTabCloseResumeBegin::Admitted(Box::new(WorkbenchTabCloseResumeAdmission {
            record,
            request,
        }))
    }

    /// `workspace-close.resume` admission: identical judgment order to the
    /// Tab-scoped resume — operation, scope, receipt replay, generation,
    /// resumability — keyed on the scope's `(project, workspace)` pair.
    pub fn begin_workspace_close_resume(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchWorkspaceCloseResume,
    ) -> WorkbenchWorkspaceCloseResumeBegin<'_> {
        let authorized =
            grant.is_some_and(|grant| grant.site_id == *self.site_id() && grant.close_terminal);
        if !authorized {
            return WorkbenchWorkspaceCloseResumeBegin::Ready(workspace_resume_result(
                &request,
                WorkbenchWorkspaceCloseResumeOutcome::Denied {},
            ));
        }
        let Some(record) = self.close_record_mut(&request.close_request_id) else {
            return WorkbenchWorkspaceCloseResumeBegin::Ready(workspace_resume_result(
                &request,
                workspace_rejected(WorkbenchTabCloseResumeRejectionReason::OperationUnavailable),
            ));
        };
        if request.site_id != record.command.site_id
            || !record.matches_context_scope(&request.project_id, &request.workspace_id)
        {
            return WorkbenchWorkspaceCloseResumeBegin::Ready(workspace_resume_result(
                &request,
                workspace_rejected(WorkbenchTabCloseResumeRejectionReason::OperationMismatch),
            ));
        }
        if let Some(AcceptedResumeReceipt::Workspace {
            request: receipt_request,
            result: receipt_result,
        }) = &record.resume_receipt
            && receipt_request.request_id == request.request_id
        {
            return WorkbenchWorkspaceCloseResumeBegin::Ready(if *receipt_request == request {
                receipt_result.clone()
            } else {
                workspace_resume_result(
                    &request,
                    workspace_rejected(
                        WorkbenchTabCloseResumeRejectionReason::RequestIdentityConflict,
                    ),
                )
            });
        }
        if request.expected_attempt_generation < record.generation {
            return WorkbenchWorkspaceCloseResumeBegin::Ready(workspace_resume_result(
                &request,
                WorkbenchWorkspaceCloseResumeOutcome::Stale {
                    attempt_generation: record.generation,
                },
            ));
        }
        if request.expected_attempt_generation > record.generation {
            return WorkbenchWorkspaceCloseResumeBegin::Ready(workspace_resume_result(
                &request,
                workspace_rejected(WorkbenchTabCloseResumeRejectionReason::GenerationMismatch),
            ));
        }
        if !record.is_resumable() || record.has_outstanding() {
            return WorkbenchWorkspaceCloseResumeBegin::Ready(workspace_resume_result(
                &request,
                workspace_rejected(WorkbenchTabCloseResumeRejectionReason::NotResumable),
            ));
        }
        WorkbenchWorkspaceCloseResumeBegin::Admitted(Box::new(
            WorkbenchWorkspaceCloseResumeAdmission { record, request },
        ))
    }

    /// Read the retained Workspace Close for the named `(project, workspace)`
    /// scope. The scope triple is the lookup identity — never the member set —
    /// so an empty Workspace reports exactly like a populated one.
    #[must_use]
    pub fn read_workspace_close(
        &self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchWorkspaceCloseRead,
    ) -> WorkbenchWorkspaceCloseReadResult {
        let authorized = grant.is_some_and(|grant| {
            grant.site_id == *self.site_id() && request.site_id == *self.site_id()
        });
        let outcome = if !authorized {
            WorkbenchWorkspaceCloseReadOutcome::Denied {}
        } else {
            self.ledger
                .values()
                .find_map(|entry| match entry {
                    RetainedRequest::TabClose(record)
                        if record
                            .matches_context_scope(&request.project_id, &request.workspace_id) =>
                    {
                        Some(record)
                    }
                    _ => None,
                })
                .map_or(
                    WorkbenchWorkspaceCloseReadOutcome::NotClosing {},
                    |record| {
                        let mut counts = WorkbenchWorkspaceCloseCounts::default();
                        let members: Vec<_> = (0..record.target_count())
                            .filter_map(|index| {
                                let target = record.target(index)?;
                                let status = member_status(record.continuation(index)?.status());
                                match &status {
                                WorkbenchWorkspaceCloseMemberStatus::Closing { .. } => {
                                    counts.closing += 1;
                                }
                                WorkbenchWorkspaceCloseMemberStatus::CleanupBlocked {
                                    ..
                                } => counts.cleanup_blocked += 1,
                                WorkbenchWorkspaceCloseMemberStatus::OutcomeUnknown {
                                    ..
                                } => counts.outcome_unknown += 1,
                                WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication {
                                    ..
                                } => {
                                    counts.disposed_awaiting_publication += 1;
                                }
                            }
                                Some(WorkbenchWorkspaceCloseMember {
                                    tab_id: target.member.tab_id().clone(),
                                    panel_id: target.member.panel_id().clone(),
                                    presentation_id: target.member.presentation_id().clone(),
                                    status,
                                })
                            })
                            .collect();
                        WorkbenchWorkspaceCloseReadOutcome::Available {
                            close_request_id: record.command.request_id.clone(),
                            project_id: request.project_id.clone(),
                            workspace_id: request.workspace_id.clone(),
                            terminal_context_id: record.terminal_context_id.clone(),
                            attempt_generation: record.generation,
                            status: record.status(),
                            counts,
                            members,
                        }
                    },
                )
        };
        WorkbenchWorkspaceCloseReadResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome,
        }
    }
}

pub(super) fn resume_result(
    request: &WorkbenchTabCloseResume,
    outcome: WorkbenchTabCloseResumeOutcome,
) -> WorkbenchTabCloseResumeResult {
    WorkbenchTabCloseResumeResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        terminal_context_id: request.terminal_context_id.clone(),
        tab_id: request.tab_id.clone(),
        close_request_id: request.close_request_id.clone(),
        outcome,
    }
}

pub(super) fn workspace_resume_result(
    request: &WorkbenchWorkspaceCloseResume,
    outcome: WorkbenchWorkspaceCloseResumeOutcome,
) -> WorkbenchWorkspaceCloseResumeResult {
    WorkbenchWorkspaceCloseResumeResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        project_id: request.project_id.clone(),
        workspace_id: request.workspace_id.clone(),
        close_request_id: request.close_request_id.clone(),
        outcome,
    }
}

/// Member truth excludes the close revision — that lives solely in the
/// scope-level status. A member continuation marked `Closed` still reports
/// its terminal disposition: it is disposed; the scope commit already owns
/// the publication outcome.
fn member_status(status: WorkbenchTabCloseStatus) -> WorkbenchWorkspaceCloseMemberStatus {
    match status {
        WorkbenchTabCloseStatus::Closing { phase } => {
            WorkbenchWorkspaceCloseMemberStatus::Closing { phase }
        }
        WorkbenchTabCloseStatus::CleanupBlocked { reason, resumable } => {
            WorkbenchWorkspaceCloseMemberStatus::CleanupBlocked { reason, resumable }
        }
        WorkbenchTabCloseStatus::OutcomeUnknown { phase, reason } => {
            WorkbenchWorkspaceCloseMemberStatus::OutcomeUnknown { phase, reason }
        }
        WorkbenchTabCloseStatus::DisposedAwaitingPublication { reason, resumable } => {
            WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication { reason, resumable }
        }
        WorkbenchTabCloseStatus::Closed { .. } => {
            WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication {
                reason: orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::Preparing,
                resumable: false,
            }
        }
    }
}

fn rejected(reason: WorkbenchTabCloseResumeRejectionReason) -> WorkbenchTabCloseResumeOutcome {
    WorkbenchTabCloseResumeOutcome::Rejected { reason }
}

fn workspace_rejected(
    reason: WorkbenchTabCloseResumeRejectionReason,
) -> WorkbenchWorkspaceCloseResumeOutcome {
    WorkbenchWorkspaceCloseResumeOutcome::Rejected { reason }
}

fn busy() -> WorkbenchTabCloseResumeOutcome {
    WorkbenchTabCloseResumeOutcome::Unavailable {
        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
    }
}

fn workspace_busy() -> WorkbenchWorkspaceCloseResumeOutcome {
    WorkbenchWorkspaceCloseResumeOutcome::Unavailable {
        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
    }
}
