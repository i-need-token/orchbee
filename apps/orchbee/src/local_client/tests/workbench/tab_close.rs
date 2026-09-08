use super::*;
mod transport;
use crate::local_client::workbench::{
    WorkbenchTabCloseAccepted, WorkbenchTabClosePrecommit, WorkbenchTabCloseReadError,
    WorkbenchTabCloseRequest, WorkbenchTabCloseResumeSubmission, WorkbenchTabCloseSubmission,
};
use orchbee_workbench_protocol::{
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseReadResult, WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason,
};

#[test]
fn close_request_only_admits_close_and_preserves_rejected_command() {
    assert!(WorkbenchTabCloseRequest::try_from(close_command(7)).is_ok());
    let other = tab_create_command(FIRST_REQUEST_ID, 7);
    assert_eq!(
        WorkbenchTabCloseRequest::try_from(other.clone()).unwrap_err(),
        other
    );
}

#[test]
fn close_accepts_primary_and_older_alias_receipts_with_exact_fields() -> TestResult {
    for (canonical, revision) in [(FIRST_REQUEST_ID.to_owned(), 7), (id(99), 7), (id(99), 3)] {
        let expected = WorkbenchTabCloseAccepted {
            close_request_id: workbench_id(&canonical),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31))?,
            accepted_revision: WorkbenchRevision::new(revision),
        };
        let actual = submit_close_response(
            7,
            command_response(FIRST_REQUEST_ID, accepted(&canonical, revision)),
            false,
        )?;
        assert!(matches!(actual,WorkbenchTabCloseSubmission::Accepted(value) if value==expected));
    }
    Ok(())
}

