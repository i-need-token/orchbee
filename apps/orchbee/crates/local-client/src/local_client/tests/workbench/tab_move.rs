use super::*;
use crate::local_client::workbench::{
    WorkbenchTabMoveCommit, WorkbenchTabMoveNotSubmitted, WorkbenchTabMoveOutcomeUnknownReason,
    WorkbenchTabMovePrecommit, WorkbenchTabMoveRequest, WorkbenchTabMoveSubmission,
};
use orchbee_workbench_protocol::WorkbenchTabMoveDirection;

#[test]
fn opaque_move_request_rejects_other_actions_unchanged() {
    for direction in [
        WorkbenchTabMoveDirection::Previous,
        WorkbenchTabMoveDirection::Next,
        WorkbenchTabMoveDirection::First,
        WorkbenchTabMoveDirection::Last,
    ] {
        let mut command = move_command(7);
        let WorkbenchAction::TabMove {
            direction: actual, ..
        } = &mut command.action
        else {
            unreachable!()
        };
        *actual = direction;
        assert!(WorkbenchTabMoveRequest::try_from(command).is_ok());
    }
    let command = tab_create_command(FIRST_REQUEST_ID, 7);
    assert_eq!(
        WorkbenchTabMoveRequest::try_from(command.clone()).unwrap_err(),
        command
    );
}

#[test]
fn move_preserves_exact_commit_noop_and_compatible_precommit() -> TestResult {
    let mut outcomes = vec![
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(7),
        },
        moved(8, 30, 31),
        WorkbenchCommandOutcome::Denied {},
    ];
    for reason in [
        WorkbenchRejectionReason::SessionMismatch,
        WorkbenchRejectionReason::RevisionMismatch,
        WorkbenchRejectionReason::TerminalContextUnavailable,
        WorkbenchRejectionReason::TabUnavailable,
        WorkbenchRejectionReason::ResourceLimit,
        WorkbenchRejectionReason::RevisionExhausted,
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
                WorkbenchRequest::Command(move_command(7))
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
        let actual = client.submit_workbench_tab_move(
            WorkbenchTabMoveRequest::try_from(move_command(7)).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        match (outcome, actual) {
            (
                WorkbenchCommandOutcome::Unchanged { revision: expected },
                WorkbenchTabMoveSubmission::Unchanged { revision },
            ) => assert_eq!(revision, expected),
            (
                WorkbenchCommandOutcome::TabMoved {
                    revision,
                    terminal_context_id,
                    tab_id,
                },
                WorkbenchTabMoveSubmission::Committed(actual),
            ) => assert_eq!(
                actual,
                WorkbenchTabMoveCommit {
                    revision,
                    terminal_context_id,
                    tab_id
                }
            ),
            (
                WorkbenchCommandOutcome::Denied {},
                WorkbenchTabMoveSubmission::DefinitePrecommit(WorkbenchTabMovePrecommit::Denied),
            ) => {}
            (
                WorkbenchCommandOutcome::Rejected { revision, reason },
                WorkbenchTabMoveSubmission::DefinitePrecommit(
                    WorkbenchTabMovePrecommit::Rejected {
                        revision: actual,
                        reason: actual_reason,
                    },
                ),
            ) => assert_eq!((actual, actual_reason), (revision, reason)),
            (
                WorkbenchCommandOutcome::Unavailable { reason },
                WorkbenchTabMoveSubmission::DefinitePrecommit(
                    WorkbenchTabMovePrecommit::Unavailable { reason: actual },
                ),
            ) => assert_eq!(actual, reason),
            other => panic!("unexpected move classification {other:?}"),
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
fn move_retires_impossible_revisions_identities_and_action_results() -> TestResult {
    for (baseline, outcome) in [
        (7, moved(7, 30, 31)),
        (7, moved(9, 30, 31)),
        (u64::MAX, moved(u64::MAX, 30, 31)),
        (7, moved(8, 99, 31)),
        (7, moved(8, 30, 99)),
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
                reason: WorkbenchRejectionReason::IdentityExhausted,
            },
        ),
        (
            7,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            },
        ),
        (
            7,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServerBusy,
            },
        ),
    ] {
        let result = invalid_move(baseline, command_response(FIRST_REQUEST_ID, outcome))?;
        assert!(
            matches!(result, WorkbenchTabMoveSubmission::OutcomeUnknown {request_id,..} if request_id.as_str()==FIRST_REQUEST_ID)
        );
    }
    Ok(())
}

#[test]
fn move_retires_wrong_kind_and_correlation_without_replay() -> TestResult {
    assert!(matches!(
        invalid_move(
            7,
            WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID))
        )?,
        WorkbenchTabMoveSubmission::OutcomeUnknown {
            reason: WorkbenchTabMoveOutcomeUnknownReason::UnexpectedResponse,
            ..
        }
    ));
    assert!(
        matches!(invalid_move(7, command_response(SECOND_REQUEST_ID,moved(8,30,31)))?, WorkbenchTabMoveSubmission::OutcomeUnknown {reason: WorkbenchTabMoveOutcomeUnknownReason::CorrelationMismatch {expected,received},..} if expected.as_str()==FIRST_REQUEST_ID && received.as_str()==SECOND_REQUEST_ID)
    );
    Ok(())
}

