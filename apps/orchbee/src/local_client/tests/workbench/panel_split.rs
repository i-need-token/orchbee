use super::*;
use crate::local_client::workbench::panel_split::{
    WorkbenchPanelSplitCommit, WorkbenchPanelSplitOutcomeUnknownReason,
    WorkbenchPanelSplitPrecommit, WorkbenchPanelSplitRequest, WorkbenchPanelSplitSubmission,
};

#[test]
fn panel_split_preserves_every_compatible_precommit_and_keeps_the_carrier() -> TestResult {
    // The server-reachable precommit set for panel.split, derived from
    // `begin_terminal_panel_split`: the grant check denies, the request
    // ledger conflicts or fills, the navigation plan rejects, the
    // aggregate-close fence reports the Tab busy, and the worker joint can be
    // unavailable. Rejection revisions pass through unvalidated — they carry
    // the authority's current revision, not the command's successor.
    let mut cases = vec![(
        WorkbenchCommandOutcome::Denied {},
        WorkbenchPanelSplitPrecommit::Denied,
    )];
    for reason in [
        WorkbenchRejectionReason::SiteMismatch,
        WorkbenchRejectionReason::RevisionMismatch,
        WorkbenchRejectionReason::TerminalContextUnavailable,
        WorkbenchRejectionReason::TabUnavailable,
        WorkbenchRejectionReason::PanelUnavailable,
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
            WorkbenchPanelSplitPrecommit::Rejected {
                revision: WorkbenchRevision::new(41),
                reason,
            },
        ));
    }
    for reason in [
        WorkbenchCommandUnavailableReason::RequestLedgerFull,
        WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        WorkbenchCommandUnavailableReason::TerminalBusy,
        WorkbenchCommandUnavailableReason::ServerBusy,
        WorkbenchCommandUnavailableReason::ServiceUnavailable,
    ] {
        cases.push((
            WorkbenchCommandOutcome::Unavailable { reason },
            WorkbenchPanelSplitPrecommit::Unavailable { reason },
        ));
    }
    for (outcome, expected) in cases {
        let server_outcome = outcome.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 61)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(panel_split_command(7))
            );
            write_workbench_response(
                &mut stream,
                &command_response(FIRST_REQUEST_ID, server_outcome),
            )
            .map_err(io::Error::other)?;
            // A definite precommit result never touched the ledger state the
            // carrier guards: the same session must still answer a read.
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
        match client.submit_workbench_panel_split(
            WorkbenchPanelSplitRequest::try_from(panel_split_command(7)).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        ) {
            WorkbenchPanelSplitSubmission::DefinitePrecommit(actual) => {
                assert_eq!(actual, expected);
            }
            other => panic!("expected definite precommit result, received {other:?}"),
        }
        assert_eq!(client.generation().map(NonZeroU64::get), Some(61));
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
fn panel_split_preserves_pending_and_every_created_terminal_outcome() -> TestResult {
    // The committed set the authority can emit for panel.split: the pending
    // receipt plus the five settlement outcomes its result table prebuilds.
    let mut outcomes = vec![panel_split_pending_outcome(8)];
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
        outcomes.push(panel_split_created_outcome(8, terminal_outcome));
    }
    for outcome in outcomes {
        let server_outcome = outcome.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 62)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(panel_split_command(7))
            );
            write_workbench_response(
                &mut stream,
                &command_response(FIRST_REQUEST_ID, server_outcome),
            )
            .map_err(io::Error::other)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let actual = client.submit_workbench_panel_split(
            WorkbenchPanelSplitRequest::try_from(panel_split_command(7)).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        match (outcome, actual) {
            (
                WorkbenchCommandOutcome::PanelSplitPending { .. },
                WorkbenchPanelSplitSubmission::Committed(WorkbenchPanelSplitCommit::Pending {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                }),
            ) => assert_eq!(
                (
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                ),
                (
                    WorkbenchRevision::new(8),
                    context_id(30),
                    WorkbenchTabId::parse(&id(31)).unwrap(),
                    WorkbenchPanelId::parse(&id(32)).unwrap(),
                    WorkbenchPanelId::parse(&id(33)).unwrap(),
                    orchbee_workbench_protocol::WorkbenchSplitId::parse(&id(34)).unwrap(),
                    orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(35)).unwrap(),
                    orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(36)).unwrap(),
                    TerminalPresentationId::parse(&id(37)).unwrap(),
                )
            ),
            (
                WorkbenchCommandOutcome::PanelSplitCreated {
                    terminal_outcome: expected_terminal,
                    ..
                },
                WorkbenchPanelSplitSubmission::Committed(WorkbenchPanelSplitCommit::Created {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    terminal_outcome,
                    ..
                }),
            ) => {
                assert_eq!(revision, WorkbenchRevision::new(8));
                assert_eq!(terminal_context_id, context_id(30));
                assert_eq!(tab_id, WorkbenchTabId::parse(&id(31)).unwrap());
                assert_eq!(panel_id, WorkbenchPanelId::parse(&id(32)).unwrap());
                assert_eq!(terminal_outcome, expected_terminal);
            }
            other => panic!("unexpected panel.split classification {other:?}"),
        }
        assert_eq!(client.generation().map(NonZeroU64::get), Some(62));
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn panel_split_retires_for_every_incompatible_kind_or_reason() -> TestResult {
    // Everything the adapter cannot truthfully classify is an unknown outcome
    // that retires the carrier: unreachable rejection/unavailable reasons for
    // panel.split and every other action's outcome kind. `require_retirement`
    // inside `invalid_split` makes the server prove the connection is
    // released without a second submission.
    for outcome in [
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
            reason: WorkbenchRejectionReason::WorkspaceDirectoryConflict,
        },
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
        },
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::DirectoryUnavailable,
        },
        WorkbenchCommandOutcome::ProjectCreated {
            revision: WorkbenchRevision::new(8),
            project_id: WorkbenchProjectId::parse(&id(40)).unwrap(),
        },
        WorkbenchCommandOutcome::WorkspaceCreated {
            revision: WorkbenchRevision::new(8),
            workspace_id: WorkbenchWorkspaceId::parse(&id(41)).unwrap(),
            terminal_context_id: context_id(30),
        },
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
            revision: WorkbenchRevision::new(8),
            directory_id: orchbee_workbench_protocol::WorkbenchDirectoryId::parse(&id(42)).unwrap(),
            worker_id: WorkerId::parse(&id(43)).unwrap(),
        },
        WorkbenchCommandOutcome::Renamed {
            revision: WorkbenchRevision::new(8),
        },
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(7),
        },
        WorkbenchCommandOutcome::TabMoved {
            revision: WorkbenchRevision::new(8),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        },
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id: workbench_id(&id(44)),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            accepted_revision: WorkbenchRevision::new(7),
        },
        WorkbenchCommandOutcome::TabCreatePending {
            revision: WorkbenchRevision::new(8),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            presentation_id: TerminalPresentationId::parse(&id(37)).unwrap(),
        },
        WorkbenchCommandOutcome::TabCreated {
            revision: WorkbenchRevision::new(8),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            presentation_id: TerminalPresentationId::parse(&id(37)).unwrap(),
            terminal_outcome: WorkbenchTerminalOutcome::Started {
                geometry: terminal_geometry(),
            },
        },
        WorkbenchCommandOutcome::PanelCloseAccepted {
            close_request_id: workbench_id(&id(45)),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            accepted_revision: WorkbenchRevision::new(7),
        },
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(8),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            target_panel_id: WorkbenchPanelId::parse(&id(33)).unwrap(),
        },
    ] {
        let submission = invalid_split(7, command_response(FIRST_REQUEST_ID, outcome))?;
        assert!(matches!(
            submission,
            WorkbenchPanelSplitSubmission::OutcomeUnknown {
                request_id,
                reason: WorkbenchPanelSplitOutcomeUnknownReason::ActionIncompatible,
            } if request_id.as_str() == FIRST_REQUEST_ID
        ));
    }
    Ok(())
}

