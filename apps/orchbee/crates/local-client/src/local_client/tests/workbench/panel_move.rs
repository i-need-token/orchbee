use super::*;
use crate::local_client::workbench::panel_placement::{
    WorkbenchPanelPlacementCommit, WorkbenchPanelPlacementNotSubmitted,
    WorkbenchPanelPlacementOutcomeUnknownReason, WorkbenchPanelPlacementPrecommit,
    WorkbenchPanelPlacementRequest, WorkbenchPanelPlacementSubmission,
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
fn opaque_panel_placement_request_rejects_unrelated_actions_unchanged() {
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
        assert!(WorkbenchPanelPlacementRequest::try_from(command).is_ok());
    }
    let command = tab_create_command(FIRST_REQUEST_ID, 7);
    assert_eq!(
        WorkbenchPanelPlacementRequest::try_from(command.clone()).unwrap_err(),
        command
    );
}

#[test]
fn panel_swap_wake_after_submission_retires_unknown_without_replay() -> TestResult {
    let (submitted, wait_for_submission) = std::sync::mpsc::channel();
    let (wake_read, mut wake_write) = UnixStream::pair()?;
    let server = FakeServer::start(move |mut stream| {
        handshake::expect_hello(&mut stream)?;
        admit(&mut stream, 51)?;
        let WorkbenchRequest::Command(command) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("swap");
        };
        assert!(matches!(command.action, WorkbenchAction::PanelSwap { .. }));
        submitted.send(()).map_err(io::Error::other)?;
        assert_eq!(
            stream.read(&mut [0])?,
            0,
            "wake closes the carrier without replay"
        );
        Ok(())
    })?;
    let wake = std::thread::spawn(move || -> io::Result<()> {
        wait_for_submission
            .recv_timeout(Duration::from_secs(5))
            .map_err(io::Error::other)?;
        wake_write.write_all(b"x")
    });
    let mut client = LocalClient::connect(server.endpoint())?;
    let mut command = panel_move_command(7);
    command.action = WorkbenchAction::PanelSwap {
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
        target_panel_id: WorkbenchPanelId::parse(&id(33)).unwrap(),
    };
    let answer = client.submit_workbench_panel_placement_with_wake(
        WorkbenchPanelPlacementRequest::try_from(command).unwrap(),
        client.operation_clock().now() + Duration::from_secs(5),
        Some(wake_read.as_local_io()),
    );
    assert!(matches!(
        answer,
        WorkbenchPanelPlacementSubmission::OutcomeUnknown { .. }
    ));
    assert!(client.generation().is_none());
    wake.join().unwrap()?;
    drop(client);
    server.finish()?;
    Ok(())
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
        WorkbenchRejectionReason::SessionMismatch,
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
            handshake::expect_hello(&mut stream)?;
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
        let mut client = LocalClient::connect(server.endpoint())?;
        let actual = client.submit_workbench_panel_placement(
            WorkbenchPanelPlacementRequest::try_from(panel_move_command(7)).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        match (outcome, actual) {
            (
                WorkbenchCommandOutcome::Unchanged { revision: expected },
                WorkbenchPanelPlacementSubmission::Unchanged { revision },
            ) => assert_eq!(revision, expected),
            (
                WorkbenchCommandOutcome::PanelMoved {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    target_panel_id,
                },
                WorkbenchPanelPlacementSubmission::Committed(actual),
            ) => assert_eq!(
                actual,
                WorkbenchPanelPlacementCommit {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    target_panel_id,
                }
            ),
            (
                WorkbenchCommandOutcome::Denied {},
                WorkbenchPanelPlacementSubmission::DefinitePrecommit(
                    WorkbenchPanelPlacementPrecommit::Denied,
                ),
            ) => {}
            (
                WorkbenchCommandOutcome::Rejected { revision, reason },
                WorkbenchPanelPlacementSubmission::DefinitePrecommit(
                    WorkbenchPanelPlacementPrecommit::Rejected {
                        revision: actual,
                        reason: actual_reason,
                    },
                ),
            ) => assert_eq!((actual, actual_reason), (revision, reason)),
            (
                WorkbenchCommandOutcome::Unavailable { reason },
                WorkbenchPanelPlacementSubmission::DefinitePrecommit(
                    WorkbenchPanelPlacementPrecommit::Unavailable { reason: actual },
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
            matches!(result, WorkbenchPanelPlacementSubmission::OutcomeUnknown {request_id,..} if request_id.as_str()==FIRST_REQUEST_ID)
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
        WorkbenchPanelPlacementSubmission::OutcomeUnknown {
            reason: WorkbenchPanelPlacementOutcomeUnknownReason::UnexpectedResponse,
            ..
        }
    ));
    assert!(
        matches!(invalid_move(7, command_response(SECOND_REQUEST_ID,moved(8,30,31,32,33)))?, WorkbenchPanelPlacementSubmission::OutcomeUnknown {reason: WorkbenchPanelPlacementOutcomeUnknownReason::CorrelationMismatch {expected,received},..} if expected.as_str()==FIRST_REQUEST_ID && received.as_str()==SECOND_REQUEST_ID)
    );
    Ok(())
}

#[test]
fn panel_move_prewrite_expiry_preserves_carrier_and_sends_no_frame() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        handshake::expect_hello(&mut stream)?;
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
    let mut client = LocalClient::connect(server.endpoint())?;
    let deadline = client.operation_clock().now();
    assert!(
        matches!(client.submit_workbench_panel_placement(WorkbenchPanelPlacementRequest::try_from(panel_move_command(7)).unwrap(),deadline),WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(WorkbenchPanelPlacementNotSubmitted::RequestNotSubmitted(error)) if error.kind()==io::ErrorKind::TimedOut)
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
) -> Result<WorkbenchPanelPlacementSubmission, Box<dyn std::error::Error>> {
    let server = FakeServer::start(move |mut stream| {
        handshake::expect_hello(&mut stream)?;
        admit(&mut stream, 54)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(panel_move_command(baseline))
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint())?;
    let result = client.submit_workbench_panel_placement(
        WorkbenchPanelPlacementRequest::try_from(panel_move_command(baseline)).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(result)
}

fn panel_swap_command() -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        7,
        WorkbenchAction::PanelSwap {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            target_panel_id: WorkbenchPanelId::parse(&id(33)).unwrap(),
        },
    )
}
fn swapped() -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelSwapped {
        revision: WorkbenchRevision::new(8),
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
        target_panel_id: WorkbenchPanelId::parse(&id(33)).unwrap(),
    }
}

