//! Each Git command of a mutation, run by `orchbee-git-process` under one exact process owner with
//! bounded output, and its outcome in the mutation's failure vocabulary.
use super::{Abort, Context, failed};
use orchbee_capability_node_protocol::{WorktreeMutationError as Error, WorktreeMutationStage};
use orchbee_git_process::{GIT_PROGRAM, Output, OutputBounds, Run, RunError};
use std::{io, path::Path, process::Command, sync::atomic::Ordering, time::Instant};

const BOUNDS: OutputBounds = OutputBounds {
    stdout: 64 * 1024,
    stderr_tail: 2048,
};

impl Context<'_> {
    pub(super) fn git(
        &mut self,
        at: &str,
        args: &[&str],
        stage: WorktreeMutationStage,
        mutation: bool,
    ) -> Result<Output, Abort> {
        self.stage = stage;
        self.check()?;
        let mut command = Command::new(GIT_PROGRAM);
        crate::git_repository::configure(&mut command, Path::new(at), self.trust)
            .map_err(|_| self.failure(Error::InvalidRepository, vec![]))?;
        command.args(["-C", at]).args(args);
        // Begin the conservative handoff observation; a proven pre-creation
        // failure below restores the previous effect fact.
        let previous_effect = self.changed;
        self.changed |= mutation;
        // The Windows wrapper may fail after suspended creation or Job assignment.
        // Retire on post-creation failure; an observed no-child failure preserves
        // normal error reporting and does not poison the owner.
        match Run::new(command, BOUNDS).wait(|| self.check()) {
            Ok(output) => Ok(output),
            Err(RunError::Pipe) => {
                self.changed = previous_effect;
                Err(self.failure(Error::Io, vec![]))
            }
            Err(RunError::Spawn(kind)) => {
                self.changed = previous_effect;
                Err(self.failure(
                    if kind == io::ErrorKind::NotFound {
                        Error::SpawnUnavailable
                    } else {
                        Error::SpawnFailed
                    },
                    vec![],
                ))
            }
            Err(RunError::Unsettled) => Err(Abort::PhysicalUnknown),
            Err(RunError::Stopped(abort)) => Err(abort),
            Err(RunError::TooLarge(diagnostics)) => Err(self.failure(Error::TooLarge, diagnostics)),
            Err(RunError::Io(diagnostics)) => Err(self.failure(Error::Io, diagnostics)),
        }
    }

    pub(super) fn required(
        &mut self,
        at: &str,
        args: &[&str],
        stage: WorktreeMutationStage,
        mutation: bool,
    ) -> Result<Vec<u8>, Abort> {
        let output = self.git(at, args, stage, mutation)?;
        if !output.status.success() {
            return Err(self.failure(Error::GitFailed(output.status.code()), output.stderr));
        }
        Ok(output.stdout)
    }

    pub(super) fn check(&self) -> Result<(), Abort> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(self.failure(Error::Cancelled, vec![]));
        }
        if Instant::now() >= self.deadline {
            return Err(self.failure(Error::Timeout, vec![]));
        }
        Ok(())
    }

    pub(super) fn failure(&self, error: Error, diagnostics: Vec<u8>) -> Abort {
        failed(self.stage, self.changed, error, diagnostics)
    }
}
