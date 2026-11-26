//! `workbench.workspace-files`: the right bar's file and Git facts for one Workspace, saving one of
//! its files, staging, discarding and committing its changes, and searching its contents. The
//! Server resolves the Workspace's own directory and asks a Workspace files service, which its
//! Worker starts only on the first request, so a Session whose windows never open the right bar
//! starts nothing and terminal work never waits on a file read. A watch and a search are answered
//! here, from what their lanes last reported; see [`watch`] and [`search`], and [`launch`] for how
//! a service starts.
mod launch;
mod search;
mod watch;

use super::LocalTerminalService;
use crate::worker_registry::WorkerRegistry;
use launch::{Connection, Launcher};
use orchbee_capability_node_protocol::WorkspaceFilesService;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchResponse, WorkerId, WorkspaceFilesFailure as Failure,
    WorkspaceFilesOperation, WorkspaceFilesOutcome as Outcome, WorkspaceFilesRequest,
    WorkspaceFilesResult, serialize_workbench_response,
};
use orchbee_workspace_files::{Answer, Request, answer_time};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How much longer than the service's own time for an operation the Server waits for its answer:
/// room to carry the request and the answer.
const ANSWER_MARGIN: Duration = Duration::from_millis(1500);
/// One answer line's bytes: the result cap and its envelope.
const MAXIMUM_ANSWER_LINE_BYTES: u64 = 9 * 1024 * 1024;

/// The services: file reads, change lists and long changes run in separate lanes, so a slow
/// `git status` in a large repository never holds up a directory listing, and a commit whose hooks
/// run for a minute, or a replacement across many files, holds up neither.
pub(super) struct WorkspaceFilesProcesses {
    launcher: Arc<Launcher>,
    files: Mutex<Option<Running>>,
    changes: Mutex<Option<Running>>,
    git: Mutex<Option<Running>>,
    watches: watch::Watches,
    searches: search::Searches,
}

impl WorkspaceFilesProcesses {
    pub(super) fn new(workers: Arc<Mutex<WorkerRegistry>>) -> Self {
        let launcher = Arc::new(Launcher::new(workers));
        Self {
            watches: watch::Watches::new(Arc::clone(&launcher)),
            searches: search::Searches::new(Arc::clone(&launcher)),
            launcher,
            files: Mutex::new(None),
            changes: Mutex::new(None),
            git: Mutex::new(None),
        }
    }
}

/// One running operations service; dropping it ends the service.
struct Running {
    connection: Connection,
    /// Disconnects once the service's connection ends.
    answers: Receiver<Vec<u8>>,
    next: u64,
}

