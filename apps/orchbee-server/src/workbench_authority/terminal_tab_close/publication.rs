use std::time::Instant;

use orchbee_workbench_protocol::{
    WorkbenchProtocolDocument, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus, serialize_document,
};

use super::super::{
    RetainedRequest, WorkbenchAuthority, WorkbenchStandaloneMembership, WorkbenchStandaloneTab,
};
use super::CloseRecord;
use crate::workbench_projection::{PreparedProjectionAdvance, WorkbenchProjectionStore};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchTabClosePublicationError {
    OperationUnavailable,
    NotReady,
    AuthorityRetired,
    CompositionMismatch,
    PreparationFailed,
}

pub struct PreparedWorkbenchTabClosePublication<'a> {
    projection: &'a mut WorkbenchProjectionStore,
    record: &'a mut CloseRecord,
    retired: &'a mut bool,
    prepared: PreparedProjectionAdvance,
    before: WorkbenchStandaloneMembership,
    after: WorkbenchStandaloneMembership,
    removed: WorkbenchStandaloneTab,
    revision: WorkbenchRevision,
    status: WorkbenchTabCloseStatus,
}

impl PreparedWorkbenchTabClosePublication<'_> {
    #[must_use]
    pub fn before_membership(&self) -> &WorkbenchStandaloneMembership {
        &self.before
    }
    #[must_use]
    pub fn after_membership(&self) -> &WorkbenchStandaloneMembership {
        &self.after
    }
    #[must_use]
    pub fn exact_removed_member(&self) -> &WorkbenchStandaloneTab {
        &self.removed
    }
    #[must_use]
    pub fn status(&self) -> &WorkbenchTabCloseStatus {
        &self.status
    }
    #[must_use]
    pub const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    /// Site composition failure retains known disposal; it cannot become a pre-effect rejection.
    pub fn retire_composition_failure(self) {
        *self.retired = true;
        self.record
            .continuation
            .publication_failed(WorkbenchTabClosePublicationReason::AuthorityRetired);
    }

    /// Commit together with Site's separately prepared reference update under the same gate.
    #[must_use]
    pub fn commit(self) -> WorkbenchTabCloseStatus {
        self.projection.commit_prepared(self.prepared);
        self.record.continuation.commit_closed(self.revision);
        self.status
    }
}

impl WorkbenchAuthority {
    pub fn prepare_terminal_tab_close_publication(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        now: Instant,
    ) -> Result<PreparedWorkbenchTabClosePublication<'_>, WorkbenchTabClosePublicationError> {
        let Some(record) = self.close_record_mut(close_request_id) else {
            return Err(WorkbenchTabClosePublicationError::OperationUnavailable);
        };
        if record.continuation.next(now) != super::continuation::CloseContinuationNext::Publication
            || !record.continuation.publication_ready()
        {
            return Err(WorkbenchTabClosePublicationError::NotReady);
        }
        let record = self
            .close_record(close_request_id)
            .ok_or(WorkbenchTabClosePublicationError::OperationUnavailable)?;
        if self.retired {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::AuthorityRetired,
            );
        }
        let target = record.target.clone();
        if self.ledger_bytes < record.reserved_bytes {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        }
        let before = match self.standalone_membership() {
            Ok(membership) => membership,
            Err(_error) => {
                return self.fail_close_publication(
                    close_request_id,
                    WorkbenchTabClosePublicationError::CompositionMismatch,
                );
            }
        };
        let mut exact = before
            .members()
            .iter()
            .filter(|member| *member == &target.member);
        if before.terminal_context_id() != &target.terminal_context_id
            || exact.next().is_none()
            || exact.next().is_some()
        {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        }
        let mut candidate = self.projection.snapshot().clone();
        let Some(revision) = candidate
            .revision
            .value()
            .checked_add(1)
            .map(WorkbenchRevision::new)
        else {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        };
        let Some(context) = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == target.terminal_context_id)
        else {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        };
        // Strict membership proved the exact Tab/Panel/Presentation/binding tuple above.
        context
            .tabs
            .retain(|tab| &tab.tab_id != target.member.tab_id());
        candidate.revision = revision;
        let after = match super::super::terminal_membership::standalone_membership(&candidate) {
            Ok(membership) => membership,
            Err(_error) => {
                return self.fail_close_publication(
                    close_request_id,
                    WorkbenchTabClosePublicationError::CompositionMismatch,
                );
            }
        };
        let prepared = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate))
            .ok()
            .and_then(|encoded| self.projection.prepare_advance(&encoded).ok());
        let Some(prepared) = prepared else {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::PreparationFailed,
            );
        };
        let status = WorkbenchTabCloseStatus::Closed { revision };
        let Some(RetainedRequest::TabClose(record)) = self.ledger.get_mut(close_request_id) else {
            return Err(WorkbenchTabClosePublicationError::OperationUnavailable);
        };
        Ok(PreparedWorkbenchTabClosePublication {
            projection: &mut self.projection,
            record,
            retired: &mut self.retired,
            prepared,
            before,
            after,
            removed: target.member,
            revision,
            status,
        })
    }

    fn fail_close_publication<T>(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        failure: WorkbenchTabClosePublicationError,
    ) -> Result<T, WorkbenchTabClosePublicationError> {
        if matches!(
            failure,
            WorkbenchTabClosePublicationError::CompositionMismatch
        ) {
            self.retired = true;
        }
        let reason = if self.retired {
            WorkbenchTabClosePublicationReason::AuthorityRetired
        } else {
            WorkbenchTabClosePublicationReason::PreparationFailed
        };
        if let Some(record) = self.close_record_mut(close_request_id) {
            record.continuation.publication_failed(reason);
        }
        Err(failure)
    }
}
