use std::time::Instant;

use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseRead,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseResume,
    WorkbenchTabCloseResumeOutcome, WorkbenchTabCloseResumeRejectionReason,
    WorkbenchTabCloseResumeResult, WorkbenchTabCloseUnavailableReason,
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
    pub fn target(&self) -> &WorkbenchTabCloseTarget {
        &self.record.target
    }

    #[must_use]
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
        if !super::reservation::receipt_charge(&self.request, &response)
            .is_some_and(|charge| charge <= self.record.reserved_bytes)
        {
            return WorkbenchTabCloseResumePreparation::Ready(resume_result(&self.request, busy()));
        }
        let receipt = AcceptedResumeReceipt {
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
                    WorkbenchTabCloseReadOutcome::Available {
                        close_request_id: record.command.request_id.clone(),
                        terminal_context_id: record.target.terminal_context_id.clone(),
                        tab_id: record.target.member.tab_id().clone(),
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
        if let Some(receipt) = &record.resume_receipt
            && receipt.request.request_id == request.request_id
        {
            return WorkbenchTabCloseResumeBegin::Ready(if receipt.request == request {
                receipt.result.clone()
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

fn rejected(reason: WorkbenchTabCloseResumeRejectionReason) -> WorkbenchTabCloseResumeOutcome {
    WorkbenchTabCloseResumeOutcome::Rejected { reason }
}

fn busy() -> WorkbenchTabCloseResumeOutcome {
    WorkbenchTabCloseResumeOutcome::Unavailable {
        reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
    }
}
