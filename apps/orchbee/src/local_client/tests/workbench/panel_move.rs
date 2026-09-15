use super::*;
use crate::local_client::workbench::panel_move::{
    WorkbenchPanelMoveCommit, WorkbenchPanelMoveNotSubmitted,
    WorkbenchPanelMoveOutcomeUnknownReason, WorkbenchPanelMovePrecommit, WorkbenchPanelMoveRequest,
    WorkbenchPanelMoveSubmission,
};
use orchbee_workbench_protocol::WorkbenchPanelSplitDirection;

fn panel_move_command(base: u64) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        base,
        WorkbenchAction::PanelMove {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            target_panel_id: WorkbenchPanelId::parse(&id(33)).unwrap(),
            direction: WorkbenchPanelSplitDirection::Right,
        },
    )
}

fn moved(
    revision: u64,
    context: u128,
    tab: u128,
    panel: u128,
    target: u128,
) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelMoved {
        revision: WorkbenchRevision::new(revision),
        terminal_context_id: context_id(context),
        tab_id: WorkbenchTabId::parse(&id(tab)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(panel)).unwrap(),
        target_panel_id: WorkbenchPanelId::parse(&id(target)).unwrap(),
    }
}

#[test]
fn opaque_panel_move_request_rejects_other_actions_unchanged() {
    for direction in [
        WorkbenchPanelSplitDirection::Above,
        WorkbenchPanelSplitDirection::Below,
        WorkbenchPanelSplitDirection::Left,
        WorkbenchPanelSplitDirection::Right,
    ] {
        let mut command = panel_move_command(7);
        let WorkbenchAction::PanelMove {
            direction: actual, ..
        } = &mut command.action
        else {
            unreachable!()
        };
        *actual = direction;
        assert!(WorkbenchPanelMoveRequest::try_from(command).is_ok());
    }
    let command = tab_create_command(FIRST_REQUEST_ID, 7);
    assert_eq!(
        WorkbenchPanelMoveRequest::try_from(command.clone()).unwrap_err(),
        command
    );
}

#[test]
fn panel_move_preserves_exact_commit_noop_and_compatible_precommit() -> TestResult {
    let mut outcomes = vec![
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(7),
        },
        moved(8, 30, 31, 32, 33),
        WorkbenchCommandOutcome::Denied {},
    ];
    for reason in [
        WorkbenchRejectionReason::SiteMismatch,
        WorkbenchRejectionReason::RevisionMismatch,
        WorkbenchRejectionReason::TerminalContextUnavailable,
        WorkbenchRejectionReason::TabUnavailable,
        WorkbenchRejectionReason::PanelUnavailable,
        WorkbenchRejectionReason::ResourceLimit,
        WorkbenchRejectionReason::RevisionExhausted,
        WorkbenchRejectionReason::IdentityExhausted,
        WorkbenchRejectionReason::RequestIdentityConflict,
    ] {
        outcomes.push(WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(99),
            reason,
        });
    }
    for reason in [
        WorkbenchCommandUnavailableReason::RequestLedgerFull,
        WorkbenchCommandUnavailableReason::ServiceUnavailable,
        WorkbenchCommandUnavailableReason::TerminalBusy,
    ] {
        outcomes.push(WorkbenchCommandOutcome::Unavailable { reason });
    }
    for outcome in outcomes {
        let server_outcome = outcome.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 51)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(panel_move_command(7))
            );
            write_workbench_response(
                &mut stream,
                &command_response(FIRST_REQUEST_ID, server_outcome),
            )
            .map_err(io::Error::other)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::SnapshotRead(snapshot_request(SECOND_REQUEST_ID))
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(snapshot_result(SECOND_REQUEST_ID)),
            )
            .map_err(io::Error::other)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let actual = client.submit_workbench_panel_move(
            WorkbenchPanelMoveRequest::try_from(panel_move_command(7)).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        match (outcome, actual) {
            (
                WorkbenchCommandOutcome::Unchanged { revision: expected },
                WorkbenchPanelMoveSubmission::Unchanged { revision },
            ) => assert_eq!(revision, expected),
            (
                WorkbenchCommandOutcome::PanelMoved {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    target_panel_id,
                },
                WorkbenchPanelMoveSubmission::Committed(actual),
            ) => assert_eq!(
                actual,
                WorkbenchPanelMoveCommit {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    target_panel_id,
                }
            ),
            (
                WorkbenchCommandOutcome::Denied {},
                WorkbenchPanelMoveSubmission::DefinitePrecommit(
                    WorkbenchPanelMovePrecommit::Denied,
                ),
            ) => {}
            (
                WorkbenchCommandOutcome::Rejected { revision, reason },
                WorkbenchPanelMoveSubmission::DefinitePrecommit(
                    WorkbenchPanelMovePrecommit::Rejected {
                        revision: actual,
                        reason: actual_reason,
                    },
                ),
            ) => assert_eq!((actual, actual_reason), (revision, reason)),
            (
                WorkbenchCommandOutcome::Unavailable { reason },
                WorkbenchPanelMoveSubmission::DefinitePrecommit(
                    WorkbenchPanelMovePrecommit::Unavailable { reason: actual },
                ),
            ) => assert_eq!(actual, reason),
            other => panic!("unexpected panel.move classification {other:?}"),
        }
        assert_eq!(client.generation().map(NonZeroU64::get), Some(51));
        assert_eq!(
            client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
            snapshot_result(SECOND_REQUEST_ID)
        );
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn panel_move_retires_impossible_revisions_identities_and_action_results() -> TestResult {
    for (baseline, outcome) in [
        (7, moved(7, 30, 31, 32, 33)),
        (7, moved(9, 30, 31, 32, 33)),
        (u64::MAX, moved(u64::MAX, 30, 31, 32, 33)),
        (7, moved(8, 99, 31, 32, 33)),
        (7, moved(8, 30, 99, 32, 33)),
        (7, moved(8, 30, 31, 99, 33)),
        (7, moved(8, 30, 31, 32, 99)),
        (
            7,
            WorkbenchCommandOutcome::Unchanged {
                revision: WorkbenchRevision::new(8),
            },
        ),
        (
            7,
            WorkbenchCommandOutcome::Renamed {
                revision: WorkbenchRevision::new(8),
            },
        ),
        (
            7,
            WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(7),
                reason: WorkbenchRejectionReason::ProjectUnavailable,
            },
        ),
        (
            7,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            },
        ),
    ] {
        let result = invalid_move(baseline, command_response(FIRST_REQUEST_ID, outcome))?;
        assert!(
            matches!(result, WorkbenchPanelMoveSubmission::OutcomeUnknown {request_id,..} if request_id.as_str()==FIRST_REQUEST_ID)
        );
    }
    Ok(())
}