#[test]
fn close_retires_maximum_canonical_revision_for_primary_and_alias() -> TestResult {
    for canonical in [FIRST_REQUEST_ID.to_owned(), id(99)] {
        assert!(matches!(
            submit_close_response(
                u64::MAX,
                command_response(FIRST_REQUEST_ID, accepted(&canonical, u64::MAX)),
                true,
            )?,
            WorkbenchTabCloseSubmission::OutcomeUnknown {
                reason: crate::local_client::workbench::WorkbenchTabCloseFailure::AcceptedRevisionMismatch { .. },
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn close_accepts_older_alias_at_maximum_admitted_base() -> TestResult {
    let canonical = id(99);
    assert!(matches!(
        submit_close_response(
            u64::MAX,
            command_response(FIRST_REQUEST_ID, accepted(&canonical, u64::MAX - 1)),
            false,
        )?,
        WorkbenchTabCloseSubmission::Accepted(value)
            if value.accepted_revision == WorkbenchRevision::new(u64::MAX - 1)
                && value.close_request_id == workbench_id(&canonical)
    ));
    Ok(())
}

#[test]
fn close_preserves_every_action_compatible_precommit_on_the_same_carrier() -> TestResult {
    let mut outcomes = vec![WorkbenchCommandOutcome::Denied {}];
    for reason in [
        WorkbenchRejectionReason::SiteMismatch,
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
        WorkbenchCommandUnavailableReason::TerminalBusy,
        WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
        WorkbenchCommandUnavailableReason::ServerBusy,
        WorkbenchCommandUnavailableReason::ServiceUnavailable,
    ] {
        outcomes.push(WorkbenchCommandOutcome::Unavailable { reason });
    }
    for outcome in outcomes {
        let actual = submit_close_response(
            7,
            command_response(FIRST_REQUEST_ID, outcome.clone()),
            false,
        )?;
        match (outcome, actual) {
            (
                WorkbenchCommandOutcome::Denied {},
                WorkbenchTabCloseSubmission::DefinitePrecommit(WorkbenchTabClosePrecommit::Denied),
            ) => {}
            (
                WorkbenchCommandOutcome::Rejected { revision, reason },
                WorkbenchTabCloseSubmission::DefinitePrecommit(
                    WorkbenchTabClosePrecommit::Rejected {
                        revision: r,
                        reason: s,
                    },
                ),
            ) => assert_eq!((r, s), (revision, reason)),
            (
                WorkbenchCommandOutcome::Unavailable { reason },
                WorkbenchTabCloseSubmission::DefinitePrecommit(
                    WorkbenchTabClosePrecommit::Unavailable { reason: r },
                ),
            ) => assert_eq!(r, reason),
            other => panic!("wrong Close precommit classification: {other:?}"),
        }
    }
    Ok(())
}

#[test]
fn close_retires_bad_primary_alias_target_correlation_and_outcome() -> TestResult {
    let mut wrong_context = accepted(FIRST_REQUEST_ID, 7);
    let WorkbenchCommandOutcome::TabCloseAccepted {
        terminal_context_id,
        ..
    } = &mut wrong_context
    else {
        unreachable!()
    };
    *terminal_context_id = context_id(99);
    let mut wrong_tab = accepted(FIRST_REQUEST_ID, 7);
    let WorkbenchCommandOutcome::TabCloseAccepted { tab_id, .. } = &mut wrong_tab else {
        unreachable!()
    };
    *tab_id = WorkbenchTabId::parse(&id(99))?;
    for outcome in [
        accepted(FIRST_REQUEST_ID, 6),
        accepted(&id(99), 8),
        wrong_context,
        wrong_tab,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(7),
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
            submit_close_response(7, command_response(FIRST_REQUEST_ID, outcome), true)?,
            WorkbenchTabCloseSubmission::OutcomeUnknown { .. }
        ));
    }
    for response in [
        command_response(SECOND_REQUEST_ID, accepted(FIRST_REQUEST_ID, 7)),
        WorkbenchResponse::Snapshot(snapshot_result(FIRST_REQUEST_ID)),
    ] {
        assert!(matches!(
            submit_close_response(7, response, true)?,
            WorkbenchTabCloseSubmission::OutcomeUnknown { .. }
        ));
    }
    Ok(())
}

#[test]
fn read_preserves_observations_and_checks_only_present_target_fields() -> TestResult {
    for outcome in [
        WorkbenchTabCloseReadOutcome::NotClosing {},
        WorkbenchTabCloseReadOutcome::Denied {},
        WorkbenchTabCloseReadOutcome::Unavailable {
            reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
        },
        available(
            FIRST_REQUEST_ID,
            1,
            WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            },
        ),
        available(
            FIRST_REQUEST_ID,
            u64::MAX,
            WorkbenchTabCloseStatus::Closed {
                revision: WorkbenchRevision::new(10),
            },
        ),
        available(
            FIRST_REQUEST_ID,
            2,
            WorkbenchTabCloseStatus::CleanupBlocked {
                reason: WorkbenchTabCloseBlockedReason::HostRefused,
                resumable: true,
            },
        ),
        available(
            FIRST_REQUEST_ID,
            2,
            WorkbenchTabCloseStatus::OutcomeUnknown {
                phase: WorkbenchTabCloseUnknownPhase::Dispose,
                reason: WorkbenchTabCloseUnknownReason::Deadline,
            },
        ),
        available(
            FIRST_REQUEST_ID,
            2,
            WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason: WorkbenchTabClosePublicationReason::PreparationFailed,
                resumable: true,
            },
        ),
    ] {
        let expected = outcome.clone();
        let actual = submit_read_response(
            read_response(outcome),
            Some(workbench_id(FIRST_REQUEST_ID)),
            false,
        )?;
        assert_eq!(actual?, expected);
    }
    // An unknown canonical operation may be discovered after ambiguous Close acceptance.
    let outcome = available(
        SECOND_REQUEST_ID,
        2,
        WorkbenchTabCloseStatus::Closing {
            phase: WorkbenchTabClosePhase::WaitingOutcome,
        },
    );
    assert_eq!(
        submit_read_response(read_response(outcome.clone()), None, false)??,
        outcome
    );
    Ok(())
}

#[test]
fn read_retires_wrong_available_identity_and_response_family() -> TestResult {
    let wrong_request = WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(SECOND_REQUEST_ID),
        outcome: WorkbenchTabCloseReadOutcome::NotClosing {},
    });
    let mut wrong_context = available(
        FIRST_REQUEST_ID,
        1,
        WorkbenchTabCloseStatus::Closing {
            phase: WorkbenchTabClosePhase::Queued,
        },
    );
    let WorkbenchTabCloseReadOutcome::Available {
        terminal_context_id,
        ..
    } = &mut wrong_context
    else {
        unreachable!()
    };
    *terminal_context_id = context_id(99);
    let mut wrong_tab = available(
        FIRST_REQUEST_ID,
        1,
        WorkbenchTabCloseStatus::Closing {
            phase: WorkbenchTabClosePhase::Queued,
        },
    );
    let WorkbenchTabCloseReadOutcome::Available { tab_id, .. } = &mut wrong_tab else {
        unreachable!()
    };
    *tab_id = WorkbenchTabId::parse(&id(99))?;
    for response in [
        wrong_request,
        read_response(wrong_context),
        read_response(wrong_tab),
        read_response(available(
            SECOND_REQUEST_ID,
            1,
            WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            },
        )),
        command_response(FIRST_REQUEST_ID, WorkbenchCommandOutcome::Denied {}),
    ] {
        assert!(matches!(
            submit_read_response(response, Some(workbench_id(FIRST_REQUEST_ID)), true)?,
            Err(WorkbenchTabCloseReadError::Failed { .. })
        ));
    }
    Ok(())
}