#[test]
fn move_prewrite_expiry_preserves_carrier_and_sends_no_frame() -> TestResult {
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
        matches!(client.submit_workbench_tab_move(WorkbenchTabMoveRequest::try_from(move_command(7)).unwrap(),deadline),WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(WorkbenchTabMoveNotSubmitted::RequestNotSubmitted(error)) if error.kind()==io::ErrorKind::TimedOut)
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

#[test]
fn move_partial_write_expiry_is_unknown_and_retires() -> TestResult {
    let mut frame = Vec::new();
    write_workbench_request(&mut frame, &WorkbenchRequest::Command(move_command(7)))?;
    let server = FakeServer::start(move |mut stream| {
        handshake::expect_hello(&mut stream)?;
        admit(&mut stream, 53)?;
        let mut received = Vec::new();
        stream.read_to_end(&mut received)?;
        assert!(!received.is_empty() && received.len() < frame.len());
        assert_eq!(received, frame[..received.len()]);
        Ok(())
    })?;
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), Arc::new(AtomicU64::new(0)));
    let deadline = clock.origin + Duration::from_millis(90);
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        SERVER_GUARD_TIMEOUT,
        Box::new(clock),
        IoChunkLimits::new(usize::MAX, 1),
    )?;
    assert!(
        matches!(client.submit_workbench_tab_move(WorkbenchTabMoveRequest::try_from(move_command(7)).unwrap(),deadline),WorkbenchTabMoveSubmission::OutcomeUnknown {reason:WorkbenchTabMoveOutcomeUnknownReason::Write(error),..} if error.kind()==io::ErrorKind::TimedOut)
    );
    assert_eq!(client.generation(), None);
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn move_completed_response_past_caller_or_exchange_budget_retires_without_replay() -> TestResult {
    for caller_is_earlier in [true, false] {
        let server = FakeServer::start(|mut stream| {
            handshake::expect_hello(&mut stream)?;
            admit(&mut stream, 55)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(move_command(7))
            );
            write_workbench_response(
                &mut stream,
                &command_response(FIRST_REQUEST_ID, moved(8, 30, 31)),
            )
            .map_err(io::Error::other)?;
            require_retirement(&mut stream)
        })?;
        let clock =
            SteppingMonotonicClock::new(Duration::from_millis(10), Arc::new(AtomicU64::new(0)));
        let deadline = clock.origin
            + if caller_is_earlier {
                Duration::from_millis(80)
            } else {
                SERVER_GUARD_TIMEOUT
            };
        let timeout = if caller_is_earlier {
            SERVER_GUARD_TIMEOUT
        } else {
            Duration::from_millis(80)
        };
        let mut client = LocalClient::connect_with_submission_runtime(
            server.endpoint(),
            timeout,
            Box::new(clock),
            IoChunkLimits::UNBOUNDED,
        )?;
        assert!(matches!(client.submit_workbench_tab_move(
            WorkbenchTabMoveRequest::try_from(move_command(7)).unwrap(), deadline),
            WorkbenchTabMoveSubmission::OutcomeUnknown {
                reason: WorkbenchTabMoveOutcomeUnknownReason::ResponseDeadline(error), ..
            } if error.kind() == io::ErrorKind::TimedOut));
        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.submit_workbench_tab_move(
                WorkbenchTabMoveRequest::try_from(move_command(7)).unwrap(),
                deadline
            ),
            WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(
                WorkbenchTabMoveNotSubmitted::ConnectionUnavailable
            )
        ));
        drop(client);
        server.finish()?;
    }
    Ok(())
}

fn invalid_move(
    baseline: u64,
    response: WorkbenchResponse,
) -> Result<WorkbenchTabMoveSubmission, Box<dyn std::error::Error>> {
    let server = FakeServer::start(move |mut stream| {
        handshake::expect_hello(&mut stream)?;
        admit(&mut stream, 54)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(move_command(baseline))
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint())?;
    let result = client.submit_workbench_tab_move(
        WorkbenchTabMoveRequest::try_from(move_command(baseline)).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(result)
}

fn move_command(baseline: u64) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        baseline,
        WorkbenchAction::TabMove {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            direction: WorkbenchTabMoveDirection::Next,
        },
    )
}

fn moved(revision: u64, context: u128, tab: u128) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::TabMoved {
        revision: WorkbenchRevision::new(revision),
        terminal_context_id: context_id(context),
        tab_id: WorkbenchTabId::parse(&id(tab)).unwrap(),
    }
}

#[test]
fn move_retires_close_reasons_acceptance_and_control_envelopes() -> TestResult {
    for outcome in [
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy,
        },
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
        },
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id: WorkbenchRequestId::parse(FIRST_REQUEST_ID)?,
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31))?,
            accepted_revision: WorkbenchRevision::new(7),
        },
    ] {
        assert!(matches!(
            invalid_move(7, command_response(FIRST_REQUEST_ID, outcome))?,
            WorkbenchTabMoveSubmission::OutcomeUnknown {
                reason: WorkbenchTabMoveOutcomeUnknownReason::ActionIncompatible,
                ..
            }
        ));
    }
    for response in close_control_responses() {
        assert!(matches!(
            invalid_move(7, response)?,
            WorkbenchTabMoveSubmission::OutcomeUnknown {
                reason: WorkbenchTabMoveOutcomeUnknownReason::UnexpectedResponse,
                ..
            }
        ));
    }
    Ok(())
}
