use std::{fs, num::NonZeroU64, path::Path};

use orchbee_capability_node_protocol::{
    TerminalCwdValidationOutcome, TerminalCwdValidationRequest, TerminalCwdValidationResult,
};
use orchbee_local_site_protocol::WorkerId;

pub(crate) fn validate(
    request: TerminalCwdValidationRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
) -> TerminalCwdValidationResult {
    let outcome = if request.worker_id.as_bytes() != worker_id.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        TerminalCwdValidationOutcome::WorkerUnavailable
    } else if fs::metadata(Path::new(request.cwd.as_str())).is_ok_and(|metadata| metadata.is_dir())
    {
        TerminalCwdValidationOutcome::DirectoryAvailable
    } else {
        TerminalCwdValidationOutcome::DirectoryUnavailable
    };
    TerminalCwdValidationResult { request, outcome }
}

#[cfg(test)]
mod tests;
