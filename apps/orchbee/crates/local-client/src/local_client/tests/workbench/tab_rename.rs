use super::*;
use crate::local_client::workbench::label_rename::{
    WorkbenchLabelRenameNotSubmitted, WorkbenchLabelRenameOutcomeUnknownReason,
    WorkbenchLabelRenamePrecommit, WorkbenchLabelRenameRequest, WorkbenchLabelRenameSubmission,
};
use orchbee_workbench_protocol::WorkbenchTabMoveDirection;

#[test]
fn opaque_rename_request_rejects_other_actions_unchanged() {
    assert!(WorkbenchLabelRenameRequest::try_from(rename_command(7)).is_ok());
    for action in [
        WorkbenchAction::ProjectCreate {
            label: workbench_label(),
        },
        WorkbenchAction::ProjectRename {
            project_id: WorkbenchProjectId::parse(&id(21)).unwrap(),
            label: workbench_label(),
        },
        WorkbenchAction::WorkspaceCreate {
            project_id: Some(WorkbenchProjectId::parse(&id(21)).unwrap()),
            label: workbench_label(),
        },
        WorkbenchAction::WorkspaceRename {
            workspace_id: WorkbenchWorkspaceId::parse(&id(22)).unwrap(),
            label: workbench_label(),
        },
        tab_create_command(FIRST_REQUEST_ID, 7).action,
        WorkbenchAction::TabMove {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            direction: WorkbenchTabMoveDirection::Next,
        },
        WorkbenchAction::TabClose {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        },
    ] {
        let command = workbench_command(FIRST_REQUEST_ID, 7, action);
        assert_eq!(
            WorkbenchLabelRenameRequest::try_from(command.clone()).unwrap_err(),
            command
        );
    }
}

