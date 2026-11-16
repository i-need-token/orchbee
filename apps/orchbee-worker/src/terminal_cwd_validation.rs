//! Checks the requested Worker incarnation before probing whether a literal launch directory
//! exists as a folder the account can open for listing, and returns a typed outcome that names
//! why it cannot be used. A successful probe neither reserves the path nor grants terminal
//! creation authority.
//!
//! The probe runs on a helper thread under a short deadline: a stalled filesystem, such as an
//! unreachable network share, must not keep the serving thread from reading the Terminal Host,
//! which drops a Worker it cannot write to for five seconds.

use std::{fs, io::ErrorKind, num::NonZeroU64, path::Path, time::Duration};

use orchbee_capability_node_protocol::{
    TerminalCwdValidationOutcome, TerminalCwdValidationRequest, TerminalCwdValidationResult,
};
use orchbee_local_worker_protocol::WorkerId;

use crate::bounded_helper::HelperPool;

/// A probe that has not answered by then is answered as inaccessible and its late result dropped.
const PROBE_DEADLINE: Duration = Duration::from_secs(2);
/// Probes still running, stalled ones included; at four, a new request is answered as inaccessible
/// without starting another.
static PROBES: HelperPool = HelperPool::new(4);

pub(crate) fn validate(
    request: TerminalCwdValidationRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
) -> TerminalCwdValidationResult {
    let outcome = if request.worker_id.as_bytes() != worker_id.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        TerminalCwdValidationOutcome::WorkerUnavailable
    } else {
        probe(request.cwd.as_str().to_owned())
    };
    TerminalCwdValidationResult { request, outcome }
}

/// Observes `path` on a helper thread and waits at most [`PROBE_DEADLINE`] for it.
fn probe(path: String) -> TerminalCwdValidationOutcome {
    PROBES
        .run("cwd-probe", PROBE_DEADLINE, move || {
            observe(Path::new(&path))
        })
        .unwrap_or(TerminalCwdValidationOutcome::DirectoryInaccessible)
}

fn observe(path: &Path) -> TerminalCwdValidationOutcome {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => listing_outcome(path),
        Ok(_) => TerminalCwdValidationOutcome::NotADirectory,
        // A file standing for a parent component means the named directory cannot exist.
        Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            TerminalCwdValidationOutcome::DirectoryMissing
        }
        Err(_) => TerminalCwdValidationOutcome::DirectoryInaccessible,
    }
}

/// A folder is usable only when the account can open it for listing. The open is the access
/// check; entries are never read.
fn listing_outcome(path: &Path) -> TerminalCwdValidationOutcome {
    match fs::read_dir(path) {
        Ok(_) => TerminalCwdValidationOutcome::DirectoryAvailable,
        Err(error) => match error.kind() {
            ErrorKind::NotFound => TerminalCwdValidationOutcome::DirectoryMissing,
            ErrorKind::NotADirectory => TerminalCwdValidationOutcome::NotADirectory,
            // PermissionDenied and every other failure: the folder cannot be opened for listing.
            _ => TerminalCwdValidationOutcome::DirectoryInaccessible,
        },
    }
}

#[cfg(test)]
mod tests;
