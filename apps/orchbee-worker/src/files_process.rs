//! Starts and ends this Worker generation's one `orchbee-files` process at the Server's request,
//! and is its only reaper. A start checks the cohort's `orchbee-files` against the digest the
//! cohort declares, ends a process this Worker still holds, and starts the new one with the
//! endpoint the Server bound, its token on standard input and the environment a terminal launch
//! starts from: in a kill-on-close Job on Windows, as the leader of its own process group through
//! the retained spawn on macOS and Linux. It answers as soon as the process exists; the process's
//! admission, link, scope and effects are between it and the Server.

use std::{
    cell::RefCell,
    error::Error,
    io::{self, Write as _},
    num::NonZeroU64,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    FilesEndOutcome, FilesProcessAction, FilesProcessIdentity, FilesProcessOutcome,
    FilesProcessRequest, FilesProcessResult, FilesStartOutcome, FilesToken,
};
use orchbee_local_runtime::{Cohort, CommandChild, FilesExecutableError};
use orchbee_local_worker_protocol::WorkerId;

use crate::default_interactive_launch::SystemShell;

/// How long checking the cohort's `orchbee-files` may take: reading a few megabytes.
const VERIFY_BUDGET: Duration = Duration::from_secs(5);

/// The one `orchbee-files` process of this Worker generation, and where its program lives.
pub(crate) struct FilesProcess<'a> {
    /// The cohort whose `orchbee-files` a start runs; a Worker started with explicit endpoints has
    /// none and answers every start `Missing`.
    cohort: Option<&'a Cohort>,
    held: RefCell<Option<Held>>,
}

struct Held {
    identity: FilesProcessIdentity,
    child: CommandChild,
}

impl<'a> FilesProcess<'a> {
    /// An owner that starts `cohort`'s `orchbee-files`, or nothing without a cohort.
    pub(crate) const fn new(cohort: Option<&'a Cohort>) -> Self {
        Self {
            cohort,
            held: RefCell::new(None),
        }
    }

    /// Answers `request`, a start or an end fenced to this Worker generation. A start reads
    /// `system_shell`'s launch `PATH` only when it spawns.
    pub(crate) fn execute(
        &self,
        request: FilesProcessRequest,
        worker_id: WorkerId,
        generation: NonZeroU64,
        system_shell: &SystemShell,
    ) -> FilesProcessResult {
        let fenced = request.worker_id.as_bytes() == worker_id.as_bytes()
            && request.worker_generation.get() == generation.get();
        let outcome = match request.action {
            FilesProcessAction::Start { .. } if !fenced => {
                FilesProcessOutcome::Start(FilesStartOutcome::RejectedFencing)
            }
            FilesProcessAction::End { .. } if !fenced => {
                FilesProcessOutcome::End(FilesEndOutcome::RejectedFencing)
            }
            FilesProcessAction::Start { endpoint, token } => {
                FilesProcessOutcome::Start(self.start(endpoint.as_str(), &token, system_shell))
            }
            FilesProcessAction::End { process } => FilesProcessOutcome::End(self.end(process)),
        };
        FilesProcessResult {
            operation_id: request.operation_id,
            outcome,
        }
    }

    /// Reaps the held process once it has ended on its own, as at its idle exit, so an ended
    /// process waits for no later request.
    pub(crate) fn reap(&self) {
        let mut held = self.held.borrow_mut();
        if held
            .as_mut()
            .is_some_and(|current| !matches!(current.child.try_wait(), Ok(None)))
        {
            // A wait that failed leaves the owner's drop to end and reap what remains.
            *held = None;
        }
    }

