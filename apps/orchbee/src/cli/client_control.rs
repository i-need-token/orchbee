//! Native Client discovery and one-shot control submission with retained status reads.
#[cfg(any(windows, test))]
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{ClientCommand, OutputFormat},
    localization::Locale,
};
#[cfg(any(windows, test))]
use orchbee_workbench_protocol::{
    ClientViewControlStatus, ClientViewOutcome, SiteId, WorkbenchClientViewResult,
    WorkbenchRequestId,
};
#[cfg(any(windows, test))]
use serde_json::json;

#[cfg(any(windows, test))]
mod layout;
#[cfg(any(windows, test))]
mod selection;
#[cfg(windows)]
mod swap;
#[cfg(windows)]
mod windows;

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Wait {
    Detached,
    TimedOut,
    Cancelled,
    TransportLost,
    ObservationFailed,
}
#[cfg(any(windows, test))]
struct Answer {
    status_only: bool,
    operation_id: Option<WorkbenchRequestId>,
    site_id: Option<SiteId>,
    reply: Option<WorkbenchClientViewResult>,
    unknown: bool,
    wait: Option<Wait>,
}
#[cfg(any(windows, test))]
impl Answer {
    fn pending(&self) -> bool {
        self.reply.as_ref().is_some_and(|reply| {
            matches!(&reply.outcome,
            ClientViewOutcome::Operation { operation } if operation.status.pending())
        })
    }
    fn document(&self) -> serde_json::Value {
        let wait = self.wait.map(|wait| match wait {
            Wait::Detached => "detached",
            Wait::TimedOut => "timed-out",
            Wait::Cancelled => "cancelled",
            Wait::TransportLost => "transport-lost",
            Wait::ObservationFailed => "observation-failed",
        });
        let observation = if self.unknown {
            json!({"kind":"unknown"})
        } else if let Some(reply) = &self.reply {
            json!({"kind":"server","reply":reply})
        } else {
            json!({"kind": if self.status_only { "unobserved" } else { "not-submitted" }})
        };
        json!({"schema":"orchbee.client.control","version":1,"siteId":self.site_id,
            "operationId":self.operation_id,"observation":observation,"wait":wait})
    }
    fn exit_code(&self) -> u8 {
        match self.wait {
            Some(Wait::Cancelled) => return 130,
            Some(Wait::TimedOut) => return 7,
            Some(Wait::TransportLost) => return 5,
            Some(Wait::ObservationFailed) => return 1,
            _ => {}
        }
        if self.unknown {
            return 8;
        }
        self.reply
            .as_ref()
            .map_or(4, |reply| outcome_exit(&reply.outcome))
    }
}
#[cfg(any(windows, test))]
fn outcome_exit(outcome: &ClientViewOutcome) -> u8 {
    match outcome {
        ClientViewOutcome::Listed { .. } | ClientViewOutcome::Found { .. } => 0,
        ClientViewOutcome::Operation { operation } => match &operation.status {
            ClientViewControlStatus::Queued {}
            | ClientViewControlStatus::Delivered {}
            | ClientViewControlStatus::LayoutAdjusted { .. }
            | ClientViewControlStatus::DirectionFocused { .. }
            | ClientViewControlStatus::DirectionUnchanged { .. }
            | ClientViewControlStatus::LayoutRead { .. }
            | ClientViewControlStatus::Applied { .. }
            | ClientViewControlStatus::ZoomUnchanged { .. }
            | ClientViewControlStatus::ZoomToggled { .. } => 0,
            ClientViewControlStatus::Rejected { .. } => 3,
            ClientViewControlStatus::Incomplete { reason, .. } => {
                if *reason
                    == orchbee_workbench_protocol::ClientViewControlReason::ClientDisconnected
                {
                    8
                } else {
                    3
                }
            }
        },
        ClientViewOutcome::Denied {}
        | ClientViewOutcome::Conflict {}
        | ClientViewOutcome::NotFound {} => 3,
        ClientViewOutcome::Unavailable { .. } => 4,
        ClientViewOutcome::Synchronized { .. } => 5,
    }
}
pub(crate) fn run(command: &ClientCommand, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        windows::run(command, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = command;
        failure_output(
            Failure::new(FailureKind::ServiceUnavailable),
            Some(format),
            locale,
        )
    }
}
#[cfg(any(windows, test))]
fn emit(answer: &Answer, format: OutputFormat, locale: Locale) -> Output {
    let document = if answer.operation_id.is_some() {
        answer.document()
    } else {
        json!({"schema":"orchbee.client.view","version":1,"result":answer.reply})
    };
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => output::encode_text(&locale.cli_client_control(&document)),
    };
    match encoded {
        Ok(stdout) => Output {
            status: answer.exit_code(),
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}

#[cfg(test)]
mod tests;
