//! Conditional refusals must never be accepted as confirmation of an ordinary command.
use super::*;
use orchbee_workbench_protocol::{ProtocolVersion, WorkbenchCommandResult};

#[test]
fn confirmation_required_is_definite_only_for_conditional_action_at_its_admitted_base() {
    for conditional in [false, true] {
        for revision in [7, 8] {
            let workspace_id =
                WorkbenchWorkspaceId::parse("00000000000000000000000000000002").unwrap();
            let command = WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse("00000000000000000000000000000003").unwrap(),
                session_id: SessionId::parse("00000000000000000000000000000001").unwrap(),
                expected_revision: WorkbenchRevision::new(7),
                action: if conditional {
                    WorkbenchAction::WorkspaceCloseIfIdle { workspace_id }
                } else {
                    WorkbenchAction::WorkspaceClose { workspace_id }
                },
            };
            assert!(WorkbenchWorkspaceCloseRequest::try_from(command.clone()).is_ok());
            let response = WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id.clone(),
                outcome: WorkbenchCommandOutcome::Rejected {
                    revision: WorkbenchRevision::new(revision),
                    reason: WorkbenchRejectionReason::ConfirmationRequired,
                },
            });
            let result = classify_command_result(&command, response);
            if conditional && revision == 7 {
                assert!(matches!(
                    result,
                    Ok(WorkbenchWorkspaceCloseSubmission::DefinitePrecommit(
                        WorkbenchWorkspaceClosePrecommit::Rejected {
                            reason: WorkbenchRejectionReason::ConfirmationRequired,
                            ..
                        }
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(WorkbenchWorkspaceCloseFailure::ActionIncompatible)
                ));
            }
        }
    }
}
