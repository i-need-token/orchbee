//! Agent launch has no Unix interrupt interval; refusal installs no signal policy.

use crate::{
    agent_start::{Report, Request},
    cli::{Failure, FailureKind, SourceFinalization},
    tui_capabilities::{Capability, Unsupported},
};
use orchbee_local_ipc::LocalIo;
use std::io;

pub(super) fn run(
    request: Request,
    _workflow: impl FnOnce(Request, Option<LocalIo<'_>>) -> Report,
) -> Result<(Report, io::Result<Option<SourceFinalization>>), (Request, Failure)> {
    Err((request, unsupported(Unsupported(Capability::AgentLaunch))))
}

fn unsupported(_capability: Unsupported) -> Failure {
    Failure::new(FailureKind::ServiceUnavailable).details(crate::cli::stage_reason_details(
        "platform",
        "unsupported-platform",
    ))
}
