use super::{Driver, absolute};
use crate::local_terminal_service::{create_reference, lease_expiry, placement, submit};
use orchbee_capability_node_protocol::WorktreeMutation;
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorktreeOperationFailure as Failure, WorktreeOperationPhase,
    WorktreeShellRestoration, WorktreeWorkspaceDisposition,
};

impl Driver<'_> {
    /// Original Git failure remains the operation failure even when recovery also fails.
    pub(super) fn restore_failed_remove(&mut self) {
        let WorktreeWorkspaceDisposition::Closing {
            workspace_id,
            close_request_id,
        } = self.plan.progress.workspace.clone()
        else {
            return;
        };
        let mut verified = false;
        let result = self.restore_shells(&close_request_id, &mut verified);
        if let Err(reason) = result
            && let Some(receipt) = &mut self.plan.progress.restoration
        {
            // A committed Create with no provable settlement must never be launched again.
            receipt.unknown +=
                receipt.attempted - receipt.started - receipt.failed - receipt.unknown;
            receipt.failure = Some(reason);
        }
        // After any rebind, publishing the old captured membership would be invalid. Once
        // verification allowed recovery, end that Close even for known partial launch failure.
        if verified {
            let retained = self
                .service
                .coordinator
                .lock()
                .ok()
                .and_then(|mut site| {
                    site.worktree_authority().map(|authority| {
                        authority.retain_worktree_workspace(self.plan, &close_request_id)
                    })
                })
                .unwrap_or(false);
            if retained {
                self.plan.progress.workspace = WorktreeWorkspaceDisposition::Open { workspace_id };
            } else if let Some(receipt) = &mut self.plan.progress.restoration {
                receipt.failure = Some(Failure::Internal {});
            }
        }
        // The final Failed receipt is also published by the retained submitter if this fails.
        let _published = self.advance(WorktreeOperationPhase::Removing);
    }

    fn restore_shells(
        &mut self,
        close: &WorkbenchRequestId,
        verified: &mut bool,
    ) -> Result<(), Failure> {
        let members = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal {})?
            .worktree_authority()
            .ok_or(Failure::Internal {})?
            .worktree_restore_members(self.plan, close)
            .ok_or(Failure::AssociationFailed {})?;
        self.plan.progress.restoration = Some(WorktreeShellRestoration {
            total: u32::try_from(members.len()).map_err(|_| Failure::Internal {})?,
            attempted: 0,
            started: 0,
            failed: 0,
            unknown: 0,
            failure: None,
        });
        self.advance(WorktreeOperationPhase::Removing)?;
        let original = self
            .plan
            .progress
            .checkout
            .clone()
            .ok_or(Failure::Internal {})?;
        let observed = self.checkout_job(WorktreeMutation::InspectRemove {
            path: absolute(&original.path)?,
        })?;
        if observed.path != original.path
            || observed.common_directory != original.common_directory
            || observed.repository_root != original.repository_root
        {
            return Err(Failure::SourceChanged {});
        }
        *verified = true;
        for member in members {
            self.check()?;
            let candidate = create_reference();
            let expiry = lease_expiry();
            let (pending, handle) = {
                let workers = self
                    .service
                    .workers
                    .lock()
                    .map_err(|_| Failure::Internal {})?;
                let handle = workers
                    .request_handle()
                    .ok_or(Failure::WorkerUnavailable {})?;
                let mut site = self
                    .service
                    .coordinator
                    .lock()
                    .map_err(|_| Failure::Internal {})?;
                let pending = site.prepare_worktree_shell(
                    self.plan,
                    close,
                    &member,
                    candidate,
                    expiry,
                    placement(Some(&handle)),
                )?;
                (pending, handle)
            };
            let completion = submit(&handle, pending.command());
            let workers = self
                .service
                .workers
                .lock()
                .map_err(|_| Failure::Internal {})?;
            let current = placement(workers.request_handle().as_ref());
            self.service
                .coordinator
                .lock()
                .map_err(|_| Failure::Internal {})?
                .complete_worktree_shell(self.plan, pending, completion, current)?;
        }
        Ok(())
    }
}
