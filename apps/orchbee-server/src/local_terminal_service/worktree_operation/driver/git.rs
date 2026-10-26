//! Admits, polls, and settles retained generation-fenced Worker Git mutations while preserving
//! physical effect and failure distinctions. It performs no Git command locally and never
//! interprets a lost response as permission to resubmit the mutation.

use super::{Driver, absolute};
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
};
use orchbee_capability_node_protocol::{
    OperationId, WorkerGeneration, WorkerId, WorktreeMutation, WorktreeMutationCompletion,
    WorktreeMutationEffect, WorktreeMutationIntent, WorktreeMutationJobAction,
    WorktreeMutationOutcome, WorktreeMutationPollOutcome, WorktreeMutationRequest,
    WorktreeMutationStartOutcome,
};
use orchbee_workbench_protocol::{
    WorkbenchDirectoryPath, WorktreeCheckoutFacts, WorktreeGitEffect, WorktreeIntent,
    WorktreeOperationFailure as Failure, WorktreeReference,
};
use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
impl Driver<'_> {
    pub(super) fn worker(&self) -> Result<WorkerRequestHandle, Failure> {
        let workers = self
            .service
            .workers
            .lock()
            .map_err(|_| Failure::Internal {})?;
        let handle = workers
            .request_handle()
            .ok_or(Failure::WorkerUnavailable {})?;
        if format!(
            "{:032x}",
            u128::from_be_bytes(handle.worker_id().as_bytes())
        ) != self.plan.source.worker_id.as_str()
            || handle.generation() != self.plan.worker_generation
        {
            return Err(Failure::WorkerUnavailable {});
        }
        Ok(handle)
    }
    pub(super) fn job(
        &mut self,
        mutation: WorktreeMutation,
    ) -> Result<WorktreeMutationCompletion, Failure> {
        self.check()?;
        let handle = self.worker()?;
        let mutating = matches!(
            mutation,
            WorktreeMutation::Create { .. } | WorktreeMutation::Remove { .. }
        );
        let trust = match &self.plan.intent {
            WorktreeIntent::Create {
                trust_repository, ..
            }
            | WorktreeIntent::Open {
                trust_repository, ..
            }
            | WorktreeIntent::Remove {
                trust_repository, ..
            } => *trust_repository,
        };
        let source = if matches!(self.plan.intent, WorktreeIntent::Remove { .. })
            && let Some(checkout) = self.plan.progress.checkout.as_ref()
        {
            &checkout.repository_root
        } else {
            &self.plan.source.path
        };
        let intent = WorktreeMutationIntent::new(absolute(source)?, trust, mutation)
            .map_err(|_| Failure::Internal {})?;
        let job_id = operation()?;
        let started = exchange(&handle, WorktreeMutationJobAction::Start(intent), job_id);
        let started = match started {
            Ok(value) => value,
            Err(error) => {
                if mutating && matches!(error, Failure::TransportUnknown {}) {
                    self.plan.progress.git_effect = WorktreeGitEffect::MayHaveChanged;
                }
                return Err(error);
            }
        };
        match started {
            WorktreeMutationOutcome::Start(
                WorktreeMutationStartOutcome::Accepted | WorktreeMutationStartOutcome::Existing,
            ) => {}
            WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Busy) => {
                return Err(Failure::WorkerBusy {});
            }
            WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Full) => {
                return Err(Failure::WorkerCapacity {});
            }
            WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Unsupported) => {
                return Err(Failure::Unsupported {});
            }
            WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::PhysicalUnknown) => {
                return Err(Failure::PhysicalUnknown {});
            }
            // This step consumes its own physical Worktree job receipt; other job states cannot confirm that effect.
            _ => return Err(Failure::WorkerUnavailable {}),
        }
        let deadline = (Instant::now() + Duration::from_secs(40)).min(self.deadline);
        loop {
            if self.stop.load(Ordering::Acquire) || Instant::now() >= deadline {
                let _cancel = exchange(
                    &handle,
                    WorktreeMutationJobAction::Cancel { job_id },
                    operation()?,
                );
                if mutating {
                    self.plan.progress.git_effect = WorktreeGitEffect::MayHaveChanged;
                }
                return Err(if self.stop.load(Ordering::Acquire) {
                    Failure::SessionStopping {}
                } else {
                    Failure::TransportUnknown {}
                });
            }
            let result = exchange(
                &handle,
                WorktreeMutationJobAction::Poll { job_id },
                operation()?,
            );
            match result {
                Err(Failure::WorkerBusy {}) => thread::sleep(Duration::from_millis(50)),
                Ok(WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Pending)) => {
                    thread::sleep(Duration::from_millis(50))
                }
                Ok(WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Completed(
                    WorktreeMutationCompletion::Failed(failure),
                ))) => {
                    if failure.effect == WorktreeMutationEffect::MayHaveChanged {
                        self.plan.progress.git_effect = WorktreeGitEffect::MayHaveChanged;
                    }
                    return Err(git_failure(failure));
                }
                Ok(WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Completed(
                    completion,
                ))) => return Ok(completion),
                Ok(WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::PhysicalUnknown)) => {
                    if mutating {
                        self.plan.progress.git_effect = WorktreeGitEffect::MayHaveChanged;
                    }
                    return Err(Failure::PhysicalUnknown {});
                }
                // This step consumes its own physical Worktree job receipt; other job states cannot confirm that effect.
                _ => {
                    if mutating {
                        self.plan.progress.git_effect = WorktreeGitEffect::MayHaveChanged;
                    }
                    return Err(Failure::TransportUnknown {});
                }
            }
        }
    }
    pub(super) fn checkout_job(
        &mut self,
        mutation: WorktreeMutation,
    ) -> Result<WorktreeCheckoutFacts, Failure> {
        let WorktreeMutationCompletion::Checkout(checkout) = self.job(mutation)? else {
            return Err(Failure::Internal {});
        };
        Ok(WorktreeCheckoutFacts {
            repository_root: WorkbenchDirectoryPath::parse(checkout.repository_root().as_str())
                .map_err(|_| Failure::Internal {})?,
            common_directory: WorkbenchDirectoryPath::parse(checkout.common_directory().as_str())
                .map_err(|_| Failure::Internal {})?,
            path: WorkbenchDirectoryPath::parse(checkout.path().as_str())
                .map_err(|_| Failure::Internal {})?,
            branch: checkout
                .branch()
                .map(|branch| WorktreeReference::parse(branch.as_str()))
                .transpose()
                .map_err(|_| Failure::Internal {})?,
            dirty: checkout.dirty(),
        })
    }
}
fn operation() -> Result<OperationId, Failure> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failure::Internal {})?;
    OperationId::from_bytes(bytes).map_err(|_| Failure::Internal {})
}
fn exchange(
    handle: &WorkerRequestHandle,
    action: WorktreeMutationJobAction,
    operation_id: OperationId,
) -> Result<WorktreeMutationOutcome, Failure> {
    let request = WorktreeMutationRequest {
        operation_id,
        worker_id: WorkerId::from_bytes(handle.worker_id().as_bytes())
            .map_err(|_| Failure::Internal {})?,
        worker_generation: WorkerGeneration::new(handle.generation()),
        action,
    };
    match handle.request(WorkerOperationRequest::WorktreeMutation(request)) {
        Ok(WorkerOperationResult::WorktreeMutation(result)) => Ok(result.outcome),
        Err(WorkerRequestError::Backpressure) => Err(Failure::WorkerBusy {}),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Err(Failure::TransportUnknown {}),
        _ => Err(Failure::WorkerUnavailable {}),
    }
}
fn git_failure(failure: orchbee_capability_node_protocol::WorktreeMutationFailure) -> Failure {
    use orchbee_capability_node_protocol::{
        WorktreeMutationError as E, WorktreeMutationStage as S,
    };
    use orchbee_workbench_protocol::{WorktreeGitError as Error, WorktreeGitStage as Stage};
    let stage = match failure.stage {
        S::Probe => Stage::Probe,
        S::Select => Stage::Select,
        S::CheckBranch => Stage::CheckBranch,
        S::DirtyCheck => Stage::DirtyCheck,
        S::Mutate => Stage::Mutate,
        S::Verify => Stage::Verify,
    };
    let status = if let E::GitFailed(status) = failure.error {
        status
    } else {
        None
    };
    let error = match failure.error {
        E::GitFailed(_) => Error::GitFailed,
        E::SpawnUnavailable => Error::SpawnUnavailable,
        E::SpawnFailed => Error::SpawnFailed,
        E::InvalidRepository => Error::InvalidRepository,
        E::CheckoutNotFound => Error::CheckoutNotFound,
        E::MainCheckout => Error::MainCheckout,
        E::Dirty => Error::Dirty,
        E::InvalidReference => Error::InvalidReference,
        E::TooLarge => Error::TooLarge,
        E::Timeout => Error::Timeout,
        E::Cancelled => Error::Cancelled,
        E::Io => Error::Io,
        E::RepositoryChanged => Error::RepositoryChanged,
        E::AmbiguousCheckout => Error::AmbiguousCheckout,
    };
    Failure::Git {
        stage,
        error,
        status,
        diagnostics: failure.diagnostics().to_vec(),
    }
}
