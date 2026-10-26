//! Resolve a configured checkout destination before any canonical or Git mutation.
use crate::{
    cli::{
        Failure, FailureKind,
        configuration::Error,
        console_interrupt::{ArmedSignals, SignalWatch},
        query,
    },
    invocation::{WorktreeStartCommand, WorktreeStartKind},
    local_client::{LocalClient, workbench::worktree_query::WorktreeQueryError},
};
use orchbee_workbench_protocol::{
    Snapshot, WorkbenchDirectoryPath, WorkbenchRequestId, WorkbenchWorktreeQuery,
    WorkbenchWorktreeQueryOutcome, WorktreeReference, WorktreeSource,
};
use std::time::{Duration, Instant};

pub(super) struct Source<'a> {
    pub(super) client: &'a mut LocalClient,
    pub(super) snapshot: &'a Snapshot,
    pub(super) deadline: Instant,
    pub(super) signals: &'a mut ArmedSignals,
}

impl Source<'_> {
    pub(super) fn prepare(
        &mut self,
        command: &mut WorktreeStartCommand,
        source: &WorktreeSource,
        operation: &WorkbenchRequestId,
        root: &WorkbenchDirectoryPath,
    ) -> Result<(), Failure> {
        let WorktreeStartKind::Create { branch, path, .. } = &mut command.kind else {
            return Ok(());
        };
        if path.is_some() {
            return Ok(());
        }
        self.check()?;
        let result = self.client.query_worktrees(
            WorkbenchWorktreeQuery {
                protocol_version: self.snapshot.protocol_version,
                request_id: query::request_id()?,
                session_id: self.snapshot.session_id.clone(),
                source: source.clone(),
                trust_repository: command.trust_repository,
            },
            self.deadline.min(Instant::now() + Duration::from_secs(5)),
            self.signals.poll_fd(),
        );
        self.check()?;
        let result = result.map_err(|error| {
            failure(
                match error {
                    WorktreeQueryError::Timeout => FailureKind::Timeout,
                    WorktreeQueryError::Transport => FailureKind::TransportLoss,
                    WorktreeQueryError::Encoding => FailureKind::InternalFailure,
                    WorktreeQueryError::Protocol => FailureKind::ProtocolInvalid,
                    WorktreeQueryError::Unavailable => FailureKind::ServiceUnavailable,
                },
                "source-query-failed",
            )
        })?;
        let WorkbenchWorktreeQueryOutcome::Found {
            session_id,
            workbench_revision,
            source,
            ..
        } = result.outcome
        else {
            return Err(failure(FailureKind::CheckFailed, "source-query-failed"));
        };
        if session_id != self.snapshot.session_id
            || workbench_revision.value() < self.snapshot.revision.value()
        {
            return Err(failure(FailureKind::ProtocolInvalid, "source-query-failed"));
        }
        let chosen = branch.clone().unwrap_or(
            WorktreeReference::parse(&format!("worktree/{operation}"))
                .map_err(|_| failure(FailureKind::InternalFailure, "invalid-path"))?,
        );
        *path = Some(
            crate::worktree_destination::destination(
                root,
                &source.repository_root,
                chosen.as_str(),
            )
            .map_err(|_| Error::new("worktrees.directory", "invalid-path").failure())?,
        );
        *branch = Some(chosen);
        Ok(())
    }

    fn check(&mut self) -> Result<(), Failure> {
        self.signals
            .check_before(self.deadline)
            .map_err(|error| failure(error.kind, "source-query-interrupted"))
    }
}

fn failure(kind: FailureKind, reason: &'static str) -> Failure {
    let mut failure = Error::new("worktrees.directory", reason).failure();
    failure.kind = kind;
    failure
}
