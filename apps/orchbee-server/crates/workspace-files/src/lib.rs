//! The Workspace file service behind `workbench.workspace-files`: it reads, and saves a file only
//! while it is still the version a client read. The Server runs it in a child process of its own
//! executable only once a Client asks, and talks to it over standard input and output, one JSON
//! object per line; a second child, the watch lane, follows changes. See this crate's README.
mod change;
mod files;
mod git;
mod status;
mod system_trash;
mod watch;
mod write;

use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILES_RESULT_BYTES, MAXIMUM_WORKSPACE_FILES_WRITE_REQUEST_BYTES,
    WorkspaceFileSource, WorkspaceFilesFailure as Failure, WorkspaceFilesOperation,
    WorkspaceFilesOutcome as Outcome,
};
use serde::{Deserialize, Serialize};
pub use status::BlobCache;
use std::io::{BufRead, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
pub use watch::{
    Backend, COALESCE, Coalescer, MAXIMUM_WATCHED_ROOTS, Notice, WATCH_FLAG, WatchEvent, WatchRoot,
    WatchSet, serve_watch, serve_watch_stdio,
};

/// The command-line flag that turns the Server's executable into this service.
pub const SERVICE_FLAG: &str = "--workspace-files";
/// How long one read may run inside the service; the Server waits a little longer.
pub const OPERATION_DEADLINE: Duration = Duration::from_secs(4);
/// How long the Server waits for a save, which writes and syncs up to the editable size and is
/// never cut short inside the service, so a slow disk does not report a save that then lands.
pub const WRITE_DEADLINE: Duration = Duration::from_secs(20);
/// How long a Git command that changes the repository, its hooks included, may run inside the
/// service before it is stopped; stopping takes up to [`git::STOP_GRACE`] more.
pub const GIT_CHANGE_DEADLINE: Duration = Duration::from_secs(60);
/// The service exits after this long without a request.
pub const IDLE_EXIT: Duration = Duration::from_secs(10 * 60);
/// How long the service may take to answer `operation`, from the moment it reads the request.
#[must_use]
pub fn answer_time(operation: &WorkspaceFilesOperation) -> Duration {
    if operation.runs_git_write() {
        GIT_CHANGE_DEADLINE.saturating_add(git::STOP_GRACE)
    } else if operation.writes() {
        WRITE_DEADLINE
    } else {
        OPERATION_DEADLINE
    }
}
/// Room the result's identity fields take beside the outcome.
const ENVELOPE_BYTES: usize = 4 * 1024;
/// One request line's bytes: the largest request, a save, and the root beside it.
const MAXIMUM_REQUEST_LINE_BYTES: u64 =
    MAXIMUM_WORKSPACE_FILES_WRITE_REQUEST_BYTES as u64 + 8 * 1024;

/// One request line.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: u64,
    /// The Workspace directory, absolute.
    pub root: String,
    pub operation: WorkspaceFilesOperation,
}

/// One answer line.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub id: u64,
    pub outcome: Outcome,
}

/// Answers one operation beneath `root`. The outcome always validates and fits the result cap.
#[must_use]
pub fn answer(
    root: &str,
    operation: &WorkspaceFilesOperation,
    cache: &BlobCache,
    deadline: Instant,
) -> Outcome {
    let outcome = if operation.validate().is_err() {
        Outcome::Failed {
            reason: Failure::Malformed,
        }
    } else {
        match files::Root::open(root) {
            Err(reason) => Outcome::Failed { reason },
            Ok(root) => match operation {
                WorkspaceFilesOperation::ListDirectory { path } => {
                    files::list_directory(&root, path, deadline)
                }
                WorkspaceFilesOperation::ReadFile {
                    path,
                    source,
                    offset,
                    encoding,
                } => {
                    let read = files::Read {
                        offset: *offset,
                        encoding: *encoding,
                    };
                    match source {
                        WorkspaceFileSource::Worktree => files::read_worktree(&root, path, read),
                        WorkspaceFileSource::Index | WorkspaceFileSource::Head => {
                            status::read_blob(&root, path, *source, read, cache, deadline)
                        }
                    }
                }
                WorkspaceFilesOperation::GitStatus {} => status::git_status(&root, deadline),
                WorkspaceFilesOperation::WriteFile {
                    path,
                    text,
                    expected,
                } => write::write_worktree(&root, path, text, expected.as_deref()),
                WorkspaceFilesOperation::GitStage { paths } => {
                    change::stage(&root, paths.as_deref(), deadline)
                }
                WorkspaceFilesOperation::GitUnstage { paths } => {
                    change::unstage(&root, paths.as_deref(), deadline)
                }
                WorkspaceFilesOperation::GitDiscard { paths } => {
                    change::discard(&root, paths, deadline)
                }
                WorkspaceFilesOperation::GitCommit {
                    message,
                    amend,
                    stage_all,
                } => change::commit(&root, message, *amend, *stage_all, deadline),
                // The Server answers a watch itself from what the watch lane reports.
                WorkspaceFilesOperation::Watch { .. } => Outcome::Failed {
                    reason: Failure::Malformed,
                },
            },
        }
    };
    bounded(outcome)
}

