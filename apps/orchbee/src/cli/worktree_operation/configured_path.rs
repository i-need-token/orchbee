//! Resolve a configured checkout destination before any canonical or Git mutation.
use crate::{
    cli::{
        Failure, FailureKind,
        configuration::Error,
        query,
        source::{ArmedSignals, SignalWatch},
    },
    invocation::{WorktreeStartCommand, WorktreeStartKind},
    local_client::{LocalClient, workbench::worktree_query::WorktreeQueryError},
};
use orchbee_workbench_protocol::{
    Snapshot, WorkbenchDirectoryPath, WorkbenchRequestId, WorkbenchWorktreeQuery,
    WorkbenchWorktreeQueryOutcome, WorktreeReference, WorktreeSource,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

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
                site_id: self.snapshot.site_id.clone(),
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
            site_id,
            workbench_revision,
            source,
            ..
        } = result.outcome
        else {
            return Err(failure(FailureKind::CheckFailed, "source-query-failed"));
        };
        if site_id != self.snapshot.site_id
            || workbench_revision.value() < self.snapshot.revision.value()
        {
            return Err(failure(FailureKind::ProtocolInvalid, "source-query-failed"));
        }
        let chosen = branch.clone().unwrap_or(
            WorktreeReference::parse(&format!("worktree/{operation}"))
                .map_err(|_| failure(FailureKind::InternalFailure, "invalid-path"))?,
        );
        *path = Some(
            destination(root, &source.repository_root, chosen.as_str()).map_err(Error::failure)?,
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

fn destination(
    root: &WorkbenchDirectoryPath,
    repository: &WorkbenchDirectoryPath,
    branch: &str,
) -> Result<WorkbenchDirectoryPath, Error> {
    let repository = Path::new(repository.as_str())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repository");
    // Git refs are not Windows directory components: slug before joining.
    let path = Path::new(root.as_str()).join(repository).join(slug(branch));
    WorkbenchDirectoryPath::parse(
        path.to_str()
            .ok_or(Error::new("worktrees.directory", "invalid-path"))?,
    )
    .map_err(|_| Error::new("worktrees.directory", "invalid-path"))
}

fn slug(branch: &str) -> String {
    let mut result = String::new();
    for character in branch.chars() {
        if character.is_ascii_alphanumeric() {
            result.push(character.to_ascii_lowercase());
        } else if !result.is_empty() && !result.ends_with('-') {
            result.push('-');
        }
    }
    let length = result.trim_end_matches('-').len();
    result.truncate(length);
    if result.is_empty() {
        result.push_str("worktree");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_herdr_slugs_and_repository_join_keep_destination_inside_root() {
        for (branch, expected) in [
            ("worktree/brave-river", "worktree-brave-river"),
            ("issue/137 Worktree Spaces", "issue-137-worktree-spaces"),
            ("///", "worktree"),
            ("../A__B/", "a-b"),
            ("中文", "worktree"),
        ] {
            assert_eq!(slug(branch), expected);
        }
        let root = WorkbenchDirectoryPath::parse(r"C:\configured").unwrap();
        let repository = WorkbenchDirectoryPath::parse(r"D:\Projects\repo").unwrap();
        assert_eq!(
            Path::new(
                destination(&root, &repository, "Feature/One")
                    .unwrap()
                    .as_str()
            ),
            Path::new(r"C:\configured\repo\feature-one")
        );
    }
}
