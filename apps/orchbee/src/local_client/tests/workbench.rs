#[cfg(windows)]
mod client_view;
#[cfg(windows)]
mod layout;
use super::*;
mod creation_placement;
mod launch_profile;
mod panel_close;
mod panel_move;
mod panel_split;
mod tab_close;
mod tab_move;
mod tab_rename;
#[cfg(any(target_os = "linux", windows))]
mod workspace_close;
mod workspace_group_close;
mod workspace_setup;
mod worktree_operation;
#[cfg(windows)]
mod worktree_query;
use crate::local_client::{
    SystemMonotonicClock,
    workbench::{
        PublicationReadError, PublicationReadFailure, SnapshotReadError, SnapshotReadFailure,
        WorkbenchTabCreateCommit, WorkbenchTabCreateNotSubmitted,
        WorkbenchTabCreateOutcomeUnknownReason, WorkbenchTabCreatePrecommit,
        WorkbenchTabCreateRequest, WorkbenchTabCreateSubmission,
    },
};
use orchbee_workbench_protocol::{
    ContractError, LaunchProfileId, MAXIMUM_ENCODED_INTENT_BYTES,
    MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES, Navigation, PanelNode, Presentation,
    PresentationBinding, ProtocolVersion, PublicationReadIntent, SiteId, Snapshot,
    SnapshotReadIntent, Tab, TerminalContext, TerminalId, TerminalPresentationId, WorkbenchAction,
    WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchLabel, WorkbenchPanelId, WorkbenchProjectId,
    WorkbenchPublicationDocument, WorkbenchPublicationOutcome, WorkbenchPublicationResult,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchSnapshotUnavailableReason, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchTerminalFailureReason, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
    WorkbenchTerminalUnavailableReason, WorkbenchWireError, WorkbenchWorkspaceId, WorkerId,
    read_workbench_request, write_workbench_request, write_workbench_response,
};

#[test]
fn tab_create_request_is_an_opaque_action_specific_conversion() {
    let accepted = tab_create_command(FIRST_REQUEST_ID, 7);
    assert!(WorkbenchTabCreateRequest::try_from(accepted).is_ok());

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
        WorkbenchAction::TabRename {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            label: workbench_label(),
        },
    ] {
        let command = workbench_command(FIRST_REQUEST_ID, 7, action);
        let expected = command.clone();
        assert_eq!(
            WorkbenchTabCreateRequest::try_from(command)
                .expect_err("non-tab action must be returned unchanged"),
            expected
        );
    }
}

