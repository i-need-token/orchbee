use super::*;
mod location_labels;
mod tab_configuration;
mod workspace_order;
use crate::local_client::workbench::workspace_setup::{
    WorkbenchSetupCommit, WorkbenchSetupNotSubmitted, WorkbenchSetupOutcomeUnknownReason,
    WorkbenchSetupPrecommit, WorkbenchSetupRequest, WorkbenchSetupSubmission,
};
use orchbee_workbench_protocol::{WorkbenchDirectoryId, WorkbenchDirectoryPath};

fn setup_command(kind: usize, baseline: u64) -> WorkbenchCommand {
    let project_id = WorkbenchProjectId::parse(&id(21)).unwrap();
    let action = match kind {
        0 => WorkbenchAction::ProjectCreate {
            label: workbench_label(),
        },
        1 => WorkbenchAction::WorkspaceCreate {
            project_id: Some(project_id),
            label: workbench_label(),
        },
        2 => WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            workspace_id: WorkbenchWorkspaceId::parse(&id(22)).unwrap(),
            path: WorkbenchDirectoryPath::parse("/tmp/literal $HOME space").unwrap(),
        },
        _ => unreachable!("setup fixture"),
    };
    workbench_command(FIRST_REQUEST_ID, baseline, action)
}

fn setup_outcome(kind: usize, revision: u64) -> WorkbenchCommandOutcome {
    let revision = WorkbenchRevision::new(revision);
    match kind {
        0 => WorkbenchCommandOutcome::ProjectCreated {
            revision,
            project_id: WorkbenchProjectId::parse(&id(41)).unwrap(),
        },
        1 => WorkbenchCommandOutcome::WorkspaceCreated {
            revision,
            workspace_id: WorkbenchWorkspaceId::parse(&id(42)).unwrap(),
            terminal_context_id: context_id(43),
        },
        2 => WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
            revision,
            directory_id: WorkbenchDirectoryId::parse(&id(44)).unwrap(),
            worker_id: WorkerId::parse(&id(45)).unwrap(),
        },
        _ => unreachable!("setup fixture"),
    }
}

#[test]
fn setup_wrapper_accepts_setup_and_location_label_actions() {
    for kind in 0..3 {
        let request = WorkbenchSetupRequest::try_from(setup_command(kind, 7)).unwrap();
        assert_eq!(request.request_id().as_str(), FIRST_REQUEST_ID);
    }
    for action in [
        tab_create_command(FIRST_REQUEST_ID, 7).action,
        WorkbenchAction::TabRename {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            label: workbench_label(),
        },
        WorkbenchAction::TabMove {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            direction: orchbee_workbench_protocol::WorkbenchTabMoveDirection::Next,
        },
        WorkbenchAction::TabClose {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        },
    ] {
        let command = workbench_command(FIRST_REQUEST_ID, 7, action);
        assert_eq!(
            WorkbenchSetupRequest::try_from(command.clone()).unwrap_err(),
            command
        );
    }
}

// The next frame must be an explicit snapshot read, never a mutation replay or implicit shell launch.
fn known_setup(
    kind: usize,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchSetupSubmission, Box<dyn std::error::Error>> {
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 61)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(setup_command(kind, 7))
        );
        write_workbench_response(&mut stream, &command_response(FIRST_REQUEST_ID, outcome))
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
    let result = client.submit_workbench_setup(
        WorkbenchSetupRequest::try_from(setup_command(kind, 7)).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert_eq!(client.generation().map(NonZeroU64::get), Some(61));
    assert_eq!(
        client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
        snapshot_result(SECOND_REQUEST_ID)
    );
    drop(client);
    server.finish()?;
    Ok(result)
}

#[test]
fn setup_commits_retain_exact_created_identities() -> TestResult {
    for kind in 0..3 {
        let expected = match kind {
            0 => WorkbenchSetupCommit::ProjectCreated {
                revision: WorkbenchRevision::new(8),
                project_id: WorkbenchProjectId::parse(&id(41))?,
            },
            1 => WorkbenchSetupCommit::WorkspaceCreated {
                revision: WorkbenchRevision::new(8),
                workspace_id: WorkbenchWorkspaceId::parse(&id(42))?,
                terminal_context_id: context_id(43),
            },
            2 => WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
                revision: WorkbenchRevision::new(8),
                directory_id: WorkbenchDirectoryId::parse(&id(44))?,
                worker_id: WorkerId::parse(&id(45))?,
            },
            _ => unreachable!(),
        };
        assert!(
            matches!(known_setup(kind, setup_outcome(kind, 8))?, WorkbenchSetupSubmission::Committed(actual) if actual == expected)
        );
    }
    Ok(())
}

