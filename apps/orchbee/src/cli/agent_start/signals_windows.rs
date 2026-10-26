//! Owns the existing console interrupt interval around the shared startup workflow.

use crate::{
    agent_start::{Report, Request},
    cli::{
        Failure, FailureKind, SourceFinalization,
        source::{ArmedSignals, Finish, SignalWatch},
    },
};
use orchbee_local_ipc::LocalIo;
use std::io;

pub(super) fn run(
    request: Request,
    workflow: impl FnOnce(Request, Option<LocalIo<'_>>) -> Report,
) -> Result<(Report, io::Result<Option<SourceFinalization>>), (Request, Failure)> {
    let armed = match ArmedSignals::arm_interrupts() {
        Ok(armed) => armed,
        Err(_) => {
            return Err((
                request,
                crate::agent_start::failure(
                    FailureKind::InternalFailure,
                    "signal-registration-failed",
                ),
            ));
        }
    };
    let report = workflow(request, armed.poll_fd());
    let finish = armed.finish().map(|finish| match finish {
        Finish::Clear => None,
        Finish::Cancelled { finalization, .. } => Some(finalization),
    });
    Ok((report, finish))
}
