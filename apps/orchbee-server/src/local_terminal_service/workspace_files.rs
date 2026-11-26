//! `workbench.workspace-files`: the right bar's file and Git facts for one Workspace, and saving
//! one of its files. The Server resolves the Workspace's own directory and asks a separate process of its own
//! executable, started only on the first request, so a Session whose windows never open the right
//! bar starts nothing and terminal work never waits on a file read. A watch is answered here, from
//! what the watch lane last reported; see [`watch`].
mod watch;

use super::LocalTerminalService;
use orchbee_server_workspace_files::{
    Answer, OPERATION_DEADLINE, Request, SERVICE_FLAG, WRITE_DEADLINE,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchResponse, WorkerId, WorkspaceFilesFailure as Failure,
    WorkspaceFilesOperation, WorkspaceFilesOutcome as Outcome, WorkspaceFilesRequest,
    WorkspaceFilesResult, serialize_workbench_response,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long the Server waits for one answer: the service's own deadline and a margin to start.
const ANSWER_DEADLINE: Duration = OPERATION_DEADLINE.saturating_add(Duration::from_millis(1500));
/// How long the Server waits for a save, which the service never cuts short.
const WRITE_ANSWER_DEADLINE: Duration = WRITE_DEADLINE.saturating_add(Duration::from_millis(1500));
/// One answer line's bytes: the result cap and its envelope.
const MAXIMUM_ANSWER_LINE_BYTES: u64 = 9 * 1024 * 1024;

/// The service processes: file reads and change lists run in separate lanes, so a slow `git
/// status` in a large repository never holds up a directory listing.
pub(super) struct WorkspaceFilesProcesses {
    executable: Option<PathBuf>,
    files: Mutex<Option<Running>>,
    changes: Mutex<Option<Running>>,
    watches: watch::Watches,
}

impl Default for WorkspaceFilesProcesses {
    fn default() -> Self {
        Self {
            executable: std::env::current_exe().ok(),
            files: Mutex::new(None),
            changes: Mutex::new(None),
            watches: watch::Watches::default(),
        }
    }
}

/// One running service process; dropping it ends the process.
struct Running {
    child: Child,
    stdin: ChildStdin,
    answers: Receiver<Vec<u8>>,
    next: u64,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _killed = self.child.kill();
        let _reaped = self.child.wait();
    }
}

impl Running {
    fn start(executable: &PathBuf) -> Result<Self, Failure> {
        let mut command = Command::new(executable);
        command
            .arg(SERVICE_FLAG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: the windowless Server's child opens no console.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().map_err(|_| Failure::Unavailable)?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _killed = child.kill();
            let _reaped = child.wait();
            return Err(Failure::Unavailable);
        };
        let (sender, answers) = mpsc::channel();
        let reader = std::thread::Builder::new()
            .name("orchbee-server-workspace-files".into())
            .spawn(move || {
                let mut stdout = BufReader::new(stdout);
                loop {
                    let mut line = Vec::new();
                    match (&mut stdout)
                        .take(MAXIMUM_ANSWER_LINE_BYTES)
                        .read_until(b'\n', &mut line)
                    {
                        Ok(0) | Err(_) => return,
                        Ok(_) => {
                            if sender.send(line).is_err() {
                                return;
                            }
                        }
                    }
                }
            });
        if reader.is_err() {
            let _killed = child.kill();
            let _reaped = child.wait();
            return Err(Failure::Unavailable);
        }
        Ok(Self {
            child,
            stdin,
            answers,
            next: 0,
        })
    }

    fn ask(&mut self, root: &str, operation: &WorkspaceFilesOperation) -> Result<Outcome, Failure> {
        self.next += 1;
        let id = self.next;
        let mut line = serde_json::to_vec(&Request {
            id,
            root: root.to_owned(),
            operation: operation.clone(),
        })
        .map_err(|_| Failure::Malformed)?;
        line.push(b'\n');
        self.stdin
            .write_all(&line)
            .and_then(|()| self.stdin.flush())
            .map_err(|_| Failure::Unavailable)?;
        let deadline = Instant::now()
            + if operation.writes() {
                WRITE_ANSWER_DEADLINE
            } else {
                ANSWER_DEADLINE
            };
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.answers.recv_timeout(left) {
                Ok(line) => match serde_json::from_slice::<Answer>(&line) {
                    Ok(answer) if answer.id == id => return Ok(answer.outcome),
                    // An earlier request's late answer: keep waiting for this one.
                    Ok(answer) if answer.id < id => {}
                    _ => return Err(Failure::Malformed),
                },
                Err(RecvTimeoutError::Timeout) => return Err(Failure::Timeout),
                Err(RecvTimeoutError::Disconnected) => return Err(Failure::Unavailable),
            }
        }
    }
}

