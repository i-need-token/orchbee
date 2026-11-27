//! One start of the `orchbee-files` process: a one-shot endpoint and a fresh 256-bit token, the
//! Worker's start, and the admission of the one same-user connection whose hello presents the
//! token. A start without an admitted link within 10 seconds, or whose process comes from another
//! build, has the Worker end the process it started. The endpoint closes as the start ends.

use crate::worker_obcn_carrier::{ExchangeWait, WorkerOperationResult, WorkerRequestHandle};
use orchbee_capability_node_protocol::{
    self as node, FilesEndpoint, FilesProcessAction, FilesProcessIdentity, FilesProcessOutcome,
    FilesProcessRequest, FilesStartOutcome, FilesToken, OperationId,
};
use orchbee_connection_handshake::{ConnectionKind, SourceCommit};
use orchbee_files_protocol::{
    BuildIdentity, LinkHello, LinkMessage, LinkRefusal, LinkSide, LinkToken, read_link_message,
    write_link_message,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use std::{
    io,
    num::NonZeroU64,
    path::Path,
    thread,
    time::{Duration, Instant},
};

/// How long a start may take to reach an admitted link.
const READY_DEADLINE: Duration = Duration::from_secs(10);
/// How long the Worker may take to end a start that failed.
const END_DEADLINE: Duration = Duration::from_secs(5);
/// How often the endpoint is checked for the process's connection.
const ACCEPT_INTERVAL: Duration = Duration::from_millis(10);

/// A started process and its admitted link.
pub(super) struct Launched {
    pub(super) stream: LocalStream,
    pub(super) process: FilesProcessIdentity,
    /// The Worker generation that started it.
    pub(super) generation: NonZeroU64,
}

/// Why a start failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LaunchError {
    /// The process comes from another build.
    BuildMismatch,
    /// The start failed at this stage.
    Failed(&'static str),
}

/// What a hello earns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Judgement {
    /// The process this start created, from this build.
    Admit,
    /// The process this start created, from another build.
    Refuse,
    /// Not this start's process; the endpoint keeps waiting.
    Ignore,
}

/// Starts the process through `worker` with an endpoint under `runtime_directory`.
pub(super) fn launch(
    runtime_directory: &Path,
    worker: &WorkerRequestHandle,
) -> Result<Launched, LaunchError> {
    let deadline = Instant::now() + READY_DEADLINE;
    let token: [u8; 32] = random().ok_or(LaunchError::Failed("random"))?;
    let nonce: [u8; 4] = random().ok_or(LaunchError::Failed("random"))?;
    let path = ModeCheckedLocalSessionRendezvous::files_endpoint_path(runtime_directory, nonce);
    let endpoint = Endpoint::bind(&path).map_err(|_| LaunchError::Failed("bind"))?;
    let address = path
        .to_str()
        .and_then(|address| FilesEndpoint::new(address.to_owned()).ok())
        .ok_or(LaunchError::Failed("endpoint"))?;
    let action = FilesProcessAction::Start {
        endpoint: address,
        token: FilesToken::from_bytes(token),
    };
    let process = match exchange(worker, action, deadline) {
        Some(FilesProcessOutcome::Start(FilesStartOutcome::Started(process))) => process,
        Some(FilesProcessOutcome::Start(FilesStartOutcome::Missing)) => {
            return Err(LaunchError::Failed("missing"));
        }
        Some(FilesProcessOutcome::Start(FilesStartOutcome::Unverified)) => {
            return Err(LaunchError::Failed("unverified"));
        }
        Some(FilesProcessOutcome::Start(FilesStartOutcome::Failed)) => {
            return Err(LaunchError::Failed("spawn"));
        }
        Some(_) | None => return Err(LaunchError::Failed("worker-start")),
    };
    match endpoint.admit(&LinkToken::from_bytes(token), deadline) {
        Ok(stream) => Ok(Launched {
            stream,
            process,
            generation: worker.generation(),
        }),
        Err(error) => {
            end(worker, process);
            Err(error)
        }
    }
}

/// Has `worker` end `process`, waiting a bounded time.
pub(super) fn end(worker: &WorkerRequestHandle, process: FilesProcessIdentity) {
    let _ = exchange(
        worker,
        FilesProcessAction::End { process },
        Instant::now() + END_DEADLINE,
    );
}

/// What a hello earns at a start that handed out `token`, in a Server of `build`.
pub(super) fn judge(hello: &LinkHello, token: &LinkToken, build: &BuildIdentity) -> Judgement {
    if hello.token != *token {
        Judgement::Ignore
    } else if hello.build != *build {
        Judgement::Refuse
    } else {
        Judgement::Admit
    }
}

