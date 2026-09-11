use super::*;
use crate::local_client::workbench::panel_close::{
    WorkbenchPanelCloseAccepted, WorkbenchPanelCloseFailure, WorkbenchPanelCloseNotSubmitted,
    WorkbenchPanelClosePrecommit, WorkbenchPanelCloseRequest, WorkbenchPanelCloseSubmission,
};

fn panel_close_command(base: u64) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        base,
        WorkbenchAction::PanelClose {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
        },
    )
}

fn accepted(canonical: &str, revision: u64) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelCloseAccepted {
        close_request_id: workbench_id(canonical),
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
        accepted_revision: WorkbenchRevision::new(revision),
    }
}

fn submit_panel_close_response(
    base: u64,
    response: WorkbenchResponse,
    retires: bool,
) -> Result<WorkbenchPanelCloseSubmission, Box<dyn std::error::Error>> {
    exchange(
        WorkbenchRequest::Command(panel_close_command(base)),
        response,
        retires,
        |client| {
            client.submit_workbench_panel_close(
                WorkbenchPanelCloseRequest::try_from(panel_close_command(base)).unwrap(),
                client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
            )
        },
    )
}

fn exchange<Output>(
    request: WorkbenchRequest,
    response: WorkbenchResponse,
    retires: bool,
    submit: impl FnOnce(&mut LocalClient) -> Output,
) -> Result<Output, Box<dyn std::error::Error>> {
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 60)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            request
        );
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        if retires {
            require_retirement(&mut stream)
        } else {
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::SnapshotRead(snapshot_request(SECOND_REQUEST_ID))
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(snapshot_result(SECOND_REQUEST_ID)),
            )
            .map_err(io::Error::other)
        }
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let result = submit(&mut client);
    assert_eq!(
        client.generation().map(NonZeroU64::get),
        if retires { None } else { Some(60) }
    );
    if !retires {
        assert_eq!(
            client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
            snapshot_result(SECOND_REQUEST_ID)
        );
    }
    drop(client);
    server.finish()?;
    Ok(result)
}

#[test]
fn panel_close_request_only_admits_panel_close_and_preserves_rejected_command() {
    assert!(WorkbenchPanelCloseRequest::try_from(panel_close_command(7)).is_ok());
    let other = close_command_for_other_action();
    assert_eq!(
        WorkbenchPanelCloseRequest::try_from(other.clone()).unwrap_err(),
        other
    );
}

fn close_command_for_other_action() -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        7,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        },
    )
}

#[test]
fn panel_close_accepts_primary_and_older_alias_receipts_with_exact_fields() -> TestResult {
    for (canonical, revision) in [(FIRST_REQUEST_ID.to_owned(), 7), (id(99), 7), (id(99), 3)] {
        let expected = WorkbenchPanelCloseAccepted {
            close_request_id: workbench_id(&canonical),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31))?,
            panel_id: WorkbenchPanelId::parse(&id(32))?,
            accepted_revision: WorkbenchRevision::new(revision),
        };
        let actual = submit_panel_close_response(
            7,
            command_response(FIRST_REQUEST_ID, accepted(&canonical, revision)),
            false,
        )?;
        assert!(matches!(actual,WorkbenchPanelCloseSubmission::Accepted(value) if value==expected));
    }
    Ok(())
}