#[test]
fn tab_create_preserves_every_compatible_precommit_on_the_same_carrier() -> TestResult {
    let mut cases = vec![
        (
            WorkbenchCommandOutcome::Denied {},
            WorkbenchTabCreatePrecommit::Denied,
        ),
        // The profile-selector resolutions are definite precommits too: the
        // carrier stays admitted for the next command.
        (
            WorkbenchCommandOutcome::LaunchProfileUnavailable {},
            WorkbenchTabCreatePrecommit::ProfileUnavailable,
        ),
        (
            WorkbenchCommandOutcome::LaunchProfileAmbiguous {
                candidate_ids: vec![
                    LaunchProfileId::parse(&format!("{}:1", id(20))).unwrap(),
                    LaunchProfileId::parse(&format!("{}:2", id(20))).unwrap(),
                ],
            },
            WorkbenchTabCreatePrecommit::ProfileAmbiguous {
                candidate_ids: vec![
                    LaunchProfileId::parse(&format!("{}:1", id(20))).unwrap(),
                    LaunchProfileId::parse(&format!("{}:2", id(20))).unwrap(),
                ],
            },
        ),
    ];
    for reason in [
        WorkbenchCommandUnavailableReason::RequestLedgerFull,
        WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        WorkbenchCommandUnavailableReason::ServerBusy,
        WorkbenchCommandUnavailableReason::ServiceUnavailable,
    ] {
        cases.push((
            WorkbenchCommandOutcome::Unavailable { reason },
            WorkbenchTabCreatePrecommit::Unavailable { reason },
        ));
    }
    for reason in [
        WorkbenchRejectionReason::SiteMismatch,
        WorkbenchRejectionReason::RevisionMismatch,
        WorkbenchRejectionReason::TerminalContextUnavailable,
        WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
        WorkbenchRejectionReason::ResourceLimit,
        WorkbenchRejectionReason::IdentityExhausted,
        WorkbenchRejectionReason::RevisionExhausted,
        WorkbenchRejectionReason::RequestIdentityConflict,
    ] {
        cases.push((
            WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(41),
                reason,
            },
            WorkbenchTabCreatePrecommit::Rejected {
                revision: WorkbenchRevision::new(41),
                reason,
            },
        ));
    }
    let server_cases = cases.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 40)?;
        for (index, (outcome, _expected)) in server_cases.into_iter().enumerate() {
            let request_id = id(100 + index as u128);
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(tab_create_command(&request_id, 7))
            );
            write_workbench_response(&mut stream, &command_response(&request_id, outcome))
                .map_err(io::Error::other)?;
        }
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
    for (index, (_outcome, expected)) in cases.into_iter().enumerate() {
        let request_id = id(100 + index as u128);
        let request = WorkbenchTabCreateRequest::try_from(tab_create_command(&request_id, 7))
            .expect("tab.create fixture");
        match client.submit_workbench_tab_create(
            request,
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        ) {
            WorkbenchTabCreateSubmission::DefinitePrecommit(actual) => {
                assert_eq!(actual, expected);
            }
            other => panic!("expected definite precommit result, received {other:?}"),
        }
    }
    assert_eq!(
        client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
        snapshot_result(SECOND_REQUEST_ID)
    );
    assert_eq!(client.generation().map(NonZeroU64::get), Some(40));
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn tab_create_preserves_every_compatible_committed_variant() -> TestResult {
    let revision = WorkbenchRevision::new(8);
    let terminal_context_id = context_id(30);
    let tab_id = WorkbenchTabId::parse(&id(31))?;
    let panel_id = WorkbenchPanelId::parse(&id(32))?;
    let presentation_id = TerminalPresentationId::parse(&id(33))?;
    let mut cases = vec![(
        WorkbenchCommandOutcome::TabCreatePending {
            revision,
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: presentation_id.clone(),
        },
        WorkbenchTabCreateCommit::Pending {
            revision,
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: presentation_id.clone(),
        },
    )];
    for terminal_outcome in [
        WorkbenchTerminalOutcome::Started {
            geometry: terminal_geometry(),
        },
        WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        },
        WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::ServerBusy,
        },
        WorkbenchTerminalOutcome::Failed {
            reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
        },
        WorkbenchTerminalOutcome::OutcomeUnknown {},
    ] {
        cases.push((
            WorkbenchCommandOutcome::TabCreated {
                revision,
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: presentation_id.clone(),
                terminal_outcome: terminal_outcome.clone(),
            },
            WorkbenchTabCreateCommit::Created {
                revision,
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: presentation_id.clone(),
                terminal_outcome,
            },
        ));
    }
    let server_cases = cases.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 41)?;
        for (index, (outcome, _expected)) in server_cases.into_iter().enumerate() {
            let request_id = id(200 + index as u128);
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(tab_create_command(&request_id, 7))
            );
            write_workbench_response(&mut stream, &command_response(&request_id, outcome))
                .map_err(io::Error::other)?;
        }
        Ok(())
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    for (index, (_outcome, expected)) in cases.into_iter().enumerate() {
        let request_id = id(200 + index as u128);
        let request = WorkbenchTabCreateRequest::try_from(tab_create_command(&request_id, 7))
            .expect("tab.create fixture");
        match client.submit_workbench_tab_create(
            request,
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        ) {
            WorkbenchTabCreateSubmission::Committed(actual) => assert_eq!(actual, expected),
            other => panic!("expected committed result, received {other:?}"),
        }
    }
    assert_eq!(client.generation().map(NonZeroU64::get), Some(41));
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn tab_create_retires_for_every_action_incompatible_outcome() -> TestResult {
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
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(7),
            reason: WorkbenchRejectionReason::ProjectUnavailable,
        },
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(7),
            reason: WorkbenchRejectionReason::WorkspaceUnavailable,
        },
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(7),
            reason: WorkbenchRejectionReason::TabUnavailable,
        },
        WorkbenchCommandOutcome::ProjectCreated {
            revision: WorkbenchRevision::new(8),
            project_id: WorkbenchProjectId::parse(&id(34))?,
        },
        WorkbenchCommandOutcome::WorkspaceCreated {
            revision: WorkbenchRevision::new(8),
            workspace_id: WorkbenchWorkspaceId::parse(&id(35))?,
            terminal_context_id: context_id(30),
        },
        WorkbenchCommandOutcome::Renamed {
            revision: WorkbenchRevision::new(8),
        },
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(7),
        },
    ] {
        let submission = invalid_typed_submission(
            tab_create_command(FIRST_REQUEST_ID, 7),
            command_response(FIRST_REQUEST_ID, outcome),
        )?;
        assert!(matches!(
            submission,
            WorkbenchTabCreateSubmission::OutcomeUnknown {
                request_id,
                reason: WorkbenchTabCreateOutcomeUnknownReason::ActionIncompatible,
            } if request_id.as_str() == FIRST_REQUEST_ID
        ));
    }
    Ok(())
}

