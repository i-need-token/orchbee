//! The invocation owns this second commit boundary; canonical results always survive it.
use super::Observation;
use crate::cli::FailureKind;
use orchbee_workbench_protocol::WorkbenchClientId;
use orchbee_workbench_protocol::{
    WorkbenchTerminalOutcome, WorkbenchWorkspaceId, WorkbenchWorktreeOutcome,
    WorktreeOperationStatus,
};
use serde_json::{Value, json};

pub(super) struct Receipt {
    pub(super) client_id: Option<WorkbenchClientId>,
    pub(super) focus: Option<FocusReceipt>,
    pub(super) stage: &'static str,
    pub(super) failure: Option<FailureKind>,
}
pub(super) struct FocusReceipt {
    document: Value,
    exit_code: u8,
    applied: bool,
}
impl Receipt {
    pub(super) fn new() -> Self {
        Self {
            client_id: None,
            focus: None,
            stage: "worktree",
            failure: None,
        }
    }
    pub(super) fn complete(&self) -> bool {
        self.failure.is_none()
            && self
                .focus
                .as_ref()
                .is_some_and(|focus| focus.applied && focus.exit_code == 0)
    }
    pub(super) fn exit_code(&self) -> u8 {
        if let Some(failure) = self.failure {
            failure.exit_code()
        } else if self.complete() {
            0
        } else {
            self.focus.as_ref().map_or(3, |focus| {
                if focus.exit_code == 0 {
                    3
                } else {
                    focus.exit_code
                }
            })
        }
    }
    pub(super) fn document(&self) -> Value {
        json!({"clientId":self.client_id,"focus":self.focus.as_ref().map(|focus| &focus.document),"stage":self.stage,"failure":self.failure.map(FailureKind::as_str),"outcome":if self.complete() {"completed"} else {"incomplete"}})
    }
    pub(super) fn record(&mut self, answer: crate::cli::client_control::Answer) {
        use orchbee_workbench_protocol::{ClientViewControlStatus, ClientViewOutcome};
        let applied = matches!(answer.reply.as_ref().map(|reply| &reply.outcome), Some(ClientViewOutcome::Operation {operation}) if matches!(operation.status, ClientViewControlStatus::Applied { .. }));
        self.focus = Some(FocusReceipt {
            document: answer.document(),
            exit_code: answer.exit_code(),
            applied,
        });
        if self.complete() {
            self.stage = "complete";
        }
    }
}

pub(super) fn opened(observation: &Observation) -> Option<&WorkbenchWorkspaceId> {
    let Observation::Server { reply } = observation else {
        return None;
    };
    match &reply.outcome {
        WorkbenchWorktreeOutcome::Available { status, .. } => match status.as_ref() {
            WorktreeOperationStatus::Opened {
                workspace_id,
                terminal_outcome: None | Some(WorkbenchTerminalOutcome::Started { .. }),
                ..
            } => Some(workspace_id),
            // Only a confirmed started terminal can receive this follow-up; other launch facts leave it unresolved.
            _ => None,
        },
        WorkbenchWorktreeOutcome::NotFound { .. }
        | WorkbenchWorktreeOutcome::Conflict { .. }
        | WorkbenchWorktreeOutcome::Denied { .. }
        | WorkbenchWorktreeOutcome::Unavailable { .. } => None,
    }
}

#[cfg(test)]
mod tests;