impl Running {
    fn start(launcher: &Launcher) -> Result<Self, Failure> {
        let connection = launcher.launch(WorkspaceFilesService::Operations)?;
        let reader = connection.reader().map_err(|_| Failure::Unavailable)?;
        let (sender, answers) = mpsc::channel();
        std::thread::Builder::new()
            .name("orchbee-server-workspace-files".into())
            .spawn(move || {
                let mut reader = BufReader::new(reader);
                loop {
                    let mut line = Vec::new();
                    match (&mut reader)
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
            })
            .map_err(|_| Failure::Unavailable)?;
        Ok(Self {
            connection,
            answers,
            next: 0,
        })
    }

    /// Whether the service's connection has ended, as after its idle time. An answer still
    /// waiting here belongs to an earlier request, which has stopped waiting for it.
    fn ended(&self) -> bool {
        loop {
            match self.answers.try_recv() {
                Ok(_late) => {}
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
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
        self.connection
            .write_line(&line)
            .map_err(|_| Failure::Unavailable)?;
        let deadline = Instant::now() + answer_time(operation).saturating_add(ANSWER_MARGIN);
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
    /// Names the Session's runtime folder, beside which services connect to this Server.
    fn configure(&self, runtime: &Path) {
        self.launcher.configure(runtime);
    }

    /// Asks the lane `operation` belongs to, starting its service if none runs. A service that
    /// fails to answer is ended, and the next request starts a fresh one.
    fn ask(&self, root: &str, operation: &WorkspaceFilesOperation) -> Result<Outcome, Failure> {
        let lane = match operation {
            WorkspaceFilesOperation::GitStatus {} => &self.changes,
            // A save runs in order with reads, so a read after it sees what it wrote.
            WorkspaceFilesOperation::ListDirectory { .. }
            | WorkspaceFilesOperation::ReadFile { .. }
            | WorkspaceFilesOperation::WriteFile { .. } => &self.files,
            // A Git change or a replacement runs alone, in order with the other long changes of
            // every Workspace, and holds up no read.
            WorkspaceFilesOperation::GitStage { .. }
            | WorkspaceFilesOperation::GitUnstage { .. }
            | WorkspaceFilesOperation::GitDiscard { .. }
            | WorkspaceFilesOperation::GitCommit { .. }
            | WorkspaceFilesOperation::ReplaceMatches { .. } => &self.git,
            // A watch, a search and a listing belong to a Client connection, which answers them
            // with `watch_workspace_files` and `search_workspace_files`.
            WorkspaceFilesOperation::Watch { .. }
            | WorkspaceFilesOperation::Search { .. }
            | WorkspaceFilesOperation::ListFiles { .. } => {
                return Err(Failure::Unavailable);
            }
        };
        // The lane stays held while its service starts: its next request waits for that service.
        let mut running = lane.lock().map_err(|_| Failure::Unavailable)?;
        // A service that ended on its own, after its idle time, is replaced.
        if running.as_ref().is_some_and(Running::ended) {
            *running = None;
        }
        if running.is_none() {
            *running = Some(Running::start(&self.launcher)?);
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
    /// Names the Session's runtime folder, without which no Workspace files service starts.
    pub(crate) fn configure_workspace_files(&self, runtime: &Path) {
        self.workspace_files.configure(runtime);
    }

    /// Wakes `signal` whenever a watched Workspace's revisions or watch state change, or a search
    /// or listing finds more or ends.
    pub(crate) fn subscribe_workspace_files(
        &self,
        signal: &std::sync::Arc<orchbee_server::SessionChangeSignal>,
    ) {
        self.workspace_files.watches.subscribe(signal);
        self.workspace_files.searches.subscribe(signal);
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

    /// Answers the next page of the search or listing `request` for the Client connection
    /// `owner`, starting it if it is new and replacing that connection's earlier one of the same
    /// kind of the same Workspace.
    pub(crate) fn search_workspace_files(
        &self,
        owner: std::num::NonZeroU64,
        request: &WorkspaceFilesRequest,
    ) -> WorkspaceFilesResult {
        let query = match &request.operation {
            WorkspaceFilesOperation::Search { query } => Some(search::Query::Search(query)),
            WorkspaceFilesOperation::ListFiles { query } => Some(search::Query::List(query)),
            _ => None,
        };
        let outcome = match (query, request.validate()) {
            (Some(query), Ok(())) => self
                .workspace_files_directory(request)
                .map(|root| {
                    self.workspace_files.searches.page(
                        owner,
                        &request.workspace_id,
                        &request.request_id,
                        &root,
                        query,
                    )
                })
                .unwrap_or_else(|reason| Outcome::Failed { reason }),
            _ => Outcome::Failed {
                reason: Failure::Malformed,
            },
        };
        checked(
            request,
            WorkspaceFilesResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id.clone(),
                session_id: request.session_id.clone(),
                workspace_id: request.workspace_id.clone(),
                outcome,
            },
        )
    }

    /// Ends the connection `owner`'s search or listing of a Workspace while it is still the named
    /// request, or all of them for none.
    pub(crate) fn forget_workspace_files_search(
        &self,
        owner: std::num::NonZeroU64,
        search: Option<(
            &orchbee_workbench_protocol::WorkbenchWorkspaceId,
            &orchbee_workbench_protocol::WorkbenchRequestId,
        )>,
    ) {
        self.workspace_files.searches.forget(owner, search);
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
        checked(&request, result)
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

/// `result` when it answers `request`, holds to its record and encodes; otherwise a malformed
/// failure in its place.
fn checked(request: &WorkspaceFilesRequest, result: WorkspaceFilesResult) -> WorkspaceFilesResult {
    if result.answers(request)
        && result.validate().is_ok()
        && serialize_workbench_response(&WorkbenchResponse::WorkspaceFiles(result.clone())).is_ok()
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