#[test]
fn panel_split_retires_for_wrong_revision_identity_or_started_geometry() -> TestResult {
    // Every committed receipt that disagrees with the command's pinned
    // revision or identities is an unknown outcome that retires the carrier.
    let wrong_revision = invalid_split(
        7,
        command_response(FIRST_REQUEST_ID, panel_split_pending_outcome(7)),
    )?;
    assert!(matches!(
        wrong_revision,
        WorkbenchPanelSplitSubmission::OutcomeUnknown {
            reason: WorkbenchPanelSplitOutcomeUnknownReason::RevisionMismatch {
                expected,
                received,
            },
            ..
        } if expected == WorkbenchRevision::new(8) && received == WorkbenchRevision::new(7)
    ));

    let skipped_revision = invalid_split(
        7,
        command_response(FIRST_REQUEST_ID, panel_split_pending_outcome(9)),
    )?;
    assert!(matches!(
        skipped_revision,
        WorkbenchPanelSplitSubmission::OutcomeUnknown {
            reason: WorkbenchPanelSplitOutcomeUnknownReason::RevisionMismatch {
                expected,
                received,
            },
            ..
        } if expected == WorkbenchRevision::new(8) && received == WorkbenchRevision::new(9)
    ));

    let overflow = invalid_split(
        u64::MAX,
        command_response(FIRST_REQUEST_ID, panel_split_pending_outcome(u64::MAX)),
    )?;
    assert!(matches!(
        overflow,
        WorkbenchPanelSplitSubmission::OutcomeUnknown {
            reason: WorkbenchPanelSplitOutcomeUnknownReason::ExpectedRevisionOverflow {
                expected,
            },
            ..
        } if expected == WorkbenchRevision::new(u64::MAX)
    ));

    let mut wrong_context = panel_split_pending_outcome(8);
    let WorkbenchCommandOutcome::PanelSplitPending {
        terminal_context_id,
        ..
    } = &mut wrong_context
    else {
        unreachable!()
    };
    *terminal_context_id = context_id(99);
    assert!(matches!(
        invalid_split(7, command_response(FIRST_REQUEST_ID, wrong_context))?,
        WorkbenchPanelSplitSubmission::OutcomeUnknown {
            reason: WorkbenchPanelSplitOutcomeUnknownReason::TerminalContextMismatch {
                expected,
                received,
            },
            ..
        } if expected == context_id(30) && received == context_id(99)
    ));

    let mut wrong_tab = panel_split_pending_outcome(8);
    let WorkbenchCommandOutcome::PanelSplitPending { tab_id, .. } = &mut wrong_tab else {
        unreachable!()
    };
    *tab_id = WorkbenchTabId::parse(&id(99)).unwrap();
    assert!(matches!(
        invalid_split(7, command_response(FIRST_REQUEST_ID, wrong_tab))?,
        WorkbenchPanelSplitSubmission::OutcomeUnknown {
            reason: WorkbenchPanelSplitOutcomeUnknownReason::TabMismatch { expected, received },
            ..
        } if expected == WorkbenchTabId::parse(&id(31)).unwrap()
            && received == WorkbenchTabId::parse(&id(99)).unwrap()
    ));

    let mut wrong_panel = panel_split_pending_outcome(8);
    let WorkbenchCommandOutcome::PanelSplitPending { panel_id, .. } = &mut wrong_panel else {
        unreachable!()
    };
    *panel_id = WorkbenchPanelId::parse(&id(99)).unwrap();
    assert!(matches!(
        invalid_split(7, command_response(FIRST_REQUEST_ID, wrong_panel))?,
        WorkbenchPanelSplitSubmission::OutcomeUnknown {
            reason: WorkbenchPanelSplitOutcomeUnknownReason::PanelMismatch {
                expected,
                received,
            },
            ..
        } if expected == WorkbenchPanelId::parse(&id(32)).unwrap()
            && received == WorkbenchPanelId::parse(&id(99)).unwrap()
    ));

    let wrong_geometry = panel_split_created_outcome(
        8,
        WorkbenchTerminalOutcome::Started {
            geometry: WorkbenchTerminalGeometry::new(81, 24, 8, 16)?,
        },
    );
    assert!(matches!(
        invalid_split(7, command_response(FIRST_REQUEST_ID, wrong_geometry))?,
        WorkbenchPanelSplitSubmission::OutcomeUnknown {
            reason: WorkbenchPanelSplitOutcomeUnknownReason::StartedGeometryMismatch {
                expected,
                received,
            },
            ..
        } if expected == terminal_geometry()
            && received == WorkbenchTerminalGeometry::new(81, 24, 8, 16)?
    ));
    Ok(())
}