/// This build as the process's hello names it; `orchbee-files` builds its hello the same way.
pub(super) fn this_build() -> Option<BuildIdentity> {
    BuildIdentity::new(
        env!("CARGO_PKG_VERSION"),
        option_env!("ORCHBEE_SOURCE_COMMIT")
            .and_then(|commit| SourceCommit::parse(commit).ok())
            .map(|commit| *commit.as_bytes()),
        orchbee_local_runtime::environment_stem(),
        *ConnectionKind::ClientServer
            .development_fingerprint()
            .as_bytes(),
    )
    .ok()
}

fn exchange(
    worker: &WorkerRequestHandle,
    action: FilesProcessAction,
    deadline: Instant,
) -> Option<FilesProcessOutcome> {
    let request = FilesProcessRequest {
        operation_id: OperationId::from_bytes(random()?).ok()?,
        worker_id: node::WorkerId::from_bytes(worker.worker_id().as_bytes()).ok()?,
        worker_generation: node::WorkerGeneration::new(worker.generation()),
        action,
    };
    let pending = worker.admit_files_process(request).ok()?;
    match pending.recv_deadline(deadline) {
        ExchangeWait::Ready(result) => match *result {
            Ok(WorkerOperationResult::FilesProcess(result)) => Some(result.outcome),
            _ => None,
        },
        ExchangeWait::Elapsed | ExchangeWait::Lost(_) => None,
    }
}

fn random<const N: usize>() -> Option<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes)
}

/// The one-shot endpoint of one start. On macOS and Linux its socket file is removed when the
/// start ends, unless something else replaced it; a Windows pipe leaves no name behind.
struct Endpoint {
    listener: LocalListener,
    #[cfg(unix)]
    path: std::path::PathBuf,
    #[cfg(unix)]
    file: (u64, u64),
}

impl Endpoint {
    fn bind(path: &Path) -> io::Result<Self> {
        let listener = LocalListener::bind(path)?;
        #[cfg(unix)]
        let file = {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let metadata = std::fs::symlink_metadata(path)?;
            let file = (metadata.dev(), metadata.ino());
            if let Err(error) =
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            {
                let _ = std::fs::remove_file(path);
                return Err(error);
            }
            file
        };
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            #[cfg(unix)]
            path: path.to_owned(),
            #[cfg(unix)]
            file,
        })
    }

    /// Admits the first same-user connection whose hello presents `token` before `deadline`.
    fn admit(&self, token: &LinkToken, deadline: Instant) -> Result<LocalStream, LaunchError> {
        let build = this_build().ok_or(LaunchError::Failed("build"))?;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(LaunchError::Failed("ready-timeout"));
            }
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    thread::sleep(ACCEPT_INTERVAL.min(deadline - now));
                    continue;
                }
                Err(_) => return Err(LaunchError::Failed("accept")),
            };
            match hello(&stream, token, &build, deadline) {
                Ok(Judgement::Admit) => return Ok(stream),
                Ok(Judgement::Refuse) => return Err(LaunchError::BuildMismatch),
                // Another process of this user dialed the endpoint; it learns nothing.
                Ok(Judgement::Ignore) | Err(_) => {}
            }
        }
    }
}

#[cfg(unix)]
impl Drop for Endpoint {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if std::fs::symlink_metadata(&self.path)
            .is_ok_and(|metadata| (metadata.dev(), metadata.ino()) == self.file)
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Reads one connection's hello and answers it when it presents `token`.
fn hello(
    stream: &LocalStream,
    token: &LinkToken,
    build: &BuildIdentity,
    deadline: Instant,
) -> io::Result<Judgement> {
    stream.set_nonblocking(false)?;
    require_same_effective_user(stream)?;
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .max(Duration::from_millis(1));
    stream.set_read_timeout(Some(remaining))?;
    stream.set_write_timeout(Some(remaining))?;
    let LinkMessage::Hello(hello) = read_link_message(&mut &*stream, LinkSide::Files)? else {
        return Ok(Judgement::Ignore);
    };
    let judgement = judge(&hello, token, build);
    match judgement {
        Judgement::Admit => write_link_message(&mut &*stream, &LinkMessage::Admitted)?,
        Judgement::Refuse => write_link_message(
            &mut &*stream,
            &LinkMessage::Refused(LinkRefusal::BuildMismatch),
        )?,
        Judgement::Ignore => {}
    }
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    Ok(judgement)
}

#[cfg(test)]
mod tests;
