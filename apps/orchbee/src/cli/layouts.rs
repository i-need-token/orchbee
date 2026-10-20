//! Windows reusable-layout workflows. Canonical membership and Client view keep separate owners.
mod apply;
mod export;
mod launch;
mod model;
pub(in crate::cli) mod structure;
mod template;
mod workflow;
use super::{
    Failure, FailureKind, Output, failure_output, output,
    source::{ArmedSignals, Finish},
};
use crate::{
    invocation::{LayoutExportCommand, OutputFormat},
    localization::Locale,
};
/// Create or replace a Windows layout with bounded recovery and retained effect receipts.
pub(crate) use apply::run as apply;
use serde_json::{Map, json};
use std::time::{Duration, Instant};

fn failure(kind: FailureKind, reason: &'static str) -> Failure {
    Failure::new(kind)
        .failure_schema("orchbee.cli.failure.layout")
        .details(Map::from_iter([("reason".into(), json!(reason))]))
}
pub(crate) fn export(
    command: &LayoutExportCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let deadline = Instant::now() + Duration::from_millis(u64::from(command.timeout_ms));
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut result = export::execute(command, deadline, &mut signals).map_err(classify);
    let mut finalization = None;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            result = Err(failure(FailureKind::Cancelled, "layout-unavailable"));
        }
        Err(_) => result = Err(Failure::new(FailureKind::InternalFailure)),
    }
    let mut answer = match result {
        Ok(document) => match output::encode_document(&document) {
            Ok(stdout) if stdout.len() <= 1024 * 1024 => Output {
                status: 0,
                stdout,
                stderr: String::new(),
                finalization: None,
            },
            _ => failure_output(
                Failure::new(FailureKind::OutputTooLarge),
                Some(format),
                locale,
            ),
        },
        Err(error) => failure_output(error, Some(format), locale),
    };
    answer.finalization = finalization;
    answer
}

// These workflow-wide outcomes do not carry the generic wait/source failure details.
fn classify(error: Failure) -> Failure {
    match error.kind {
        FailureKind::Timeout | FailureKind::Cancelled => failure(error.kind, "layout-unavailable"),
        _ => error,
    }
}

#[cfg(test)]
mod tests;