#[test]
fn resume_accepts_checked_successor_stale_current_and_fixed_receipt_replay() -> TestResult {
    for reason in [
        WorkbenchTabCloseResumeRejectionReason::OperationUnavailable,
        WorkbenchTabCloseResumeRejectionReason::OperationMismatch,
        WorkbenchTabCloseResumeRejectionReason::RequestIdentityConflict,
        WorkbenchTabCloseResumeRejectionReason::GenerationMismatch,
        WorkbenchTabCloseResumeRejectionReason::NotResumable,
    ] {
        let actual = submit_resume_response(
            1,
            resume_response(WorkbenchTabCloseResumeOutcome::Rejected { reason }),
            false,
        )?;
        assert!(
            matches!(actual, WorkbenchTabCloseResumeSubmission::Returned(WorkbenchTabCloseResumeOutcome::Rejected {reason: actual}) if actual==reason)
        );
    }
    for (expected, outcome) in [
        (
            1,
            WorkbenchTabCloseResumeOutcome::Resumed {
                attempt_generation: generation(2),
            },
        ),
        (
            1,
            WorkbenchTabCloseResumeOutcome::Stale {
                attempt_generation: generation(u64::MAX),
            },
        ),
        (
            u64::MAX - 1,
            WorkbenchTabCloseResumeOutcome::Resumed {
                attempt_generation: generation(u64::MAX),
            },
        ),
        (u64::MAX, WorkbenchTabCloseResumeOutcome::Denied {}),
        (
            u64::MAX,
            WorkbenchTabCloseResumeOutcome::Rejected {
                reason: WorkbenchTabCloseResumeRejectionReason::GenerationExhausted,
            },
        ),
        (
            1,
            WorkbenchTabCloseResumeOutcome::Unavailable {
                reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
            },
        ),
    ] {
        assert!(
            matches!(submit_resume_response(expected,resume_response(outcome.clone()),false)?,WorkbenchTabCloseResumeSubmission::Returned(actual) if actual==outcome)
        );
    }
    // Exact replay acknowledges its original attempt even after a later status observation.
    let request = resume(1);
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 61)?;
        for read_between in [true, false] {
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::TabCloseResume(request.clone())
            );
            write_workbench_response(
                &mut stream,
                &resume_response(WorkbenchTabCloseResumeOutcome::Resumed {
                    attempt_generation: generation(2),
                }),
            )
            .map_err(io::Error::other)?;
            if read_between {
                let _read = read_workbench_request(&mut stream).map_err(io::Error::other)?;
                write_workbench_response(
                    &mut stream,
                    &read_response(available(
                        FIRST_REQUEST_ID,
                        9,
                        WorkbenchTabCloseStatus::Closing {
                            phase: WorkbenchTabClosePhase::Queued,
                        },
                    )),
                )
                .map_err(io::Error::other)?;
            }
        }
        require_retirement(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    for read_between in [true, false] {
        let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
        assert!(
            matches!(client.submit_workbench_tab_close_resume(resume(1),deadline),WorkbenchTabCloseResumeSubmission::Returned(WorkbenchTabCloseResumeOutcome::Resumed {attempt_generation}) if attempt_generation==generation(2))
        );
        if read_between {
            assert!(
                matches!(client.submit_workbench_tab_close_read(close_read(),None,deadline)?,WorkbenchTabCloseReadOutcome::Available {attempt_generation,..} if attempt_generation==generation(9))
            );
        }
    }
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn resume_checks_every_echo_on_denied_and_retires_impossible_generations() -> TestResult {
    assert!(matches!(
        submit_resume_response(
            1,
            read_response(WorkbenchTabCloseReadOutcome::Denied {}),
            true
        )?,
        WorkbenchTabCloseResumeSubmission::OutcomeUnknown { .. }
    ));
    for field in 0..5 {
        let WorkbenchResponse::TabCloseResume(mut result) =
            resume_response(WorkbenchTabCloseResumeOutcome::Denied {})
        else {
            unreachable!()
        };
        match field {
            0 => result.request_id = workbench_id(SECOND_REQUEST_ID),
            1 => result.site_id = SiteId::parse(&id(99))?,
            2 => result.terminal_context_id = context_id(99),
            3 => result.tab_id = WorkbenchTabId::parse(&id(99))?,
            4 => result.close_request_id = workbench_id(SECOND_REQUEST_ID),
            _ => unreachable!(),
        }
        assert!(matches!(
            submit_resume_response(1, WorkbenchResponse::TabCloseResume(result), true)?,
            WorkbenchTabCloseResumeSubmission::OutcomeUnknown { .. }
        ));
    }
    for (expected, outcome) in [
        (
            1,
            WorkbenchTabCloseResumeOutcome::Resumed {
                attempt_generation: generation(1),
            },
        ),
        (
            1,
            WorkbenchTabCloseResumeOutcome::Resumed {
                attempt_generation: generation(3),
            },
        ),
        (
            u64::MAX,
            WorkbenchTabCloseResumeOutcome::Resumed {
                attempt_generation: generation(u64::MAX),
            },
        ),
        (
            1,
            WorkbenchTabCloseResumeOutcome::Stale {
                attempt_generation: generation(1),
            },
        ),
        (
            u64::MAX,
            WorkbenchTabCloseResumeOutcome::Stale {
                attempt_generation: generation(1),
            },
        ),
    ] {
        assert!(matches!(
            submit_resume_response(expected, resume_response(outcome), true)?,
            WorkbenchTabCloseResumeSubmission::OutcomeUnknown { .. }
        ));
    }
    Ok(())
}

fn close_command(base: u64) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        base,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        },
    )
}
fn accepted(canonical: &str, revision: u64) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::TabCloseAccepted {
        close_request_id: workbench_id(canonical),
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        accepted_revision: WorkbenchRevision::new(revision),
    }
}
fn close_read() -> WorkbenchTabCloseRead {
    WorkbenchTabCloseRead {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(FIRST_REQUEST_ID),
        site_id: SiteId::parse(&id(20)).unwrap(),
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
    }
}
fn generation(value: u64) -> WorkbenchTabCloseAttemptGeneration {
    WorkbenchTabCloseAttemptGeneration::new(value).unwrap()
}
fn available(
    canonical: &str,
    value: u64,
    status: WorkbenchTabCloseStatus,
) -> WorkbenchTabCloseReadOutcome {
    WorkbenchTabCloseReadOutcome::Available {
        close_request_id: workbench_id(canonical),
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        attempt_generation: generation(value),
        status,
    }
}
fn read_response(outcome: WorkbenchTabCloseReadOutcome) -> WorkbenchResponse {
    WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(FIRST_REQUEST_ID),
        outcome,
    })
}
fn resume(expected: u64) -> WorkbenchTabCloseResume {
    let read = close_read();
    WorkbenchTabCloseResume {
        protocol_version: ProtocolVersion,
        request_id: read.request_id,
        site_id: read.site_id,
        terminal_context_id: read.terminal_context_id,
        tab_id: read.tab_id,
        close_request_id: workbench_id(FIRST_REQUEST_ID),
        expected_attempt_generation: generation(expected),
    }
}
fn resume_response(outcome: WorkbenchTabCloseResumeOutcome) -> WorkbenchResponse {
    let request = resume(1);
    WorkbenchResponse::TabCloseResume(WorkbenchTabCloseResumeResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id,
        site_id: request.site_id,
        terminal_context_id: request.terminal_context_id,
        tab_id: request.tab_id,
        close_request_id: request.close_request_id,
        outcome,
    })
}

