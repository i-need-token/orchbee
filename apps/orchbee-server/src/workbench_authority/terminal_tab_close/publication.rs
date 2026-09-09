use std::time::Instant;

use orchbee_workbench_protocol::{
    WorkbenchProtocolDocument, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus, serialize_document,
};

use super::super::{
    RetainedRequest, WorkbenchAuthority, WorkbenchTerminalMembership, WorkbenchTerminalTab,
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
    before: WorkbenchTerminalMembership,
    after: WorkbenchTerminalMembership,
    removed: WorkbenchTerminalTab,
    removed_members: Vec<WorkbenchTerminalTab>,
    revision: WorkbenchRevision,
    status: WorkbenchTabCloseStatus,
}

impl PreparedWorkbenchTabClosePublication<'_> {
    #[must_use]
    pub fn before_membership(&self) -> &WorkbenchTerminalMembership {
        &self.before
    }
    #[must_use]
    pub fn after_membership(&self) -> &WorkbenchTerminalMembership {
        &self.after
    }
    #[must_use]
    pub fn exact_removed_member(&self) -> &WorkbenchTerminalTab {
        &self.removed
    }
    #[must_use]
    pub fn exact_removed_members(&self) -> &[WorkbenchTerminalTab] {
        &self.removed_members
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
            .publication_failed(WorkbenchTabClosePublicationReason::AuthorityRetired);
    }

    /// Commit together with Site's separately prepared reference update under the same gate.
    #[must_use]
    pub fn commit(self) -> WorkbenchTabCloseStatus {
        self.projection.commit_prepared(self.prepared);
        self.record.commit_closed(self.revision);
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
        if record.next_step(now).next() != super::continuation::CloseContinuationNext::Publication
            || !record.publication_ready()
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
        let targets: Vec<_> = record.targets().cloned().collect();
        let target = targets
            .first()
            .cloned()
            .ok_or(WorkbenchTabClosePublicationError::CompositionMismatch)?;
        if self.ledger_bytes < record.reserved_bytes {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        }
        let before = match self.terminal_tree_membership(&target.terminal_context_id) {
            Ok(membership) => membership,
            Err(_error) => {
                return self.fail_close_publication(
                    close_request_id,
                    WorkbenchTabClosePublicationError::CompositionMismatch,
                );
            }
        };
        let actual_tab_members: Vec<_> = before.members_for_tab(target.member.tab_id()).collect();
        if before.terminal_context_id() != &target.terminal_context_id
            || actual_tab_members.len() != targets.len()
            || !actual_tab_members
                .into_iter()
                .eq(targets.iter().map(|target| &target.member))
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
        let after = match super::super::terminal_membership::tree_terminal_membership(
            &candidate,
            &target.terminal_context_id,
        ) {
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
            removed_members: targets.into_iter().map(|target| target.member).collect(),
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
            record.publication_failed(reason);
        }
        Err(failure)
    }
}
