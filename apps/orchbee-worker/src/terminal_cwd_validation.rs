//! Checks the requested Worker incarnation before probing whether a literal launch directory
//! exists as a directory and returning a typed outcome that names why it cannot be used. A
//! successful probe neither reserves the path nor grants terminal creation authority.

use std::{fs, io::ErrorKind, num::NonZeroU64, path::Path};

use orchbee_capability_node_protocol::{
    TerminalCwdValidationOutcome, TerminalCwdValidationRequest, TerminalCwdValidationResult,
};
use orchbee_local_worker_protocol::WorkerId;

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
        match fs::metadata(Path::new(request.cwd.as_str())) {
            Ok(metadata) if metadata.is_dir() => TerminalCwdValidationOutcome::DirectoryAvailable,
            Ok(_) => TerminalCwdValidationOutcome::NotADirectory,
            // A file standing for a parent component means the named directory cannot exist.
            Err(error)
                if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) =>
            {
                TerminalCwdValidationOutcome::DirectoryMissing
            }
            Err(_) => TerminalCwdValidationOutcome::DirectoryInaccessible,
        }
    };
    TerminalCwdValidationResult { request, outcome }
}

#[cfg(test)]
mod tests;
