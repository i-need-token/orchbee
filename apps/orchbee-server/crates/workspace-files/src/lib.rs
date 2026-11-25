//! The read-only Workspace file service behind `workbench.workspace-files`. The Server runs it in a
//! child process of its own executable only once a Client asks, and talks to it over standard
//! input and output, one JSON object per line; see this crate's README.
mod files;
mod git;
mod status;

use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILES_RESULT_BYTES, WorkspaceFileContent, WorkspaceFileSource,
    WorkspaceFilesFailure as Failure, WorkspaceFilesOperation, WorkspaceFilesOutcome as Outcome,
};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The command-line flag that turns the Server's executable into this service.
pub const SERVICE_FLAG: &str = "--workspace-files";
/// How long one request may run inside the service; the Server waits a little longer.
pub const OPERATION_DEADLINE: Duration = Duration::from_secs(4);
/// The service exits after this long without a request.
pub const IDLE_EXIT: Duration = Duration::from_secs(10 * 60);
/// Room the result's identity fields take beside the outcome.
const ENVELOPE_BYTES: usize = 4 * 1024;
/// One request line's bytes; a request is one relative path, a root and a closed operation.
const MAXIMUM_REQUEST_LINE_BYTES: u64 = 64 * 1024;

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
pub fn answer(root: &str, operation: &WorkspaceFilesOperation, deadline: Instant) -> Outcome {
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
                    source: WorkspaceFileSource::Worktree,
                } => files::read_worktree(&root, path),
                WorkspaceFilesOperation::ReadFile { path, source } => {
                    status::read_blob(&root, path, *source, deadline)
                }
                WorkspaceFilesOperation::GitStatus {} => status::git_status(&root, deadline),
            },
        }
    };
    bounded(outcome)
}

/// An outcome that breaks its record or would not fit the result cap answers as far as it can:
/// text that escapes past the cap is too large, anything else malformed.
fn bounded(outcome: Outcome) -> Outcome {
    let fits = serde_json::to_vec(&outcome).is_ok_and(|encoded| {
        encoded.len() + ENVELOPE_BYTES <= MAXIMUM_WORKSPACE_FILES_RESULT_BYTES
    });
    match outcome {
        _ if fits && outcome.validate().is_ok() => outcome,
        Outcome::File {
            byte_length,
            content: WorkspaceFileContent::Text { .. },
        } if outcome.validate().is_ok() => Outcome::File {
            byte_length,
            content: WorkspaceFileContent::TooLarge {},
        },
        _ => Outcome::Failed {
            reason: Failure::Malformed,
        },
    }
}

/// Answers request lines from `input` in order until it ends. A line that is not a request is
/// skipped, since it names no identity to answer. `activity` is odd while a line is handled.
pub fn serve(input: impl BufRead, mut output: impl Write, activity: &AtomicU64) {
    let mut input = input;
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
        let open = handle(&mut input, &mut output, &line);
        activity.fetch_add(1, Ordering::Relaxed);
        if !open {
            return;
        }
    }
}

/// Handles one line; false once input or output has closed.
fn handle(input: &mut impl BufRead, output: &mut impl Write, line: &[u8]) -> bool {
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
    let outcome = answer(
        &request.root,
        &request.operation,
        Instant::now() + OPERATION_DEADLINE,
    );
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