#[test]
fn setup_refusals_are_action_specific_and_preserve_the_carrier() -> TestResult {
    for kind in 0..3 {
        assert!(matches!(
            known_setup(kind, WorkbenchCommandOutcome::Denied {})?,
            WorkbenchSetupSubmission::DefinitePrecommit(WorkbenchSetupPrecommit::Denied)
        ));
        for (reason, admitted) in [
            (WorkbenchRejectionReason::SessionMismatch, true),
            (WorkbenchRejectionReason::RevisionMismatch, true),
            (WorkbenchRejectionReason::ResourceLimit, true),
            (WorkbenchRejectionReason::IdentityExhausted, true),
            (WorkbenchRejectionReason::RevisionExhausted, true),
            (WorkbenchRejectionReason::RequestIdentityConflict, true),
            (WorkbenchRejectionReason::ProjectUnavailable, kind != 0),
            (WorkbenchRejectionReason::WorkspaceUnavailable, kind == 2),
            (
                WorkbenchRejectionReason::WorkspaceDirectoryConflict,
                kind == 2,
            ),
            (
                WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                false,
            ),
            (WorkbenchRejectionReason::TerminalContextUnavailable, false),
            (WorkbenchRejectionReason::TabUnavailable, false),
        ] {
            let revision = WorkbenchRevision::new(99);
            let outcome = WorkbenchCommandOutcome::Rejected { revision, reason };
            if admitted {
                assert!(
                    matches!(known_setup(kind, outcome)?, WorkbenchSetupSubmission::DefinitePrecommit(WorkbenchSetupPrecommit::Rejected { revision: actual_revision, reason: actual_reason }) if actual_revision == revision && actual_reason == reason)
                );
            } else {
                assert!(matches!(
                    invalid_setup(kind, 7, command_response(FIRST_REQUEST_ID, outcome))?,
                    WorkbenchSetupSubmission::OutcomeUnknown {
                        reason: WorkbenchSetupOutcomeUnknownReason::ActionIncompatible,
                        ..
                    }
                ));
            }
        }
        for (reason, admitted) in [
            (WorkbenchCommandUnavailableReason::RequestLedgerFull, true),
            (WorkbenchCommandUnavailableReason::ServiceUnavailable, true),
            (
                WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                kind == 2,
            ),
            (WorkbenchCommandUnavailableReason::ServerBusy, kind == 2),
            (
                WorkbenchCommandUnavailableReason::DirectoryUnavailable,
                kind == 2,
            ),
            (WorkbenchCommandUnavailableReason::TerminalBusy, false),
            (
                WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
                false,
            ),
        ] {
            let outcome = WorkbenchCommandOutcome::Unavailable { reason };
            if admitted {
                assert!(
                    matches!(known_setup(kind, outcome)?, WorkbenchSetupSubmission::DefinitePrecommit(WorkbenchSetupPrecommit::Unavailable { reason: actual }) if actual == reason)
                );
            } else {
                assert!(matches!(
                    invalid_setup(kind, 7, command_response(FIRST_REQUEST_ID, outcome))?,
                    WorkbenchSetupSubmission::OutcomeUnknown {
                        reason: WorkbenchSetupOutcomeUnknownReason::ActionIncompatible,
                        ..
                    }
                ));
            }
        }
    }
    Ok(())
}

fn invalid_setup(
    kind: usize,
    baseline: u64,
    response: WorkbenchResponse,
) -> Result<WorkbenchSetupSubmission, Box<dyn std::error::Error>> {
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 62)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(setup_command(kind, baseline))
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
    let result = client.submit_workbench_setup(
        WorkbenchSetupRequest::try_from(setup_command(kind, baseline)).unwrap(),
        deadline,
    );
    assert_eq!(client.generation(), None);
    assert!(matches!(
        client.submit_workbench_setup(
            WorkbenchSetupRequest::try_from(setup_command(kind, baseline)).unwrap(),
            deadline
        ),
        WorkbenchSetupSubmission::DefinitelyNotSubmitted(
            WorkbenchSetupNotSubmitted::ConnectionUnavailable
        )
    ));
    server.finish()?;
    Ok(result)
}