#[test]
fn rename_preserves_exact_commit_noop_and_compatible_precommit() -> TestResult {
    let mut outcomes = vec![
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(7),
        },
        renamed(8),
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
    outcomes.push(WorkbenchCommandOutcome::Unchanged {
        revision: WorkbenchRevision::new(u64::MAX),
    });
    for outcome in outcomes {
        let baseline = match outcome {
            WorkbenchCommandOutcome::Unchanged { revision } => revision.value(),
            _ => 7,
        };
        let server_outcome = outcome.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 51)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(rename_command(baseline))
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
        let actual = client.submit_workbench_label_rename(
            WorkbenchLabelRenameRequest::try_from(rename_command(baseline)).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        match (outcome, actual) {
            (
                WorkbenchCommandOutcome::Unchanged { revision: expected },
                WorkbenchLabelRenameSubmission::Unchanged { revision },
            ) => assert_eq!(revision, expected),
            (
                WorkbenchCommandOutcome::Renamed { revision: expected },
                WorkbenchLabelRenameSubmission::Committed { revision },
            ) => assert_eq!(revision, expected),
            (
                WorkbenchCommandOutcome::Denied {},
                WorkbenchLabelRenameSubmission::DefinitePrecommit(
                    WorkbenchLabelRenamePrecommit::Denied,
                ),
            ) => {}
            (
                WorkbenchCommandOutcome::Rejected { revision, reason },
                WorkbenchLabelRenameSubmission::DefinitePrecommit(
                    WorkbenchLabelRenamePrecommit::Rejected {
                        revision: actual,
                        reason: actual_reason,
                    },
                ),
            ) => assert_eq!((actual, actual_reason), (revision, reason)),
            (
                WorkbenchCommandOutcome::Unavailable { reason },
                WorkbenchLabelRenameSubmission::DefinitePrecommit(
                    WorkbenchLabelRenamePrecommit::Unavailable { reason: actual },
                ),
            ) => assert_eq!(actual, reason),
            other => panic!("unexpected rename classification {other:?}"),
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
fn rename_retires_impossible_revisions_and_action_results() -> TestResult {
    for (baseline, outcome) in [
        (
            7,
            WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(7),
                reason: WorkbenchRejectionReason::ProjectUnavailable,
            },
        ),
        (
            7,
            WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(7),
                reason: WorkbenchRejectionReason::WorkspaceUnavailable,
            },
        ),
        (7, renamed(7)),
        (7, renamed(9)),
        (u64::MAX, renamed(u64::MAX)),
        (
            7,
            WorkbenchCommandOutcome::TabCreated {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
                panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
                presentation_id: TerminalPresentationId::parse(&id(33)).unwrap(),
                terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
            },
        ),
        (
            7,
            WorkbenchCommandOutcome::Unchanged {
                revision: WorkbenchRevision::new(8),
            },
        ),
        (
            7,
            WorkbenchCommandOutcome::TabMoved {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
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
        let result = invalid_rename(baseline, command_response(FIRST_REQUEST_ID, outcome))?;
        assert!(
            matches!(result, WorkbenchLabelRenameSubmission::OutcomeUnknown {request_id,..} if request_id.as_str()==FIRST_REQUEST_ID)
        );
    }
    Ok(())
}

#[test]
fn rename_retires_wrong_kind_and_correlation_without_replay() -> TestResult {
    assert!(matches!(
        invalid_rename(
            7,
            WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID))
        )?,
        WorkbenchLabelRenameSubmission::OutcomeUnknown {
            reason: WorkbenchLabelRenameOutcomeUnknownReason::UnexpectedResponse,
            ..
        }
    ));
    assert!(
        matches!(invalid_rename(7, command_response(SECOND_REQUEST_ID,renamed(8)))?, WorkbenchLabelRenameSubmission::OutcomeUnknown {reason: WorkbenchLabelRenameOutcomeUnknownReason::CorrelationMismatch {expected,received},..} if expected.as_str()==FIRST_REQUEST_ID && received.as_str()==SECOND_REQUEST_ID)
    );
    Ok(())
}

#[test]
fn rename_prewrite_expiry_preserves_carrier_and_sends_no_frame() -> TestResult {
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
        matches!(client.submit_workbench_label_rename(WorkbenchLabelRenameRequest::try_from(rename_command(7)).unwrap(),deadline),WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(WorkbenchLabelRenameNotSubmitted::RequestNotSubmitted(error)) if error.kind()==io::ErrorKind::TimedOut)
    );
    client.set_submission_deadline(Some(client.operation_clock().now()));
    assert!(matches!(client.submit_workbench_label_rename(
        WorkbenchLabelRenameRequest::try_from(rename_command(7)).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    ), WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(WorkbenchLabelRenameNotSubmitted::RequestNotSubmitted(error)) if error.kind() == io::ErrorKind::TimedOut));
    client.set_submission_deadline(None);
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
fn rename_partial_write_expiry_is_unknown_and_retires() -> TestResult {
    let mut frame = Vec::new();
    write_workbench_request(&mut frame, &WorkbenchRequest::Command(rename_command(7)))?;
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
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
        COHORT,
        SERVER_GUARD_TIMEOUT,
        Box::new(clock),
        IoChunkLimits::new(usize::MAX, 1),
    )?;
    assert!(
        matches!(client.submit_workbench_label_rename(WorkbenchLabelRenameRequest::try_from(rename_command(7)).unwrap(),deadline),WorkbenchLabelRenameSubmission::OutcomeUnknown {reason:WorkbenchLabelRenameOutcomeUnknownReason::Write(error),..} if error.kind()==io::ErrorKind::TimedOut)
    );
    assert_eq!(client.generation(), None);
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn rename_completed_response_past_caller_or_exchange_budget_retires_without_replay() -> TestResult {
    for caller_is_earlier in [true, false] {
        let server = FakeServer::start(|mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 55)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(rename_command(7))
            );
            write_workbench_response(&mut stream, &command_response(FIRST_REQUEST_ID, renamed(8)))
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
            COHORT,
            timeout,
            Box::new(clock),
            IoChunkLimits::UNBOUNDED,
        )?;
        assert!(matches!(client.submit_workbench_label_rename(
            WorkbenchLabelRenameRequest::try_from(rename_command(7)).unwrap(), deadline),
            WorkbenchLabelRenameSubmission::OutcomeUnknown {
                reason: WorkbenchLabelRenameOutcomeUnknownReason::ResponseDeadline(error), ..
            } if error.kind() == io::ErrorKind::TimedOut));
        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.submit_workbench_label_rename(
                WorkbenchLabelRenameRequest::try_from(rename_command(7)).unwrap(),
                deadline
            ),
            WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(
                WorkbenchLabelRenameNotSubmitted::ConnectionUnavailable
            )
        ));
        drop(client);
        server.finish()?;
    }
    Ok(())
}

fn invalid_rename(
    baseline: u64,
    response: WorkbenchResponse,
) -> Result<WorkbenchLabelRenameSubmission, Box<dyn std::error::Error>> {
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 54)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(rename_command(baseline))
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let result = client.submit_workbench_label_rename(
        WorkbenchLabelRenameRequest::try_from(rename_command(baseline)).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(result)
}