#[test]
fn tab_create_retires_for_wrong_committed_fields_and_revision_overflow() -> TestResult {
    let pending = |revision, terminal_context_id| WorkbenchCommandOutcome::TabCreatePending {
        revision,
        terminal_context_id,
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(33)).unwrap(),
    };

    let wrong_revision = invalid_typed_submission(
        tab_create_command(FIRST_REQUEST_ID, 7),
        command_response(
            FIRST_REQUEST_ID,
            pending(WorkbenchRevision::new(9), context_id(30)),
        ),
    )?;
    assert!(matches!(
        wrong_revision,
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            reason: WorkbenchTabCreateOutcomeUnknownReason::CommittedRevisionMismatch {
                expected,
                received,
            },
            ..
        } if expected == WorkbenchRevision::new(8) && received == WorkbenchRevision::new(9)
    ));

    let wrong_context = invalid_typed_submission(
        tab_create_command(FIRST_REQUEST_ID, 7),
        command_response(
            FIRST_REQUEST_ID,
            pending(WorkbenchRevision::new(8), context_id(99)),
        ),
    )?;
    assert!(matches!(
        wrong_context,
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            reason: WorkbenchTabCreateOutcomeUnknownReason::TerminalContextMismatch {
                expected,
                received,
            },
            ..
        } if expected == context_id(30) && received == context_id(99)
    ));

    let wrong_geometry = invalid_typed_submission(
        tab_create_command(FIRST_REQUEST_ID, 7),
        command_response(
            FIRST_REQUEST_ID,
            WorkbenchCommandOutcome::TabCreated {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context_id(30),
                tab_id: WorkbenchTabId::parse(&id(31))?,
                panel_id: WorkbenchPanelId::parse(&id(32))?,
                presentation_id: TerminalPresentationId::parse(&id(33))?,
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: WorkbenchTerminalGeometry::new(81, 24, 8, 16)?,
                },
            },
        ),
    )?;
    assert!(matches!(
        wrong_geometry,
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            reason: WorkbenchTabCreateOutcomeUnknownReason::StartedGeometryMismatch {
                expected,
                received,
            },
            ..
        } if expected == terminal_geometry()
            && received == WorkbenchTerminalGeometry::new(81, 24, 8, 16)?
    ));

    let overflow = invalid_typed_submission(
        tab_create_command(FIRST_REQUEST_ID, u64::MAX),
        command_response(
            FIRST_REQUEST_ID,
            pending(WorkbenchRevision::new(u64::MAX), context_id(30)),
        ),
    )?;
    assert!(matches!(
        overflow,
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            reason: WorkbenchTabCreateOutcomeUnknownReason::ExpectedRevisionOverflow { expected },
            ..
        } if expected == WorkbenchRevision::new(u64::MAX)
    ));
    Ok(())
}

#[test]
fn tab_create_retires_on_wrong_response_kind_or_correlation() -> TestResult {
    let wrong_kind = invalid_typed_submission(
        tab_create_command(FIRST_REQUEST_ID, 7),
        WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID)),
    )?;
    assert!(matches!(
        wrong_kind,
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            reason: WorkbenchTabCreateOutcomeUnknownReason::UnexpectedResponse,
            ..
        }
    ));

    let wrong_correlation = invalid_typed_submission(
        tab_create_command(FIRST_REQUEST_ID, 7),
        command_response(SECOND_REQUEST_ID, WorkbenchCommandOutcome::Denied {}),
    )?;
    assert!(matches!(
        wrong_correlation,
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            request_id,
            reason: WorkbenchTabCreateOutcomeUnknownReason::CorrelationMismatch {
                expected,
                received,
            },
        } if request_id.as_str() == FIRST_REQUEST_ID
            && expected.as_str() == FIRST_REQUEST_ID
            && received.as_str() == SECOND_REQUEST_ID
    ));
    Ok(())
}