#[test]
fn panel_split_retires_on_wrong_response_kind_or_correlation() -> TestResult {
    for response in [
        WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID)),
        WorkbenchResponse::Publication(publication_result(
            FIRST_REQUEST_ID,
            WorkbenchPublicationOutcome::Denied {},
        )),
    ]
    .into_iter()
    .chain(close_control_responses())
    {
        let submission = invalid_split(7, response)?;
        assert!(matches!(
            submission,
            WorkbenchPanelSplitSubmission::OutcomeUnknown {
                reason: WorkbenchPanelSplitOutcomeUnknownReason::UnexpectedResponse,
                ..
            }
        ));
    }

    let wrong_correlation = invalid_split(
        7,
        command_response(SECOND_REQUEST_ID, WorkbenchCommandOutcome::Denied {}),
    )?;
    assert!(matches!(
        wrong_correlation,
        WorkbenchPanelSplitSubmission::OutcomeUnknown {
            request_id,
            reason: WorkbenchPanelSplitOutcomeUnknownReason::CorrelationMismatch {
                expected,
                received,
            },
        } if request_id.as_str() == FIRST_REQUEST_ID
            && expected.as_str() == FIRST_REQUEST_ID
            && received.as_str() == SECOND_REQUEST_ID
    ));
    Ok(())
}