#[test]
fn panel_move_retires_wrong_kind_and_correlation_without_replay() -> TestResult {
    assert!(matches!(
        invalid_move(
            7,
            WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID))
        )?,
        WorkbenchPanelMoveSubmission::OutcomeUnknown {
            reason: WorkbenchPanelMoveOutcomeUnknownReason::UnexpectedResponse,
            ..
        }
    ));
    assert!(
        matches!(invalid_move(7, command_response(SECOND_REQUEST_ID,moved(8,30,31,32,33)))?, WorkbenchPanelMoveSubmission::OutcomeUnknown {reason: WorkbenchPanelMoveOutcomeUnknownReason::CorrelationMismatch {expected,received},..} if expected.as_str()==FIRST_REQUEST_ID && received.as_str()==SECOND_REQUEST_ID)
    );
    Ok(())
}

#[test]
fn panel_move_prewrite_expiry_preserves_carrier_and_sends_no_frame() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 52)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(snapshot_request(SECOND_REQUEST_ID))
        );
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(snapshot_result(SECOND_REQUEST_ID)),
        )
        .map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let deadline = client.operation_clock().now();
    assert!(
        matches!(client.submit_workbench_panel_move(WorkbenchPanelMoveRequest::try_from(panel_move_command(7)).unwrap(),deadline),WorkbenchPanelMoveSubmission::DefinitelyNotSubmitted(WorkbenchPanelMoveNotSubmitted::RequestNotSubmitted(error)) if error.kind()==io::ErrorKind::TimedOut)
    );
    assert_eq!(
        client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
        snapshot_result(SECOND_REQUEST_ID)
    );
    assert_eq!(client.generation().map(NonZeroU64::get), Some(52));
    drop(client);
    server.finish()?;
    Ok(())
}

fn invalid_move(
    baseline: u64,
    response: WorkbenchResponse,
) -> Result<WorkbenchPanelMoveSubmission, Box<dyn std::error::Error>> {
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 54)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(panel_move_command(baseline))
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let result = client.submit_workbench_panel_move(
        WorkbenchPanelMoveRequest::try_from(panel_move_command(baseline)).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(result)
}