#[test]
fn tab_create_retires_on_malformed_duplicate_oversized_or_truncated_responses() -> TestResult {
    for invalid in [
        InvalidCommandResponse::Malformed,
        InvalidCommandResponse::DuplicateKey,
        InvalidCommandResponse::Oversized,
        InvalidCommandResponse::Truncated,
    ] {
        let command = tab_create_command(FIRST_REQUEST_ID, 7);
        let expected = command.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 42)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(expected)
            );
            match invalid {
                InvalidCommandResponse::Malformed => {
                    write_workbench_header(&mut stream, 9, 0)?;
                }
                InvalidCommandResponse::DuplicateKey => {
                    let payload = format!(
                        "{{\"kind\":\"workbench.command.result\",\"protocolVersion\":1,\"requestId\":\"{FIRST_REQUEST_ID}\",\"requestId\":\"{FIRST_REQUEST_ID}\",\"outcome\":{{\"kind\":\"denied\"}}}}"
                    );
                    write_workbench_header(&mut stream, 4, payload.len())?;
                    stream.write_all(payload.as_bytes())?;
                }
                InvalidCommandResponse::Oversized => {
                    write_workbench_header(&mut stream, 4, MAXIMUM_ENCODED_INTENT_BYTES + 1)?;
                }
                InvalidCommandResponse::Truncated => {
                    write_workbench_header(&mut stream, 4, 10)?;
                    stream.write_all(b"short")?;
                    stream.shutdown(std::net::Shutdown::Write)?;
                }
            }
            require_retirement(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let request = WorkbenchTabCreateRequest::try_from(command).expect("tab.create fixture");
        let submission = client.submit_workbench_tab_create(
            request,
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        match invalid {
            InvalidCommandResponse::Malformed => assert!(matches!(
                submission,
                WorkbenchTabCreateSubmission::OutcomeUnknown {
                    reason: WorkbenchTabCreateOutcomeUnknownReason::Response(
                        WorkbenchWireError::InvalidHeader
                    ),
                    ..
                }
            )),
            InvalidCommandResponse::DuplicateKey => assert!(matches!(
                submission,
                WorkbenchTabCreateSubmission::OutcomeUnknown {
                    reason: WorkbenchTabCreateOutcomeUnknownReason::Response(
                        WorkbenchWireError::Contract(ContractError::InvalidDocument)
                    ),
                    ..
                }
            )),
            InvalidCommandResponse::Oversized => assert!(matches!(
                submission,
                WorkbenchTabCreateSubmission::OutcomeUnknown {
                    reason: WorkbenchTabCreateOutcomeUnknownReason::Response(
                        WorkbenchWireError::Contract(ContractError::EncodedValueTooLarge { .. })
                    ),
                    ..
                }
            )),
            InvalidCommandResponse::Truncated => assert!(matches!(
                submission,
                WorkbenchTabCreateSubmission::OutcomeUnknown {
                    reason: WorkbenchTabCreateOutcomeUnknownReason::Response(
                        WorkbenchWireError::Io(error)
                    ),
                    ..
                } if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                )
            )),
        }
        assert_eq!(client.generation(), None);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn tab_create_prewrite_deadlines_are_definite_and_do_not_change_the_carrier() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 43)?;
        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(deferred_create(SECOND_REQUEST_ID)?),
        )?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(create_denied(SECOND_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;

    let request = WorkbenchTabCreateRequest::try_from(tab_create_command(FIRST_REQUEST_ID, 7))
        .expect("tab.create fixture");
    let deadline = client.operation_clock().now();
    assert!(matches!(
        client.submit_workbench_tab_create(request, deadline),
        WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(
            WorkbenchTabCreateNotSubmitted::RequestNotSubmitted(error)
        ) if error.kind() == io::ErrorKind::TimedOut
    ));

    client.set_submission_deadline(Some(client.operation_clock().now()));
    let request = WorkbenchTabCreateRequest::try_from(tab_create_command(FIRST_REQUEST_ID, 7))
        .expect("tab.create fixture");
    assert!(matches!(
        client.submit_workbench_tab_create(
            request,
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        ),
        WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(
            WorkbenchTabCreateNotSubmitted::RequestNotSubmitted(error)
        ) if error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation().map(NonZeroU64::get), Some(43));

    client.set_submission_deadline(None);
    assert_eq!(
        client.submit_terminal_create(deferred_create(SECOND_REQUEST_ID)?)?,
        create_denied(SECOND_REQUEST_ID)?
    );
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn tab_create_partial_write_deadline_is_unknown_and_retires() -> TestResult {
    let command = tab_create_command(FIRST_REQUEST_ID, 7);
    let mut expected_frame = Vec::new();
    write_workbench_request(
        &mut expected_frame,
        &WorkbenchRequest::Command(command.clone()),
    )?;
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 44)?;
        let mut received = Vec::new();
        stream.read_to_end(&mut received)?;
        assert!(!received.is_empty(), "one request write must begin");
        assert!(
            received.len() < expected_frame.len(),
            "the request must remain only partially handed off"
        );
        assert_eq!(received, expected_frame[..received.len()]);
        Ok(())
    })?;
    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
    let deadline = clock.origin + Duration::from_millis(90);
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        SERVER_GUARD_TIMEOUT,
        Box::new(clock),
        IoChunkLimits::new(usize::MAX, 1),
    )?;
    let request = WorkbenchTabCreateRequest::try_from(command).expect("tab.create fixture");
    assert!(matches!(
        client.submit_workbench_tab_create(request, deadline),
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            request_id,
            reason: WorkbenchTabCreateOutcomeUnknownReason::Write(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    drop(client);
    server.finish()?;
    assert!(observations.load(Ordering::Relaxed) >= 10);
    Ok(())
}

#[test]
fn tab_create_read_deadline_is_unknown_and_retires_without_retry() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 45)?;
        let _request = read_workbench_request(&mut stream).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let request = WorkbenchTabCreateRequest::try_from(tab_create_command(FIRST_REQUEST_ID, 7))
        .expect("tab.create fixture");
    assert!(matches!(
        client.submit_workbench_tab_create(
            request,
            client.operation_clock().now() + TEST_REQUEST_TIMEOUT,
        ),
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            request_id,
            reason: WorkbenchTabCreateOutcomeUnknownReason::Response(
                WorkbenchWireError::Io(error)
            ),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    let request = WorkbenchTabCreateRequest::try_from(tab_create_command(SECOND_REQUEST_ID, 7))
        .expect("tab.create fixture");
    assert!(matches!(
        client.submit_workbench_tab_create(
            request,
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        ),
        WorkbenchTabCreateSubmission::DefinitelyNotSubmitted(
            WorkbenchTabCreateNotSubmitted::SessionUnavailable
        )
    ));
    server.finish()?;
    Ok(())
}

#[test]
fn tab_create_completed_response_after_the_fixed_deadline_is_unknown() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 47)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(tab_create_command(FIRST_REQUEST_ID, 7))
        );
        write_workbench_response(
            &mut stream,
            &command_response(FIRST_REQUEST_ID, WorkbenchCommandOutcome::Denied {}),
        )
        .map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
    let deadline = clock.origin + Duration::from_millis(80);
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        SERVER_GUARD_TIMEOUT,
        Box::new(clock),
        IoChunkLimits::UNBOUNDED,
    )?;
    let request = WorkbenchTabCreateRequest::try_from(tab_create_command(FIRST_REQUEST_ID, 7))
        .expect("tab.create fixture");
    assert!(matches!(
        client.submit_workbench_tab_create(request, deadline),
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            request_id,
            reason: WorkbenchTabCreateOutcomeUnknownReason::ResponseDeadline(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    assert_eq!(observations.load(Ordering::Relaxed), 9);
    Ok(())
}

#[test]
fn tab_create_caller_deadline_cannot_extend_the_single_exchange_budget() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 48)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(tab_create_command(FIRST_REQUEST_ID, 7))
        );
        write_workbench_response(
            &mut stream,
            &command_response(FIRST_REQUEST_ID, WorkbenchCommandOutcome::Denied {}),
        )
        .map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
    let caller_deadline = clock.origin + SERVER_GUARD_TIMEOUT;
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        Duration::from_millis(80),
        Box::new(clock),
        IoChunkLimits::UNBOUNDED,
    )?;
    let request = WorkbenchTabCreateRequest::try_from(tab_create_command(FIRST_REQUEST_ID, 7))
        .expect("tab.create fixture");
    assert!(matches!(
        client.submit_workbench_tab_create(request, caller_deadline),
        WorkbenchTabCreateSubmission::OutcomeUnknown {
            request_id,
            reason: WorkbenchTabCreateOutcomeUnknownReason::ResponseDeadline(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    assert_eq!(observations.load(Ordering::Relaxed), 9);
    Ok(())
}

#[test]
fn snapshot_read_supports_partial_io_and_preserves_the_same_mixed_carrier() -> TestResult {
    let expected = snapshot_result(FIRST_REQUEST_ID);
    let server = FakeServer::start(move |stream| {
        let mut stream = ChunkedStream::new(stream, 1, 1);
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 31)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(snapshot_request(FIRST_REQUEST_ID))
        );
        write_workbench_response(&mut stream, &WorkbenchResponse::Snapshot(expected.clone()))
            .map_err(io::Error::other)?;
        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(deferred_create(SECOND_REQUEST_ID)?),
        )?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(create_denied(SECOND_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;

    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        SERVER_GUARD_TIMEOUT,
        Box::new(SystemMonotonicClock),
        IoChunkLimits::new(1, 1),
    )?;
    assert_eq!(
        client.submit_workbench_snapshot_read(snapshot_request(FIRST_REQUEST_ID))?,
        snapshot_result(FIRST_REQUEST_ID)
    );
    assert_eq!(
        client.submit_terminal_create(deferred_create(SECOND_REQUEST_ID)?)?,
        create_denied(SECOND_REQUEST_ID)?
    );
    assert_eq!(client.generation().map(NonZeroU64::get), Some(31));
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn snapshot_read_retires_on_wrong_kind_or_correlation() -> TestResult {
    for response in [
        WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: workbench_id(FIRST_REQUEST_ID),
            outcome: WorkbenchCommandOutcome::Denied {},
        }),
        WorkbenchResponse::Snapshot(snapshot_result(SECOND_REQUEST_ID)),
    ] {
        let expected_wrong_kind = matches!(response, WorkbenchResponse::Command(_));
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 32)?;
            let _request = read_workbench_request(&mut stream).map_err(io::Error::other)?;
            write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
            require_retirement(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let error = client
            .submit_workbench_snapshot_read(snapshot_request(FIRST_REQUEST_ID))
            .expect_err("invalid response classification must fail");
        if expected_wrong_kind {
            assert!(matches!(
                error,
                SnapshotReadError::Failed {
                    reason: SnapshotReadFailure::UnexpectedResponse,
                    ..
                }
            ));
        } else {
            assert!(matches!(
                error,
                SnapshotReadError::Failed {
                    request_id,
                    reason: SnapshotReadFailure::CorrelationMismatch { expected, received },
                } if request_id.as_str() == FIRST_REQUEST_ID
                    && expected.as_str() == FIRST_REQUEST_ID
                    && received.as_str() == SECOND_REQUEST_ID
            ));
        }
        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID)),
            Err(SnapshotReadError::SessionUnavailable)
        ));
        server.finish()?;
    }
    Ok(())
}