#[test]
fn panel_close_retires_maximum_canonical_revision_for_primary_and_alias() -> TestResult {
    for canonical in [FIRST_REQUEST_ID.to_owned(), id(99)] {
        assert!(matches!(
            submit_panel_close_response(
                u64::MAX,
                command_response(FIRST_REQUEST_ID, accepted(&canonical, u64::MAX)),
                true,
            )?,
            WorkbenchPanelCloseSubmission::OutcomeUnknown {
                reason: WorkbenchPanelCloseFailure::AcceptedRevisionMismatch { .. },
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn panel_close_accepts_older_alias_at_maximum_admitted_base() -> TestResult {
    let canonical = id(99);
    assert!(matches!(
        submit_panel_close_response(
            u64::MAX,
            command_response(FIRST_REQUEST_ID, accepted(&canonical, u64::MAX - 1)),
            false,
        )?,
        WorkbenchPanelCloseSubmission::Accepted(value)
            if value.accepted_revision == WorkbenchRevision::new(u64::MAX - 1)
                && value.close_request_id == workbench_id(&canonical)
    ));
    Ok(())
}

#[test]
fn panel_close_preserves_every_action_compatible_precommit_on_the_same_carrier() -> TestResult {
    let mut outcomes = vec![WorkbenchCommandOutcome::Denied {}];
    for reason in [
        WorkbenchRejectionReason::SiteMismatch,
        WorkbenchRejectionReason::RevisionMismatch,
        WorkbenchRejectionReason::TerminalContextUnavailable,
        WorkbenchRejectionReason::TabUnavailable,
        WorkbenchRejectionReason::PanelUnavailable,
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
        WorkbenchCommandUnavailableReason::TerminalBusy,
        WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
        WorkbenchCommandUnavailableReason::ServerBusy,
        WorkbenchCommandUnavailableReason::ServiceUnavailable,
    ] {
        outcomes.push(WorkbenchCommandOutcome::Unavailable { reason });
    }
    for outcome in outcomes {
        let actual = submit_panel_close_response(
            7,
            command_response(FIRST_REQUEST_ID, outcome.clone()),
            false,
        )?;
        match (outcome, actual) {
            (
                WorkbenchCommandOutcome::Denied {},
                WorkbenchPanelCloseSubmission::DefinitePrecommit(
                    WorkbenchPanelClosePrecommit::Denied,
                ),
            ) => {}
            (
                WorkbenchCommandOutcome::Rejected { revision, reason },
                WorkbenchPanelCloseSubmission::DefinitePrecommit(
                    WorkbenchPanelClosePrecommit::Rejected {
                        revision: r,
                        reason: s,
                    },
                ),
            ) => assert_eq!((r, s), (revision, reason)),
            (
                WorkbenchCommandOutcome::Unavailable { reason },
                WorkbenchPanelCloseSubmission::DefinitePrecommit(
                    WorkbenchPanelClosePrecommit::Unavailable { reason: r },
                ),
            ) => assert_eq!(r, reason),
            other => panic!("wrong panel.close precommit classification: {other:?}"),
        }
    }
    Ok(())
}

#[test]
fn panel_close_retires_bad_primary_alias_target_correlation_and_outcome() -> TestResult {
    let mut wrong_context = accepted(FIRST_REQUEST_ID, 7);
    let WorkbenchCommandOutcome::PanelCloseAccepted {
        terminal_context_id,
        ..
    } = &mut wrong_context
    else {
        unreachable!()
    };
    *terminal_context_id = context_id(99);
    let mut wrong_tab = accepted(FIRST_REQUEST_ID, 7);
    let WorkbenchCommandOutcome::PanelCloseAccepted { tab_id, .. } = &mut wrong_tab else {
        unreachable!()
    };
    *tab_id = WorkbenchTabId::parse(&id(99))?;
    let mut wrong_panel = accepted(FIRST_REQUEST_ID, 7);
    let WorkbenchCommandOutcome::PanelCloseAccepted { panel_id, .. } = &mut wrong_panel else {
        unreachable!()
    };
    *panel_id = WorkbenchPanelId::parse(&id(99))?;
    for outcome in [
        accepted(FIRST_REQUEST_ID, 6),
        accepted(&id(99), 8),
        wrong_context,
        wrong_tab,
        wrong_panel,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(7),
        },
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id: workbench_id(FIRST_REQUEST_ID),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            accepted_revision: WorkbenchRevision::new(7),
        },
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(7),
            reason: WorkbenchRejectionReason::IdentityExhausted,
        },
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(7),
            reason: WorkbenchRejectionReason::ProjectUnavailable,
        },
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(7),
            reason: WorkbenchRejectionReason::WorkspaceUnavailable,
        },
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        },
    ] {
        assert!(matches!(
            submit_panel_close_response(7, command_response(FIRST_REQUEST_ID, outcome), true)?,
            WorkbenchPanelCloseSubmission::OutcomeUnknown { .. }
        ));
    }
    for response in [
        command_response(SECOND_REQUEST_ID, accepted(FIRST_REQUEST_ID, 7)),
        WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID)),
    ] {
        assert!(matches!(
            submit_panel_close_response(7, response, true)?,
            WorkbenchPanelCloseSubmission::OutcomeUnknown { .. }
        ));
    }
    Ok(())
}

#[test]
fn panel_close_prewrite_deadline_preserves_the_carrier_and_writes_nothing() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 62)?;
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
    assert!(matches!(
        client.submit_workbench_panel_close(
            WorkbenchPanelCloseRequest::try_from(panel_close_command(7)).unwrap(),
            deadline,
        ),
        WorkbenchPanelCloseSubmission::DefinitelyNotSubmitted(
            WorkbenchPanelCloseNotSubmitted::RequestNotSubmitted(error)
        ) if error.kind() == io::ErrorKind::TimedOut
    ));
    assert_eq!(client.generation().map(NonZeroU64::get), Some(62));
    assert_eq!(
        client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
        snapshot_result(SECOND_REQUEST_ID)
    );
    drop(client);
    server.finish()?;
    Ok(())
}