    fn start(
        &self,
        endpoint: &str,
        token: &FilesToken,
        system_shell: &SystemShell,
    ) -> FilesStartOutcome {
        // The Server asks for a start only when it holds no link, so a process still held here is
        // one whose link ended; it goes first, keeping one process per generation.
        if let Err(error) = self.end_held() {
            crate::diagnostic_events::files_start_failed("previous", &error);
            return FilesStartOutcome::Failed;
        }
        let Some(cohort) = self.cohort else {
            return FilesStartOutcome::Missing;
        };
        let executable = match cohort.verified_files_executable(Instant::now() + VERIFY_BUDGET) {
            Ok(executable) => executable,
            Err(FilesExecutableError::Missing) => return FilesStartOutcome::Missing,
            Err(FilesExecutableError::Unverified(error)) => {
                crate::diagnostic_events::files_start_failed("unverified", &error);
                return FilesStartOutcome::Unverified;
            }
        };
        match spawn(&executable, endpoint, token, launch_path(system_shell)) {
            Ok((child, identity)) => {
                *self.held.borrow_mut() = Some(Held { identity, child });
                FilesStartOutcome::Started(identity)
            }
            Err(error) => {
                crate::diagnostic_events::files_start_failed("spawn", error.as_ref());
                FilesStartOutcome::Failed
            }
        }
    }

    fn end(&self, process: FilesProcessIdentity) -> FilesEndOutcome {
        self.reap();
        if self
            .held
            .borrow()
            .as_ref()
            .is_none_or(|current| current.identity != process)
        {
            return FilesEndOutcome::Unknown;
        }
        match self.end_held() {
            Ok(()) => FilesEndOutcome::Ended,
            Err(_) => FilesEndOutcome::Failed,
        }
    }

    /// Ends and reaps the held process, when there is one: `SIGTERM` to its group and then
    /// `SIGKILL` on macOS and Linux, its Job's end on Windows, within the settle budget. A process
    /// that does not settle is released all the same, and its owner's drop kills what remains, so
    /// one stuck process never blocks every later start.
    fn end_held(&self) -> io::Result<()> {
        let Some(mut current) = self.held.borrow_mut().take() else {
            return Ok(());
        };
        current.child.settle().map(drop)
    }
}

/// The `PATH` a started process gets: on macOS a new terminal's login `PATH`, which a Worker that
/// launchd started lacks; elsewhere the Worker's own, which a terminal launch also starts from.
#[cfg(target_os = "macos")]
fn launch_path(system_shell: &SystemShell) -> Option<String> {
    system_shell.launch_path().map(str::to_owned)
}

#[cfg(not(target_os = "macos"))]
const fn launch_path(_system_shell: &SystemShell) -> Option<String> {
    None
}

/// Starts `executable` with `--local-endpoint <endpoint>` and the token line on standard input,
/// and reads its exact identity while the root is held unreaped.
fn spawn(
    executable: &Path,
    endpoint: &str,
    token: &FilesToken,
    path: Option<String>,
) -> Result<(CommandChild, FilesProcessIdentity), Box<dyn Error>> {
    let (input, mut writer) = io::pipe()?;
    // A pipe holds far more than the 65-byte line, so the write completes before the process
    // starts, and closing the writer ends the input after it.
    writer.write_all(&token_line(token))?;
    drop(writer);
    let mut command = Command::new(executable);
    command.arg("--local-endpoint").arg(endpoint);
    if let Some(path) = path {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    let child = {
        command
            .stdin(input)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        CommandChild::spawn_contained(command)
    };
    #[cfg(unix)]
    let child = {
        let null = std::fs::File::options().write(true).open("/dev/null")?;
        CommandChild::spawn_with_streams(
            command,
            orchbee_local_runtime::SpawnEnvironment::Inherited,
            [input.into(), null.try_clone()?.into(), null.into()],
        )
    };
    let mut child = child.map_err(|failure| failure.error)?;
    match child.exact_identity() {
        Ok((process_id, start)) => Ok((child, FilesProcessIdentity { process_id, start })),
        Err(error) => {
            // A process the Server cannot name, it cannot end, so it does not outlive the start.
            let _settled = child.settle();
            Err(error.into())
        }
    }
}

/// The token as `orchbee-files` reads it: 64 lowercase hexadecimal digits and a line feed.
fn token_line(token: &FilesToken) -> Vec<u8> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut line = Vec::with_capacity(65);
    for byte in token.as_bytes() {
        line.push(DIGITS[usize::from(byte >> 4)]);
        line.push(DIGITS[usize::from(byte & 0x0f)]);
    }
    line.push(b'\n');
    line
}

#[cfg(test)]
mod tests;
