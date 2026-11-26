//! Workspace files services, the processes behind the window's right bar. The Server asks its
//! Worker to start one, and the Worker starts it as a process of this executable that reads no
//! data root and keeps no log: it connects to the Server's endpoint for its ticket, presents the
//! ticket, and serves the right bar's file and Git work there until the Server closes the
//! connection or it has been idle for ten minutes. The Worker only starts it, so that work never
//! waits on terminal requests or holds them up.

use std::{
    error::Error,
    ffi::{OsStr, OsString},
    io::{self, BufReader, Write},
    net::Shutdown,
    num::NonZeroU64,
    path::{Path, PathBuf},
};

use orchbee_capability_node_protocol::{
    WorkspaceFilesService, WorkspaceFilesServiceOutcome, WorkspaceFilesServiceRequest,
    WorkspaceFilesServiceResult,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::{CohortArtifact, ModeCheckedLocalSessionRendezvous};
use orchbee_local_worker_protocol::WorkerId;

/// The first argument that makes this executable a Workspace files service.
pub(crate) const FLAG: &str = "--workspace-files";

/// What a Worker needs to start services: its own program, as its cohort names it, and the
/// Session's runtime folder, beside which the Server's endpoints are named.
pub(crate) struct WorkspaceFilesStarts {
    executable: PathBuf,
    runtime: PathBuf,
}

impl WorkspaceFilesStarts {
    pub(crate) fn new(cohort: &orchbee_local_runtime::Cohort, runtime: &Path) -> Self {
        Self {
            executable: cohort.executable(CohortArtifact::Worker),
            runtime: runtime.to_owned(),
        }
    }

    /// Starts the service `request` names, unless it names another Worker or generation. The
    /// process is not waited for: the Server observes its connection, and the process ends on its
    /// own.
    pub(crate) fn start(
        &self,
        request: WorkspaceFilesServiceRequest,
        worker_id: WorkerId,
        generation: NonZeroU64,
    ) -> WorkspaceFilesServiceResult {
        let outcome = if request.worker_id.as_bytes() != worker_id.as_bytes()
            || request.worker_generation.get() != generation.get()
        {
            WorkspaceFilesServiceOutcome::WorkerUnavailable
        } else {
            let ticket: String = request
                .ticket
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let arguments = [
                OsStr::new(FLAG),
                OsStr::new(service_name(request.service)),
                OsStr::new("--runtime-directory"),
                self.runtime.as_os_str(),
                OsStr::new("--ticket"),
                OsStr::new(&ticket),
            ];
            // Dropping the owner leaves the process running and reaps it when it exits.
            match orchbee_local_runtime::spawn_retained(&self.executable, &arguments) {
                Ok(_process) => WorkspaceFilesServiceOutcome::Started,
                Err(_) => WorkspaceFilesServiceOutcome::NotStarted,
            }
        };
        WorkspaceFilesServiceResult { request, outcome }
    }
}

const fn service_name(service: WorkspaceFilesService) -> &'static str {
    match service {
        WorkspaceFilesService::Operations => "operations",
        WorkspaceFilesService::Watch => "watch",
        WorkspaceFilesService::Search => "search",
    }
}

/// Runs the service the arguments after [`FLAG`] name:
/// `<operations|watch|search> --runtime-directory <folder> --ticket <32 hex digits>`.
pub(crate) fn run(mut arguments: impl Iterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    let invalid = || io::Error::from(io::ErrorKind::InvalidInput);
    let service = match arguments.next().as_deref().and_then(OsStr::to_str) {
        Some("operations") => WorkspaceFilesService::Operations,
        Some("watch") => WorkspaceFilesService::Watch,
        Some("search") => WorkspaceFilesService::Search,
        _ => return Err(invalid().into()),
    };
    if arguments.next().as_deref() != Some(OsStr::new("--runtime-directory")) {
        return Err(invalid().into());
    }
    let runtime = PathBuf::from(arguments.next().ok_or_else(invalid)?);
    if arguments.next().as_deref() != Some(OsStr::new("--ticket")) {
        return Err(invalid().into());
    }
    let ticket = arguments
        .next()
        .as_deref()
        .and_then(crate::worker_identity::parse_hex_identity)
        .ok_or_else(invalid)?;
    if arguments.next().is_some() {
        return Err(invalid().into());
    }
    let endpoint = ModeCheckedLocalSessionRendezvous::workspace_files_socket_path(&runtime, ticket);
    let mut stream = LocalStream::connect(&endpoint)?;
    stream.write_all(&orchbee_workspace_files::hello(ticket))?;
    stream.flush()?;
    let input = BufReader::new(stream.try_clone()?);
    match service {
        WorkspaceFilesService::Operations => {
            let closing = stream.try_clone()?;
            orchbee_workspace_files::serve_operations(input, stream, move || {
                let _closed = closing.shutdown(Shutdown::Both);
            });
        }
        WorkspaceFilesService::Watch => {
            orchbee_workspace_files::serve_platform_watch(input, stream)
        }
        WorkspaceFilesService::Search => orchbee_workspace_files::serve_search(input, stream),
    }
    Ok(())
}