fn rename_command(baseline: u64) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        baseline,
        WorkbenchAction::TabRename {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            label: WorkbenchLabel::parse("  \u{6807}\u{7b7e} \u{1f680}  ").unwrap(),
        },
    )
}

fn renamed(revision: u64) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::Renamed {
        revision: WorkbenchRevision::new(revision),
    }
}

#[test]
fn rename_retires_close_reasons_acceptance_and_control_envelopes() -> TestResult {
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
            invalid_rename(7, command_response(FIRST_REQUEST_ID, outcome))?,
            WorkbenchLabelRenameSubmission::OutcomeUnknown {
                reason: WorkbenchLabelRenameOutcomeUnknownReason::ActionIncompatible,
                ..
            }
        ));
    }
    for response in close_control_responses() {
        assert!(matches!(
            invalid_rename(7, response)?,
            WorkbenchLabelRenameSubmission::OutcomeUnknown {
                reason: WorkbenchLabelRenameOutcomeUnknownReason::UnexpectedResponse,
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn rename_retires_malformed_or_truncated_response_without_retry() -> TestResult {
    for truncated in [false, true] {
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 56)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(rename_command(7))
            );
            if truncated {
                write_workbench_header(&mut stream, 4, 10)?;
                stream.write_all(b"short")?;
                stream.shutdown(std::net::Shutdown::Write)?;
            } else {
                write_workbench_header(&mut stream, 9, 0)?;
            }
            require_retirement(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
        assert!(
            matches!(client.submit_workbench_label_rename(WorkbenchLabelRenameRequest::try_from(rename_command(7)).unwrap(), deadline), WorkbenchLabelRenameSubmission::OutcomeUnknown { request_id, reason: WorkbenchLabelRenameOutcomeUnknownReason::Response(_) } if request_id.as_str() == FIRST_REQUEST_ID)
        );
        assert!(matches!(
            client.submit_workbench_label_rename(
                WorkbenchLabelRenameRequest::try_from(rename_command(7)).unwrap(),
                deadline
            ),
            WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(
                WorkbenchLabelRenameNotSubmitted::ConnectionUnavailable
            )
        ));
        assert_eq!(client.generation(), None);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn panel_rename_carrier_preserves_exact_target_and_keeps_tab_rejection_scope_closed() -> TestResult
{
    for panel in [false, true] {
        let mut command = rename_command(7);
        if panel {
            command.action = WorkbenchAction::PanelRename {
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
                panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(32)).unwrap(),
                label: orchbee_workbench_protocol::PanelLabelUpdate::default(),
            };
        }
        let sent = command.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 55)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(sent)
            );
            write_workbench_response(
                &mut stream,
                &command_response(
                    FIRST_REQUEST_ID,
                    WorkbenchCommandOutcome::Rejected {
                        revision: WorkbenchRevision::new(7),
                        reason: WorkbenchRejectionReason::PanelUnavailable,
                    },
                ),
            )
            .map_err(io::Error::other)?;
            require_retirement(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let result = client.submit_workbench_label_rename(
            WorkbenchLabelRenameRequest::try_from(command).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        if panel {
            assert!(matches!(
                result,
                WorkbenchLabelRenameSubmission::DefinitePrecommit(
                    WorkbenchLabelRenamePrecommit::Rejected {
                        reason: WorkbenchRejectionReason::PanelUnavailable,
                        ..
                    }
                )
            ));
            assert!(client.generation().is_some());
        } else {
            assert!(matches!(
                result,
                WorkbenchLabelRenameSubmission::OutcomeUnknown {
                    reason: WorkbenchLabelRenameOutcomeUnknownReason::ActionIncompatible,
                    ..
                }
            ));
            assert!(client.generation().is_none());
        }
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn panel_rename_lost_reply_retires_without_replay() -> TestResult {
    let command = workbench_command(
        FIRST_REQUEST_ID,
        7,
        WorkbenchAction::PanelRename {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(32)).unwrap(),
            label: orchbee_workbench_protocol::PanelLabelUpdate::parse(Some("shared title"))
                .unwrap(),
        },
    );
    let sent = command.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 56)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(sent)
        );
        Ok(()) // Accepted bytes, then the response is lost.
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    assert!(matches!(
        client.submit_workbench_label_rename(
            WorkbenchLabelRenameRequest::try_from(command).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT
        ),
        WorkbenchLabelRenameSubmission::OutcomeUnknown { .. }
    ));
    assert!(client.generation().is_none());
    server.finish()?;
    Ok(())
}