#[test]
fn snapshot_read_retires_on_malformed_invalid_oversized_or_truncated_frames() -> TestResult {
    for invalid in [
        InvalidResponse::Malformed,
        InvalidResponse::InvalidSnapshot,
        InvalidResponse::Oversized,
        InvalidResponse::Truncated,
    ] {
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 33)?;
            let _request = read_workbench_request(&mut stream).map_err(io::Error::other)?;
            match invalid {
                InvalidResponse::Malformed => {
                    write_workbench_header(&mut stream, 9, 0)?;
                }
                InvalidResponse::InvalidSnapshot => {
                    let mut response = snapshot_result(FIRST_REQUEST_ID);
                    let WorkbenchSnapshotOutcome::Available { snapshot } = &mut response.outcome
                    else {
                        unreachable!("fixture snapshot")
                    };
                    snapshot.terminal_contexts.clear();
                    write_raw_snapshot_response(&mut stream, &response)?;
                }
                InvalidResponse::Oversized => {
                    write_workbench_header(
                        &mut stream,
                        3,
                        MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES + 1,
                    )?;
                }
                InvalidResponse::Truncated => {
                    write_workbench_header(&mut stream, 3, 10)?;
                    stream.write_all(b"short")?;
                    stream.shutdown(std::net::Shutdown::Write)?;
                }
            }
            require_retirement(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let error = client
            .submit_workbench_snapshot_read(snapshot_request(FIRST_REQUEST_ID))
            .expect_err("invalid Workbench framing must fail");
        match invalid {
            InvalidResponse::Malformed => assert!(matches!(
                error,
                SnapshotReadError::Failed {
                    reason: SnapshotReadFailure::Response(WorkbenchWireError::InvalidHeader),
                    ..
                }
            )),
            InvalidResponse::InvalidSnapshot => assert!(matches!(
                error,
                SnapshotReadError::Failed {
                    reason: SnapshotReadFailure::Response(WorkbenchWireError::Contract(
                        ContractError::InvalidDocument
                    )),
                    ..
                }
            )),
            InvalidResponse::Oversized => assert!(matches!(
                error,
                SnapshotReadError::Failed {
                    reason: SnapshotReadFailure::Response(WorkbenchWireError::Contract(
                        ContractError::EncodedValueTooLarge { .. }
                    )),
                    ..
                }
            )),
            InvalidResponse::Truncated => assert!(matches!(
                error,
                SnapshotReadError::Failed {
                    reason: SnapshotReadFailure::Response(WorkbenchWireError::Io(error)),
                    ..
                } if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                )
            )),
        }
        assert_eq!(client.generation(), None);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn snapshot_read_timeout_retires_without_retry() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 34)?;
        let _request = read_workbench_request(&mut stream).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client =
        LocalClient::connect_with_request_timeout(server.endpoint(), COHORT, TEST_REQUEST_TIMEOUT)?;
    let error = client
        .submit_workbench_snapshot_read(snapshot_request(FIRST_REQUEST_ID))
        .expect_err("silent response must exhaust the request deadline");
    assert!(matches!(
        error,
        SnapshotReadError::Failed {
            request_id,
            reason: SnapshotReadFailure::Response(WorkbenchWireError::Io(error)),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(())
}

#[test]
fn snapshot_partial_write_deadline_retires_the_mixed_carrier() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 36)?;
        let mut received = Vec::new();
        stream.read_to_end(&mut received)?;
        assert_eq!(received, b"OBWP", "only one partial request was handed off");
        Ok(())
    })?;
    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
    let deadline = clock.origin + Duration::from_millis(90);
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        SERVER_GUARD_TIMEOUT,
        Box::new(clock),
        IoChunkLimits::new(usize::MAX, 1),
    )?;
    let error = client
        .submit_workbench_snapshot_read_before(snapshot_request(FIRST_REQUEST_ID), deadline)
        .expect_err("partial snapshot request handoff must remain deadline bounded");
    assert!(matches!(
        error,
        SnapshotReadError::Failed {
            request_id,
            reason: SnapshotReadFailure::Write(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    assert!(matches!(
        client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID)),
        Err(SnapshotReadError::SessionUnavailable)
    ));
    drop(client);
    server.finish()?;
    assert!(observations.load(Ordering::Relaxed) >= 10);
    Ok(())
}

#[test]
fn snapshot_caller_deadline_cannot_extend_the_single_exchange_budget() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 50)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(snapshot_request(FIRST_REQUEST_ID))
        );
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID)),
        )
        .map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let observations = Arc::new(AtomicU64::new(0));
    let clock = SteppingMonotonicClock::new(Duration::from_millis(10), observations.clone());
    let caller_deadline = clock.origin + SERVER_GUARD_TIMEOUT;
    let mut client = LocalClient::connect_with_submission_runtime(
        server.endpoint(),
        COHORT,
        Duration::from_millis(80),
        Box::new(clock),
        IoChunkLimits::UNBOUNDED,
    )?;
    let error = client
        .submit_workbench_snapshot_read_before(snapshot_request(FIRST_REQUEST_ID), caller_deadline)
        .expect_err("ordinary snapshot budget must clamp a later caller deadline");
    assert!(matches!(
        error,
        SnapshotReadError::Failed {
            request_id,
            reason: SnapshotReadFailure::ResponseDeadline(error),
        } if request_id.as_str() == FIRST_REQUEST_ID
            && error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation(), None);
    server.finish()?;
    assert_eq!(observations.load(Ordering::Relaxed), 9);
    Ok(())
}