impl WorkspaceFilesProcesses {
    /// Asks the lane `operation` belongs to, starting its process if none runs. A process that
    /// fails to answer is ended, and the next request starts a fresh one.
    fn ask(&self, root: &str, operation: &WorkspaceFilesOperation) -> Result<Outcome, Failure> {
        let lane = match operation {
            WorkspaceFilesOperation::GitStatus {} => &self.changes,
            // A save runs in order with reads, so a read after it sees what it wrote.
            WorkspaceFilesOperation::ListDirectory { .. }
            | WorkspaceFilesOperation::ReadFile { .. }
            | WorkspaceFilesOperation::WriteFile { .. } => &self.files,
            // A watch belongs to a Client connection, which answers it with `watch_workspace_files`.
            WorkspaceFilesOperation::Watch { .. } => return Err(Failure::Unavailable),
        };
        let mut running = lane.lock().map_err(|_| Failure::Unavailable)?;
        // A process that exited on its own, after its idle time, is replaced.
        if running
            .as_mut()
            .is_some_and(|process| !matches!(process.child.try_wait(), Ok(None)))
        {
            *running = None;
        }
        if running.is_none() {
            let executable = self.executable.as_ref().ok_or(Failure::Unavailable)?;
            *running = Some(Running::start(executable)?);
        }
        let process = running.as_mut().ok_or(Failure::Unavailable)?;
        let answer = process.ask(root, operation);
        if answer.is_err() {
            *running = None;
        }
        answer
    }
}

impl LocalTerminalService {
    /// Wakes `signal` whenever a watched Workspace's revisions or watch state change.
    pub(crate) fn subscribe_workspace_files(
        &self,
        signal: &std::sync::Arc<orchbee_server::SessionChangeSignal>,
    ) {
        self.workspace_files.watches.subscribe(signal);
    }

    /// Answers the watch `request` for the Client connection `owner`, replacing that
    /// connection's earlier watch of the same Workspace.
    pub(crate) fn watch_workspace_files(
        &self,
        owner: std::num::NonZeroU64,
        request: &WorkspaceFilesRequest,
    ) -> WorkspaceFilesResult {
        let outcome = match (&request.operation, request.validate()) {
            (WorkspaceFilesOperation::Watch { directories, files }, Ok(())) => self
                .workspace_files_directory(request)
                .map(|root| {
                    self.workspace_files.watches.watch(
                        owner,
                        &request.workspace_id,
                        &root,
                        directories,
                        files,
                    )
                })
                .unwrap_or_else(|reason| Outcome::Failed { reason }),
            _ => Outcome::Failed {
                reason: Failure::Malformed,
            },
        };
        WorkspaceFilesResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            workspace_id: request.workspace_id.clone(),
            outcome,
        }
    }

    /// Ends the connection `owner`'s watch of `workspace`, or all its watches for none.
    pub(crate) fn forget_workspace_files_watch(
        &self,
        owner: std::num::NonZeroU64,
        workspace: Option<&orchbee_workbench_protocol::WorkbenchWorkspaceId>,
    ) {
        self.workspace_files.watches.forget(owner, workspace);
    }

    pub(crate) fn execute_workspace_files(
        &self,
        request: WorkspaceFilesRequest,
    ) -> WorkspaceFilesResult {
        let outcome = match request.validate() {
            Err(_) => Outcome::Failed {
                reason: Failure::Malformed,
            },
            Ok(()) => self
                .workspace_files_directory(&request)
                .and_then(|root| self.workspace_files.ask(&root, &request.operation))
                .unwrap_or_else(|reason| Outcome::Failed { reason }),
        };
        let result = WorkspaceFilesResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            workspace_id: request.workspace_id.clone(),
            outcome,
        };
        if result.answers(&request)
            && result.validate().is_ok()
            && serialize_workbench_response(&WorkbenchResponse::WorkspaceFiles(result.clone()))
                .is_ok()
        {
            result
        } else {
            WorkspaceFilesResult {
                outcome: Outcome::Failed {
                    reason: Failure::Malformed,
                },
                ..result
            }
        }
    }

    /// The Workspace's primary directory, when this Session's active Worker owns it.
    fn workspace_files_directory(
        &self,
        request: &WorkspaceFilesRequest,
    ) -> Result<String, Failure> {
        let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
        let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
        let authority = coordinator
            .worktree_authority()
            .ok_or(Failure::Unavailable)?;
        if authority.session_id() != &request.session_id {
            return Err(Failure::Denied);
        }
        let navigation = &authority.snapshot().navigation;
        if navigation.workspace(&request.workspace_id).is_none() {
            return Err(Failure::NotFound);
        }
        let directory = navigation
            .workspace_directory(&request.workspace_id)
            .ok_or(Failure::Unavailable)?;
        let handle = workers
            .request_handle()
            .filter(|handle| handle.is_active())
            .ok_or(Failure::Unavailable)?;
        let worker = WorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(handle.worker_id().as_bytes())
        ))
        .map_err(|_| Failure::Unavailable)?;
        // Only a directory on this machine's Worker is this process's to read.
        if directory.worker_id != worker {
            return Err(Failure::Unavailable);
        }
        Ok(directory.path.as_str().to_owned())
    }
}
