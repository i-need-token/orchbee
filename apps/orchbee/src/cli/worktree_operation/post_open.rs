//! The invocation owns this second commit boundary; canonical results always survive it.
#[cfg(any(windows, test))]
use super::Observation;
use crate::cli::FailureKind;
use orchbee_workbench_protocol::WorkbenchClientId;
#[cfg(any(windows, test))]
use orchbee_workbench_protocol::{
    WorkbenchProjectId, WorkbenchTerminalOutcome, WorkbenchWorkspaceId, WorkbenchWorktreeOutcome,
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
    #[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
pub(super) fn opened(
    observation: &Observation,
) -> Option<(&WorkbenchProjectId, &WorkbenchWorkspaceId)> {
    let Observation::Server { reply } = observation else {
        return None;
    };
    match &reply.outcome {
        WorkbenchWorktreeOutcome::Available {
            project_id, status, ..
        } => match status.as_ref() {
            WorktreeOperationStatus::Opened {
                workspace_id,
                terminal_outcome: None | Some(WorkbenchTerminalOutcome::Started { .. }),
                ..
            } => Some((project_id, workspace_id)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::cli::workspace_focus::unchanged;
    use orchbee_workbench_protocol::ClientViewRecord;
    #[test]
    fn revision_and_derived_count_refresh_do_not_override_navigation_guard() {
        let mut original: ClientViewRecord = serde_json::from_value(json!({"clientId":"00000000000000000000000000000001","heartbeat":"1","viewRevision":"1","availability":"current","view":{"phase":"ready","workbenchRevision":"2","selection":{"terminalContextId":"00000000000000000000000000000002","tabId":"00000000000000000000000000000004","panelId":"00000000000000000000000000000005","zoomed":false,"panelCount":1}}})).unwrap();
        let mut fresh = original.clone();
        fresh.view_revision = serde_json::from_value(json!("3")).unwrap();
        fresh.view.workbench_revision = orchbee_workbench_protocol::WorkbenchRevision::new(3);
        fresh.view.selection.as_mut().unwrap().panel_count = 2;
        assert!(unchanged(&original, &fresh));
        fresh.view.selection.as_mut().unwrap().zoomed = true;
        assert!(!unchanged(&original, &fresh));
        fresh.view.selection.as_mut().unwrap().zoomed = false;
        original
            .view
            .selection
            .as_mut()
            .unwrap()
            .terminal_context_id = orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
            "00000000000000000000000000000003",
        )
        .unwrap();
        assert!(!unchanged(&original, &fresh));
    }
}