fn invalid_split(
    baseline: u64,
    response: WorkbenchResponse,
) -> Result<WorkbenchPanelSplitSubmission, Box<dyn std::error::Error>> {
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 63)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(panel_split_command(baseline))
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let result = client.submit_workbench_panel_split(
        WorkbenchPanelSplitRequest::try_from(panel_split_command(baseline)).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert_eq!(client.generation(), None);
    server.finish()?;
    Ok(result)
}

fn panel_split_pending_outcome(revision: u64) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelSplitPending {
        revision: WorkbenchRevision::new(revision),
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
        new_panel_id: WorkbenchPanelId::parse(&id(33)).unwrap(),
        split_id: orchbee_workbench_protocol::WorkbenchSplitId::parse(&id(34)).unwrap(),
        first_slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(35)).unwrap(),
        second_slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(36)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(37)).unwrap(),
    }
}

fn panel_split_created_outcome(
    revision: u64,
    terminal_outcome: WorkbenchTerminalOutcome,
) -> WorkbenchCommandOutcome {
    let WorkbenchCommandOutcome::PanelSplitPending {
        revision: _,
        terminal_context_id,
        tab_id,
        panel_id,
        new_panel_id,
        split_id,
        first_slot_id,
        second_slot_id,
        presentation_id,
    } = panel_split_pending_outcome(revision)
    else {
        unreachable!()
    };
    WorkbenchCommandOutcome::PanelSplitCreated {
        revision: WorkbenchRevision::new(revision),
        terminal_context_id,
        tab_id,
        panel_id,
        new_panel_id,
        split_id,
        first_slot_id,
        second_slot_id,
        presentation_id,
        terminal_outcome,
    }
}

fn panel_split_command(expected_revision: u64) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        expected_revision,
        WorkbenchAction::PanelSplit {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            direction: orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Right,
            label: workbench_label(),
            geometry: terminal_geometry(),
            launch: None,
        },
    )
}