#[test]
fn panel_swap_accepts_only_its_commit_and_retires_cross_action_or_lost_replies() -> TestResult {
    for outcome in [
        Some(swapped()),
        Some(moved(8, 30, 31, 32, 33)),
        None,
        Some(WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(7),
        }),
    ] {
        let valid = outcome == Some(swapped());
        let server = FakeServer::start(move |mut stream| {
            handshake::expect_hello(&mut stream)?;
            admit(&mut stream, 71)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(panel_swap_command())
            );
            if let Some(outcome) = outcome {
                write_workbench_response(&mut stream, &command_response(FIRST_REQUEST_ID, outcome))
                    .map_err(io::Error::other)?;
                require_retirement(&mut stream)?;
            }
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint())?;
        let answer = client.submit_workbench_panel_placement(
            WorkbenchPanelPlacementRequest::try_from(panel_swap_command()).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        if valid {
            assert!(matches!(
                answer,
                WorkbenchPanelPlacementSubmission::Committed(_)
            ));
            assert_eq!(client.generation().map(NonZeroU64::get), Some(71));
        } else {
            assert!(matches!(
                answer,
                WorkbenchPanelPlacementSubmission::OutcomeUnknown { .. }
            ));
            assert_eq!(client.generation(), None);
        }
        drop(client);
        server.finish()?;
    }
    // A Panel swap result also cannot acknowledge the old move action.
    assert!(matches!(
        invalid_move(7, command_response(FIRST_REQUEST_ID, swapped()))?,
        WorkbenchPanelPlacementSubmission::OutcomeUnknown { .. }
    ));
    Ok(())
}