/// An outcome that breaks its record or would not fit the result cap answers as malformed. A
/// chunk escapes to at most six times its size, far inside the cap.
fn bounded(outcome: Outcome) -> Outcome {
    let fits = serde_json::to_vec(&outcome).is_ok_and(|encoded| {
        encoded.len() + ENVELOPE_BYTES <= MAXIMUM_WORKSPACE_FILES_RESULT_BYTES
    });
    if fits && outcome.validate().is_ok() {
        outcome
    } else {
        Outcome::Failed {
            reason: Failure::Malformed,
        }
    }
}

/// Answers request lines from `input` in order until it ends. A line that is not a request is
/// skipped, since it names no identity to answer. `activity` is odd while a line is handled.
pub fn serve(input: impl BufRead, mut output: impl Write, activity: &AtomicU64) {
    let mut input = input;
    let cache = BlobCache::default();
    loop {
        let mut line = Vec::new();
        match (&mut input)
            .take(MAXIMUM_REQUEST_LINE_BYTES)
            .read_until(b'\n', &mut line)
        {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        activity.fetch_add(1, Ordering::Relaxed);
        let open = handle(&mut input, &mut output, &line, &cache);
        activity.fetch_add(1, Ordering::Relaxed);
        if !open {
            return;
        }
    }
}

/// Handles one line; false once input or output has closed.
fn handle(
    input: &mut impl BufRead,
    output: &mut impl Write,
    line: &[u8],
    cache: &BlobCache,
) -> bool {
    if line.last() != Some(&b'\n') {
        // An overlong line: drop the rest of it.
        let mut rest = Vec::new();
        return input
            .read_until(b'\n', &mut rest)
            .is_ok_and(|read| read > 0);
    }
    let Ok(request) = serde_json::from_slice::<Request>(line) else {
        return true;
    };
    let deadline = Instant::now()
        + if request.operation.runs_git_write() {
            GIT_CHANGE_DEADLINE
        } else {
            OPERATION_DEADLINE
        };
    let outcome = answer(&request.root, &request.operation, cache, deadline);
    let Ok(mut encoded) = serde_json::to_vec(&Answer {
        id: request.id,
        outcome,
    }) else {
        return true;
    };
    encoded.push(b'\n');
    output
        .write_all(&encoded)
        .and_then(|()| output.flush())
        .is_ok()
}

/// The service process: serves standard input until it closes, or exits after [`IDLE_EXIT`]
/// without a request.
pub fn serve_stdio() {
    let activity = Arc::new(AtomicU64::new(0));
    let watched = Arc::clone(&activity);
    std::thread::spawn(move || {
        let mut seen = watched.load(Ordering::Relaxed);
        let mut quiet_since = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let now = watched.load(Ordering::Relaxed);
            // An odd count is a request still being answered.
            if now != seen || now % 2 == 1 {
                seen = now;
                quiet_since = Instant::now();
            } else if quiet_since.elapsed() >= IDLE_EXIT {
                std::process::exit(0);
            }
        }
    });
    serve(std::io::stdin().lock(), std::io::stdout().lock(), &activity);
}

#[cfg(test)]
mod tests;