#[test]
fn snapshot_prewrite_deadline_is_definite_and_keeps_the_carrier() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 35)?;
        require_message(
            read_client_message(&mut stream).map_err(io::Error::other)?,
            ClientMessage::TerminalCreate(deferred_create(SECOND_REQUEST_ID)?),
        )?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalCreateResult(create_denied(SECOND_REQUEST_ID)?),
        )
        .map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let deadline = client.operation_clock().now();
    assert!(matches!(
        client.submit_workbench_snapshot_read_before(snapshot_request(FIRST_REQUEST_ID), deadline),
        Err(SnapshotReadError::RequestNotSubmitted(error))
            if error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation().map(NonZeroU64::get), Some(35));
    assert_eq!(
        client.submit_terminal_create(deferred_create(SECOND_REQUEST_ID)?)?,
        create_denied(SECOND_REQUEST_ID)?
    );
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn publication_read_preserves_every_outcome_on_the_same_carrier() -> TestResult {
    let outcomes = [
        WorkbenchPublicationOutcome::Current {
            revision: WorkbenchRevision::new(7),
        },
        WorkbenchPublicationOutcome::Published {
            publications: vec![WorkbenchPublicationDocument::Snapshot(snapshot())],
        },
        WorkbenchPublicationOutcome::Denied {},
        WorkbenchPublicationOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    ];
    let server_outcomes = outcomes.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 42)?;
        for (index, outcome) in server_outcomes.into_iter().enumerate() {
            let request_id = id(200 + index as u128);
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::PublicationRead(publication_request(&request_id, 6))
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Publication(publication_result(&request_id, outcome)),
            )
            .map_err(io::Error::other)?;
        }
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
    for (index, expected) in outcomes.into_iter().enumerate() {
        let request_id = id(200 + index as u128);
        assert_eq!(
            client.submit_workbench_publication_read(publication_request(&request_id, 6))?,
            publication_result(&request_id, expected)
        );
    }
    assert_eq!(
        client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
        snapshot_result(SECOND_REQUEST_ID)
    );
    assert_eq!(client.generation().map(NonZeroU64::get), Some(42));
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn publication_read_retires_on_wrong_kind_or_correlation() -> TestResult {
    for response in [
        WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID)),
        WorkbenchResponse::Publication(publication_result(
            SECOND_REQUEST_ID,
            WorkbenchPublicationOutcome::Denied {},
        )),
    ] {
        let expected_wrong_kind = matches!(response, WorkbenchResponse::Snapshot(_));
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 43)?;
            let _request = read_workbench_request(&mut stream).map_err(io::Error::other)?;
            write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
            require_retirement(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let error = client
            .submit_workbench_publication_read(publication_request(FIRST_REQUEST_ID, 6))
            .expect_err("invalid response classification must fail");
        if expected_wrong_kind {
            assert!(matches!(
                error,
                PublicationReadError::Failed {
                    reason: PublicationReadFailure::UnexpectedResponse,
                    ..
                }
            ));
        } else {
            assert!(matches!(
                error,
                PublicationReadError::Failed {
                    request_id,
                    reason: PublicationReadFailure::CorrelationMismatch { expected, received },
                } if request_id.as_str() == FIRST_REQUEST_ID
                    && expected.as_str() == FIRST_REQUEST_ID
                    && received.as_str() == SECOND_REQUEST_ID
            ));
        }
        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.submit_workbench_publication_read(publication_request(SECOND_REQUEST_ID, 6)),
            Err(PublicationReadError::SessionUnavailable)
        ));
        server.finish()?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum InvalidResponse {
    Malformed,
    InvalidSnapshot,
    Oversized,
    Truncated,
}