#[test]
fn setup_retires_wrong_commit_kind_revision_and_correlation() -> TestResult {
    for kind in 0..3 {
        for wrong_kind in 0..3 {
            if wrong_kind != kind {
                assert!(matches!(
                    invalid_setup(
                        kind,
                        7,
                        command_response(FIRST_REQUEST_ID, setup_outcome(wrong_kind, 8))
                    )?,
                    WorkbenchSetupSubmission::OutcomeUnknown {
                        reason: WorkbenchSetupOutcomeUnknownReason::ActionIncompatible,
                        ..
                    }
                ));
            }
        }
        for revision in [7, 9] {
            assert!(
                matches!(invalid_setup(kind, 7, command_response(FIRST_REQUEST_ID, setup_outcome(kind, revision)))?, WorkbenchSetupSubmission::OutcomeUnknown { reason: WorkbenchSetupOutcomeUnknownReason::RevisionMismatch { expected, received }, .. } if expected.value() == 8 && received.value() == revision)
            );
        }
        assert!(matches!(
            invalid_setup(
                kind,
                u64::MAX,
                command_response(FIRST_REQUEST_ID, setup_outcome(kind, u64::MAX))
            )?,
            WorkbenchSetupSubmission::OutcomeUnknown {
                reason: WorkbenchSetupOutcomeUnknownReason::ExpectedRevisionOverflow { .. },
                ..
            }
        ));
        assert!(
            matches!(invalid_setup(kind, 7, command_response(SECOND_REQUEST_ID, setup_outcome(kind, 8)))?, WorkbenchSetupSubmission::OutcomeUnknown { request_id, reason: WorkbenchSetupOutcomeUnknownReason::CorrelationMismatch { expected, received } } if request_id.as_str() == FIRST_REQUEST_ID && expected.as_str() == FIRST_REQUEST_ID && received.as_str() == SECOND_REQUEST_ID)
        );
        for response in close_control_responses()
            .into_iter()
            .chain([WorkbenchResponse::Snapshot(snapshot_result(
                FIRST_REQUEST_ID,
            ))])
        {
            assert!(matches!(
                invalid_setup(kind, 7, response)?,
                WorkbenchSetupSubmission::OutcomeUnknown {
                    reason: WorkbenchSetupOutcomeUnknownReason::UnexpectedResponse,
                    ..
                }
            ));
        }
        for outcome in [
            WorkbenchCommandOutcome::Unchanged {
                revision: WorkbenchRevision::new(7),
            },
            WorkbenchCommandOutcome::Renamed {
                revision: WorkbenchRevision::new(8),
            },
        ] {
            assert!(matches!(
                invalid_setup(kind, 7, command_response(FIRST_REQUEST_ID, outcome))?,
                WorkbenchSetupSubmission::OutcomeUnknown {
                    reason: WorkbenchSetupOutcomeUnknownReason::ActionIncompatible,
                    ..
                }
            ));
        }
    }
    Ok(())
}

