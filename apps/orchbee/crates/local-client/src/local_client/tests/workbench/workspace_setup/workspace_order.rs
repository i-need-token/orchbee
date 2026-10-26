use super::*;
use orchbee_workbench_protocol::{WorkbenchWorkspaceInsertIndex, WorkbenchWorkspaceList};

fn ids(values: &[u128]) -> WorkbenchWorkspaceList {
    WorkbenchWorkspaceList::new(
        values
            .iter()
            .map(|value| WorkbenchWorkspaceId::parse(&id(*value)).unwrap())
            .collect(),
    )
    .unwrap()
}

#[test]
fn workspace_order_receipts_preserve_known_effects_and_retire_bad_results() -> TestResult {
    for action in [
        WorkbenchAction::WorkspaceMove {
            workspace_id: WorkbenchWorkspaceId::parse(&id(21))?,
            insert_index: WorkbenchWorkspaceInsertIndex::new(0)?,
        },
        WorkbenchAction::WorkspaceMoveBlock {
            workspace_ids: ids(&[21, 22]),
            before_workspace_id: Some(WorkbenchWorkspaceId::parse(&id(23))?),
        },
    ] {
        let command = workbench_command(FIRST_REQUEST_ID, 7, action);
        let order = ids(&[21, 22, 23]);
        let committed = WorkbenchCommandOutcome::WorkspacesReordered {
            revision: WorkbenchRevision::new(8),
            workspace_order: order.clone(),
        };
        let actual = exchange_setup(
            command.clone(),
            command_response(FIRST_REQUEST_ID, committed.clone()),
            true,
        )?;
        assert!(
            matches!(actual,WorkbenchSetupSubmission::Committed(WorkbenchSetupCommit::WorkspaceOrder { workspace_order:Some(received), .. }) if received == order)
        );
        let actual = exchange_setup(
            command.clone(),
            command_response(
                FIRST_REQUEST_ID,
                WorkbenchCommandOutcome::Unchanged {
                    revision: WorkbenchRevision::new(7),
                },
            ),
            true,
        )?;
        assert!(matches!(
            actual,
            WorkbenchSetupSubmission::Committed(WorkbenchSetupCommit::WorkspaceOrder {
                workspace_order: None,
                ..
            })
        ));
        for outcome in [
            WorkbenchCommandOutcome::Unchanged {
                revision: WorkbenchRevision::new(8),
            },
            WorkbenchCommandOutcome::WorkspacesReordered {
                revision: WorkbenchRevision::new(7),
                workspace_order: order.clone(),
            },
            WorkbenchCommandOutcome::WorkspacesReordered {
                revision: WorkbenchRevision::new(8),
                workspace_order: ids(&[22, 23, 21]),
            },
            WorkbenchCommandOutcome::Renamed {
                revision: WorkbenchRevision::new(8),
            },
        ] {
            assert!(matches!(
                exchange_setup(
                    command.clone(),
                    command_response(FIRST_REQUEST_ID, outcome),
                    false
                )?,
                WorkbenchSetupSubmission::OutcomeUnknown { .. }
            ));
        }
        assert!(matches!(
            exchange_setup(
                command,
                command_response(SECOND_REQUEST_ID, committed),
                false
            )?,
            WorkbenchSetupSubmission::OutcomeUnknown {
                reason: WorkbenchSetupOutcomeUnknownReason::CorrelationMismatch { .. },
                ..
            }
        ));
    }
    Ok(())
}
