//! Native Client discovery and one-shot control submission with retained status reads.
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::cli_text::CliText;
use crate::{
    invocation::{ClientCommand, OutputFormat},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientViewControlStatus, ClientViewOutcome, SessionId, WorkbenchClientViewResult,
    WorkbenchRequestId,
};
use serde_json::json;

pub(super) mod exchange;
mod layout;
pub(super) mod selection;
mod swap;
pub(super) mod view_fence;
mod workflow;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Wait {
    Detached,
    TimedOut,
    Cancelled,
    TransportLost,
    ObservationFailed,
}
pub(super) struct Answer {
    status_only: bool,
    operation_id: Option<WorkbenchRequestId>,
    session_id: Option<SessionId>,
    pub(super) reply: Option<WorkbenchClientViewResult>,
    unknown: bool,
    wait: Option<Wait>,
}
impl Answer {
    pub(super) fn retain_matching_command(
        &mut self,
        command: &orchbee_workbench_protocol::ClientViewControl,
    ) {
        if matches!(&self.reply,Some(reply) if matches!(&reply.outcome,ClientViewOutcome::Operation { operation } if &operation.command!=command))
        {
            self.reply = None;
            self.unknown = true;
        }
    }
    /// A control the Server or Client refused before any action because the view it named had
    /// gone stale. An unknown, incomplete, still pending or unobserved outcome never qualifies.
    pub(super) fn stale_refusal(&self) -> bool {
        !self.unknown
            && self.wait.is_none()
            && self.reply.as_ref().is_some_and(|reply| {
                matches!(&reply.outcome, ClientViewOutcome::Operation { operation }
                if operation.status == ClientViewControlStatus::Rejected {
                    reason: orchbee_workbench_protocol::ClientViewControlReason::StaleView,
                })
            })
    }
    fn pending(&self) -> bool {
        self.reply.as_ref().is_some_and(|reply| {
            matches!(&reply.outcome,
            ClientViewOutcome::Operation { operation } if operation.status.pending())
        })
    }
    pub(super) fn document(&self) -> serde_json::Value {
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
        json!({"schema":"orchbee.client.control","version":1,"result":{"sessionId":self.session_id,
            "operationId":self.operation_id,"observation":observation,"wait":wait}})
    }
    pub(super) fn exit_code(&self) -> u8 {
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
fn outcome_exit(outcome: &ClientViewOutcome) -> u8 {
    match outcome {
        ClientViewOutcome::Listed { .. } | ClientViewOutcome::Found { .. } => 0,
        ClientViewOutcome::Operation { operation } => match &operation.status {
            ClientViewControlStatus::PopupOpened { .. }
            | ClientViewControlStatus::PopupClosed { .. }
            | ClientViewControlStatus::Queued {}
            | ClientViewControlStatus::Delivered {}
            | ClientViewControlStatus::LayoutAdjusted { .. }
            | ClientViewControlStatus::DirectionFocused { .. }
            | ClientViewControlStatus::DirectionUnchanged { .. }
            | ClientViewControlStatus::LayoutRead { .. }
            | ClientViewControlStatus::AttentionRead { .. }
            | ClientViewControlStatus::TerminalAttentionRead { .. }
            | ClientViewControlStatus::NavigationRead { .. }
            | ClientViewControlStatus::PresentationRead { .. }
            | ClientViewControlStatus::ViewportRead { .. }
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
    if let Err(failure) = super::execution::admit(super::execution::Capability::ClientControl) {
        return failure_output(failure, Some(format), locale);
    }
    workflow::run(command, format, locale)
}

pub(super) fn emit(answer: &Answer, format: OutputFormat, locale: Locale) -> Output {
    let document = if answer.operation_id.is_some() {
        answer.document()
    } else {
        json!({"schema":"orchbee.client.view","version":1,"result":answer.reply})
    };
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => {
            output::encode_text(&CliText::new(locale).cli_client_control(&document))
        }
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
