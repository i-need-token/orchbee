//! Generation-fenced editor preference reads do not require a Host or grant launch authority.
use orchbee_capability_node_protocol::{
    TerminalEditorCommandOutcome as Outcome, TerminalEditorCommandRefusal as Refusal,
    TerminalEditorCommandRequest as Request, TerminalEditorCommandResult as Result,
};
use orchbee_local_site_protocol::WorkerId;
use std::num::NonZeroU64;

#[cfg(windows)]
mod windows;

/// Refuses a wrong incarnation before consulting machine editor preferences.
pub(crate) fn execute(request: Request, worker: WorkerId, generation: NonZeroU64) -> Result {
    execute_with(request, worker, generation, || {
        #[cfg(windows)]
        {
            windows::resolve().map(|(source, selection)| Outcome::Resolved { source, selection })
        }
        #[cfg(not(windows))]
        {
            Err(Refusal::UnsupportedPlatform)
        }
    })
}

fn execute_with(
    request: Request,
    worker: WorkerId,
    generation: NonZeroU64,
    resolve: impl FnOnce() -> std::result::Result<Outcome, Refusal>,
) -> Result {
    let outcome = if request.worker_id.as_bytes() != worker.as_bytes() {
        Outcome::Refused(Refusal::WrongWorker)
    } else if request.worker_generation.get() != generation.get() {
        Outcome::Refused(Refusal::StaleWorkerGeneration)
    } else {
        resolve().unwrap_or_else(Outcome::Refused)
    };
    Result { request, outcome }
}

#[cfg(test)]
mod tests;