#[derive(Clone, Copy)]
enum InvalidCommandResponse {
    Malformed,
    DuplicateKey,
    Oversized,
    Truncated,
}

fn invalid_typed_submission(
    command: WorkbenchCommand,
    response: WorkbenchResponse,
) -> Result<WorkbenchTabCreateSubmission, Box<dyn std::error::Error>> {
    let expected = command.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 46)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(expected)
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let request = WorkbenchTabCreateRequest::try_from(command).expect("tab.create fixture");
    let submission = client.submit_workbench_tab_create(
        request,
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(submission)
}

fn tab_create_command(request_id: &str, expected_revision: u64) -> WorkbenchCommand {
    workbench_command(
        request_id,
        expected_revision,
        WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: context_id(30),
            label: workbench_label(),
            geometry: terminal_geometry(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    )
}

fn workbench_command(
    request_id: &str,
    expected_revision: u64,
    action: WorkbenchAction,
) -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_id),
        site_id: SiteId::parse(&id(20)).unwrap(),
        expected_revision: WorkbenchRevision::new(expected_revision),
        action,
    }
}

fn command_response(request_id: &str, outcome: WorkbenchCommandOutcome) -> WorkbenchResponse {
    WorkbenchResponse::Command(WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_id),
        outcome,
    })
}

fn workbench_label() -> WorkbenchLabel {
    WorkbenchLabel::parse("2").unwrap()
}

fn terminal_geometry() -> WorkbenchTerminalGeometry {
    WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap()
}

fn snapshot_request(request_id: &str) -> SnapshotReadIntent {
    SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_id),
    }
}

fn snapshot_result(request_id: &str) -> WorkbenchSnapshotResult {
    WorkbenchSnapshotResult {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_id),
        outcome: WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(),
        },
    }
}

fn publication_request(request_id: &str, known_revision: u64) -> PublicationReadIntent {
    PublicationReadIntent {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_id),
        known_revision: WorkbenchRevision::new(known_revision),
    }
}

