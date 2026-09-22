//! A caller's environment identifies a product terminal, never its location or
//! permissions. The single validated snapshot owns the current binding.

use std::ffi::OsString;

use orchbee_workbench_protocol::{Snapshot, TerminalId, WorkerId};
use serde_json::Value;

use super::{Failure, FailureKind, results, stage_details, target_details};
use crate::invocation::{QueryCommand, Resource, Target, UsageReason};

struct Caller {
    terminal: TerminalId,
    worker: WorkerId,
}

impl Caller {
    fn read(mut environment: impl FnMut(&str) -> Option<OsString>) -> Result<Self, Failure> {
        let terminal = environment("ORCHBEE_TERMINAL_ID").ok_or_else(invalid_context)?;
        let worker = environment("ORCHBEE_WORKER_ID").ok_or_else(invalid_context)?;
        Ok(Self {
            terminal: TerminalId::parse(terminal.to_str().ok_or_else(invalid_context)?)
                .map_err(|_| invalid_context())?,
            worker: WorkerId::parse(worker.to_str().ok_or_else(invalid_context)?)
                .map_err(|_| invalid_context())?,
        })
    }

    fn panel(&self, snapshot: &Snapshot) -> Result<Target, Failure> {
        let rows = results::terminal_rows(snapshot);
        let mut matches = rows
            .iter()
            .filter(|row| row.terminal_id == self.terminal.as_str());
        let first = matches.next();
        if matches.next().is_some() {
            return Err(
                Failure::new(FailureKind::ProtocolInvalid).details(stage_details("snapshot"))
            );
        }
        match first {
            Some(row) if row.worker_id == self.worker.as_str() => {
                Ok(Target::Id(row.panel_id.clone()))
            }
            _ => Err(Failure::new(FailureKind::NotFound)
                .details(target_details(Resource::Terminal, self.terminal.as_str()))),
        }
    }
}

fn invalid_context() -> Failure {
    let mut failure = Failure::new(FailureKind::InvalidUsage);
    failure.usage = Some(Box::new(UsageReason::CallerContextUnavailable));
    failure
}

pub(super) fn read(
    explicit: Option<&Target>,
    environment: impl FnMut(&str) -> Option<OsString>,
    snapshot: impl FnOnce() -> Result<Snapshot, Failure>,
) -> Result<(Value, results::Projection), Failure> {
    // Explicit mode never consults caller state. Missing/invalid caller state
    // fails before the runtime capability can be invoked.
    let caller = explicit
        .is_none()
        .then(|| Caller::read(environment))
        .transpose()?;
    let snapshot = snapshot()?;
    let source = results::snapshot_source(&snapshot);
    let target = match (explicit, caller) {
        (Some(target), _) => target.clone(),
        (None, Some(caller)) => caller
            .panel(&snapshot)
            .map_err(|error| error.source(source.clone()))?,
        (None, None) => return Err(Failure::new(FailureKind::InternalFailure)),
    };
    results::project(&QueryCommand::PanelGet(target), &snapshot)
        .map(|projection| (source.clone(), projection))
        .map_err(|error| error.source(source))
}

#[cfg(test)]
mod tests;
