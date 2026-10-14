//! Bounded, nonblocking output and one exact Job owner for each Git command.
use super::{Abort, Context, failed};
use orchbee_capability_node_protocol::{WorktreeMutationError as Error, WorktreeMutationStage};
use orchbee_local_ipc::ChildOutputReader;
use orchbee_local_site_runtime::WindowsCommandChild;
use std::{
    io::{self, Read},
    path::Path,
    process::{Command, ExitStatus, Stdio},
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

const STDOUT_CAP: usize = 64 * 1024;
const STDERR_CAP: usize = 2048;

pub(super) struct Output {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

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
        let mut command = Command::new("git.exe");
        crate::git_repository_windows::configure(&mut command, Path::new(at), self.trust)
            .map_err(|_| self.failure(Error::InvalidRepository, vec![]))?;
        command.args(["-C", at]).args(args).stdin(Stdio::null());
        let (out, stdout) =
            ChildOutputReader::pipe().map_err(|_| self.failure(Error::Io, vec![]))?;
        let (err, stderr) =
            ChildOutputReader::pipe().map_err(|_| self.failure(Error::Io, vec![]))?;
        command.stdout(stdout).stderr(stderr);
        // Begin the conservative handoff observation; a proven pre-creation
        // failure below restores the previous effect fact.
        let previous_effect = self.changed;
        self.changed |= mutation;
        // The wrapper may fail after suspended creation or Job assignment.
        // Retire on post-creation failure; an observed no-child failure preserves
        // normal error reporting and does not poison the owner.
        let mut child = WindowsCommandChild::spawn(command).map_err(|failure| {
            if failure.created {
                Abort::PhysicalUnknown
            } else {
                self.changed = previous_effect;
                self.failure(
                    if failure.error.kind() == io::ErrorKind::NotFound {
                        Error::SpawnUnavailable
                    } else {
                        Error::SpawnFailed
                    },
                    vec![],
                )
            }
        })?;
        let mut out = Some(out);
        let mut err = Some(err);
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        loop {
            let observation = self.check().and_then(|()| {
                drain(&mut out, &mut output, STDOUT_CAP, true, false)
                    .and_then(|()| drain(&mut err, &mut diagnostics, STDERR_CAP, false, false))
                    .map_err(|error| self.failure(error, diagnostics.clone()))
            });
            if let Err(failure) = observation {
                child.settle().map_err(|_| Abort::PhysicalUnknown)?;
                return Err(failure);
            }
            let status = match child.try_wait() {
                Ok(Some(status)) => status,
                Ok(None) => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(_) => {
                    child.settle().map_err(|_| Abort::PhysicalUnknown)?;
                    return Err(self.failure(Error::Io, diagnostics));
                }
            };
            // The Job is settled: no descendant can keep writing. Repeated
            // bounded drains consume the finite buffered tail, never wait for EOF.
            while out.is_some() || err.is_some() {
                self.check()?;
                drain(&mut out, &mut output, STDOUT_CAP, true, true)
                    .and_then(|()| drain(&mut err, &mut diagnostics, STDERR_CAP, false, true))
                    .map_err(|error| self.failure(error, diagnostics.clone()))?;
            }
            return Ok(Output {
                status,
                stdout: output,
                stderr: diagnostics,
            });
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

fn drain(
    reader: &mut Option<ChildOutputReader>,
    retained: &mut Vec<u8>,
    cap: usize,
    reject_overflow: bool,
    settled: bool,
) -> Result<(), Error> {
    let Some(pipe) = reader.as_mut() else {
        return Ok(());
    };
    let mut buffer = [0; 4096];
    for _ in 0..8 {
        match pipe.read(&mut buffer) {
            Ok(0) => {
                *reader = None;
                return Ok(());
            }
            Ok(count) => {
                if reject_overflow && retained.len() + count > cap {
                    return Err(Error::TooLarge);
                }
                // Git's final error follows its progress output. Keep the bounded
                // tail of stderr so progress cannot hide the cause of failure.
                if !reject_overflow && retained.len() + count > cap {
                    let overflow = retained.len() + count - cap;
                    if overflow >= retained.len() {
                        retained.clear();
                        retained.extend_from_slice(&buffer[count - cap..count]);
                    } else {
                        retained.drain(..overflow);
                        retained.extend_from_slice(&buffer[..count]);
                    }
                } else {
                    retained.extend_from_slice(&buffer[..count]);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if settled {
                    *reader = None;
                }
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Error::Io),
        }
    }
    Ok(())
}