fn publication_result(
    request_id: &str,
    outcome: WorkbenchPublicationOutcome,
) -> WorkbenchPublicationResult {
    WorkbenchPublicationResult {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_id),
        outcome,
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        navigation: {
            let fixture_context_id = context_id(1);
            let fixture_workspace_id = orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap();
            Navigation {
                project_order: Vec::new(),
                directories: Vec::new(),
                workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                    workspace_id: fixture_workspace_id.clone(),
                    terminal_context_id: fixture_context_id.clone(),
                    project_id: None,
                    primary_directory_id: None,
                    label: "Workspace".into(),
                })
                .chain(Vec::new())
                .collect(),
                workspace_order: Vec::new(),
                projects: Vec::new(),
            }
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(1),
        site_id: SiteId::parse(&id(2)).unwrap(),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            tabs: vec![Tab {
                automatic_label: false,
                group_id: None,
                label: "tab".into(),
                pinned: false,
                root: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&id(3)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            terminal_id: TerminalId::parse(&id(4)).unwrap(),
                            worker_id: WorkerId::parse(&id(5)).unwrap(),
                        }),
                        label: "presentation".into(),
                        presentation_id: TerminalPresentationId::parse(&id(6)).unwrap(),
                        launch_failure: None,
                    },
                },
                tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&id(7)).unwrap(),
            }],
            terminal_context_id: context_id(1),
        }],
    }
}

fn workbench_id(value: &str) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(value).unwrap()
}

fn context_id(value: u128) -> WorkbenchTerminalContextId {
    WorkbenchTerminalContextId::parse(&id(value)).unwrap()
}

fn id(value: u128) -> String {
    format!("{value:032x}")
}

fn write_raw_snapshot_response(
    stream: &mut UnixStream,
    response: &WorkbenchSnapshotResult,
) -> io::Result<()> {
    let payload = serde_json::to_vec(&WorkbenchResponse::Snapshot(response.clone()))?;
    write_workbench_header(stream, 3, payload.len())?;
    stream.write_all(&payload)
}

fn write_workbench_header(stream: &mut UnixStream, kind: u16, length: usize) -> io::Result<()> {
    let length = u32::try_from(length).map_err(io::Error::other)?;
    let mut header = [0_u8; 12];
    header[..4].copy_from_slice(b"OBWP");
    header[4..6].copy_from_slice(&1_u16.to_be_bytes());
    header[6..8].copy_from_slice(&kind.to_be_bytes());
    header[8..].copy_from_slice(&length.to_be_bytes());
    stream.write_all(&header)
}

fn require_retirement(stream: &mut UnixStream) -> io::Result<()> {
    let mut byte = [0_u8; 1];
    match stream.read(&mut byte) {
        Ok(0) => Ok(()),
        Ok(_) => Err(io::Error::other("Client retried after Workbench failure")),
        Err(error) => Err(io::Error::other(format!(
            "Client did not retire after Workbench failure: {error}"
        ))),
    }
}

fn close_control_responses() -> [WorkbenchResponse; 2] {
    use orchbee_workbench_protocol::{
        WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseResumeOutcome,
        WorkbenchTabCloseResumeResult,
    };
    [
        WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(FIRST_REQUEST_ID).unwrap(),
            outcome: WorkbenchTabCloseReadOutcome::NotClosing {},
        }),
        WorkbenchResponse::TabCloseResume(WorkbenchTabCloseResumeResult {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(FIRST_REQUEST_ID).unwrap(),
            site_id: SiteId::parse(&id(29)).unwrap(),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            close_request_id: WorkbenchRequestId::parse(&id(32)).unwrap(),
            outcome: WorkbenchTabCloseResumeOutcome::Denied {},
        }),
    ]
}

#[test]
fn tab_create_retires_close_control_envelopes_as_wrong_response_kind() -> TestResult {
    for response in close_control_responses() {
        assert!(matches!(
            invalid_typed_submission(tab_create_command(FIRST_REQUEST_ID, 7), response)?,
            WorkbenchTabCreateSubmission::OutcomeUnknown {
                reason: WorkbenchTabCreateOutcomeUnknownReason::UnexpectedResponse,
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn tab_create_wake_after_submission_retires_unknown_without_replay() -> TestResult {
    let (submitted, wait_for_submission) = std::sync::mpsc::channel();
    let (wake_read, mut wake_write) = UnixStream::pair()?;
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 51)?;
        let WorkbenchRequest::Command(command) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("swap");
        };
        assert!(matches!(command.action, WorkbenchAction::TabCreate { .. }));
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
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let command = tab_create_command(FIRST_REQUEST_ID, 7);
    let answer = client.submit_workbench_tab_create_with_wake(
        WorkbenchTabCreateRequest::try_from(command).unwrap(),
        client.operation_clock().now() + Duration::from_secs(5),
        Some(wake_read.as_local_io()),
    );
    assert!(matches!(
        answer,
        WorkbenchTabCreateSubmission::OutcomeUnknown { .. }
    ));
    assert!(client.generation().is_none());
    wake.join().unwrap()?;
    drop(client);
    server.finish()?;
    Ok(())
}

mod panel_relocate;

#[cfg(windows)]
mod workspace_metadata;

mod panel_agent;
mod panel_metadata;

mod agent_view;