fn submit_close_response(
    base: u64,
    response: WorkbenchResponse,
    retires: bool,
) -> Result<WorkbenchTabCloseSubmission, Box<dyn std::error::Error>> {
    exchange(
        WorkbenchRequest::Command(close_command(base)),
        response,
        retires,
        |client| {
            client.submit_workbench_tab_close(
                WorkbenchTabCloseRequest::try_from(close_command(base)).unwrap(),
                client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
            )
        },
    )
}
fn submit_read_response(
    response: WorkbenchResponse,
    known: Option<WorkbenchRequestId>,
    retires: bool,
) -> Result<
    Result<WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadError>,
    Box<dyn std::error::Error>,
> {
    exchange(
        WorkbenchRequest::TabCloseRead(close_read()),
        response,
        retires,
        |client| {
            client.submit_workbench_tab_close_read(
                close_read(),
                known.as_ref(),
                client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
            )
        },
    )
}
fn submit_resume_response(
    expected: u64,
    response: WorkbenchResponse,
    retires: bool,
) -> Result<WorkbenchTabCloseResumeSubmission, Box<dyn std::error::Error>> {
    exchange(
        WorkbenchRequest::TabCloseResume(resume(expected)),
        response,
        retires,
        |client| {
            client.submit_workbench_tab_close_resume(
                resume(expected),
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