#[test]
fn setup_prewrite_expiry_preserves_carrier_and_sends_no_frame() -> TestResult {
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
        matches!(client.submit_workbench_setup(WorkbenchSetupRequest::try_from(setup_command(2, 7)).unwrap(),deadline),WorkbenchSetupSubmission::DefinitelyNotSubmitted(WorkbenchSetupNotSubmitted::RequestNotSubmitted(error)) if error.kind()==io::ErrorKind::TimedOut)
    );
    client.set_submission_deadline(Some(client.operation_clock().now()));
    assert!(matches!(client.submit_workbench_setup(
        WorkbenchSetupRequest::try_from(setup_command(2, 7)).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    ), WorkbenchSetupSubmission::DefinitelyNotSubmitted(WorkbenchSetupNotSubmitted::RequestNotSubmitted(error)) if error.kind() == io::ErrorKind::TimedOut));
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
fn setup_partial_write_expiry_is_unknown_and_retires() -> TestResult {
    let mut frame = Vec::new();
    write_workbench_request(&mut frame, &WorkbenchRequest::Command(setup_command(2, 7)))?;
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
        matches!(client.submit_workbench_setup(WorkbenchSetupRequest::try_from(setup_command(2, 7)).unwrap(),deadline),WorkbenchSetupSubmission::OutcomeUnknown {reason:WorkbenchSetupOutcomeUnknownReason::Write(error),..} if error.kind()==io::ErrorKind::TimedOut)
    );
    assert_eq!(client.generation(), None);
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn setup_completed_response_past_caller_or_exchange_budget_retires_without_replay() -> TestResult {
    for caller_is_earlier in [true, false] {
        let server = FakeServer::start(|mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 55)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(setup_command(2, 7))
            );
            write_workbench_response(
                &mut stream,
                &command_response(FIRST_REQUEST_ID, setup_outcome(2, 8)),
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
            COHORT,
            timeout,
            Box::new(clock),
            IoChunkLimits::UNBOUNDED,
        )?;
        assert!(matches!(client.submit_workbench_setup(
            WorkbenchSetupRequest::try_from(setup_command(2, 7)).unwrap(), deadline),
            WorkbenchSetupSubmission::OutcomeUnknown {
                reason: WorkbenchSetupOutcomeUnknownReason::ResponseDeadline(error), ..
            } if error.kind() == io::ErrorKind::TimedOut));
        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.submit_workbench_setup(
                WorkbenchSetupRequest::try_from(setup_command(2, 7)).unwrap(),
                deadline
            ),
            WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                WorkbenchSetupNotSubmitted::ConnectionUnavailable
            )
        ));
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn setup_retires_malformed_or_truncated_response_without_retry() -> TestResult {
    for truncated in [false, true] {
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 56)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(setup_command(2, 7))
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
            matches!(client.submit_workbench_setup(WorkbenchSetupRequest::try_from(setup_command(2, 7)).unwrap(), deadline), WorkbenchSetupSubmission::OutcomeUnknown { request_id, reason: WorkbenchSetupOutcomeUnknownReason::Response(_) } if request_id.as_str() == FIRST_REQUEST_ID)
        );
        assert!(matches!(
            client.submit_workbench_setup(
                WorkbenchSetupRequest::try_from(setup_command(2, 7)).unwrap(),
                deadline
            ),
            WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                WorkbenchSetupNotSubmitted::ConnectionUnavailable
            )
        ));
        assert_eq!(client.generation(), None);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn setup_lost_result_retires_each_action_without_replay() -> TestResult {
    for kind in 0..3 {
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 63)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(setup_command(kind, 7))
            );
            stream.shutdown(std::net::Shutdown::Write)?;
            require_retirement(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
        assert!(
            matches!(client.submit_workbench_setup(WorkbenchSetupRequest::try_from(setup_command(kind, 7)).unwrap(), deadline), WorkbenchSetupSubmission::OutcomeUnknown { request_id, reason: WorkbenchSetupOutcomeUnknownReason::Response(_) } if request_id.as_str() == FIRST_REQUEST_ID)
        );
        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.submit_workbench_setup(
                WorkbenchSetupRequest::try_from(setup_command(kind, 7)).unwrap(),
                deadline
            ),
            WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                WorkbenchSetupNotSubmitted::ConnectionUnavailable
            )
        ));
        server.finish()?;
    }
    Ok(())
}

#[test]
fn setup_retires_terminal_mutation_receipts() -> TestResult {
    for kind in 0..3 {
        for outcome in [
            WorkbenchCommandOutcome::TabMoved {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31))?,
            },
            WorkbenchCommandOutcome::TabCloseAccepted {
                close_request_id: workbench_id(FIRST_REQUEST_ID),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31))?,
                accepted_revision: WorkbenchRevision::new(8),
            },
            WorkbenchCommandOutcome::TabCreatePending {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31))?,
                panel_id: WorkbenchPanelId::parse(&id(32))?,
                presentation_id: TerminalPresentationId::parse(&id(33))?,
            },
            WorkbenchCommandOutcome::TabCreated {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31))?,
                panel_id: WorkbenchPanelId::parse(&id(32))?,
                presentation_id: TerminalPresentationId::parse(&id(33))?,
                terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
            },
        ] {
            assert!(matches!(
                invalid_setup(kind, 7, command_response(FIRST_REQUEST_ID, outcome))?,
                WorkbenchSetupSubmission::OutcomeUnknown {
                    reason: WorkbenchSetupOutcomeUnknownReason::ActionIncompatible,
                    ..
                }
            ));
        }
    }
    Ok(())
}

fn exchange_setup(
    command: WorkbenchCommand,
    response: WorkbenchResponse,
    retain_carrier: bool,
) -> Result<WorkbenchSetupSubmission, Box<dyn std::error::Error>> {
    let expected = command.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 71)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(expected)
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        if retain_carrier {
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::SnapshotRead(snapshot_request(SECOND_REQUEST_ID))
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(snapshot_result(SECOND_REQUEST_ID)),
            )
            .map_err(io::Error::other)
        } else {
            require_retirement(&mut stream)
        }
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let request = WorkbenchSetupRequest::try_from(command).unwrap();
    let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
    let actual = client.submit_workbench_setup(request.clone(), deadline);
    if retain_carrier {
        assert_eq!(client.generation().map(NonZeroU64::get), Some(71));
        assert_eq!(
            client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
            snapshot_result(SECOND_REQUEST_ID)
        );
    } else {
        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.submit_workbench_setup(request, deadline),
            WorkbenchSetupSubmission::DefinitelyNotSubmitted(
                WorkbenchSetupNotSubmitted::ConnectionUnavailable
            )
        ));
    }
    drop(client);
    server.finish()?;
    Ok(actual)
}
