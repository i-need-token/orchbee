use super::*;
use crate::local_client::workbench::{
    WorkbenchWorkspaceCloseFailure, WorkbenchWorkspaceCloseNotSubmitted,
    WorkbenchWorkspaceClosePrecommit, WorkbenchWorkspaceCloseReadError,
    WorkbenchWorkspaceCloseRequest, WorkbenchWorkspaceCloseResumeSubmission,
    WorkbenchWorkspaceCloseSubmission,
};
use orchbee_workbench_protocol::{
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePhase,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchWorkspaceCloseCounts, WorkbenchWorkspaceCloseRead,
    WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceCloseReadResult,
    WorkbenchWorkspaceCloseResume, WorkbenchWorkspaceCloseResumeOutcome,
    WorkbenchWorkspaceCloseResumeResult,
};

/// The exact exchange order the local client emits for `workspace close` →
/// `workspace close-status` → `workspace close-resume` → `workspace
/// close-status`: one scripted peer sees every real request frame and answers
/// valid typed responses for an empty Workspace context. This proves the
/// adapter exchange order only — the CLI command route is covered separately.
#[test]
fn workspace_close_exchange_order_produces_the_typed_close_status_resume_closed_chain() -> TestResult
{
    let close = close_command(7);
    let first_read = close_read();
    let resume_request = resume(1);
    let second_read = close_read();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 60)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(close)
        );
        write_workbench_response(
            &mut stream,
            &command_response(FIRST_REQUEST_ID, accepted(SECOND_REQUEST_ID, 7)),
        )
        .map_err(io::Error::other)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::WorkspaceCloseRead(first_read)
        );
        write_workbench_response(
            &mut stream,
            &read_response(available(
                SECOND_REQUEST_ID,
                1,
                WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                },
            )),
        )
        .map_err(io::Error::other)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::WorkspaceCloseResume(resume_request)
        );
        write_workbench_response(
            &mut stream,
            &resume_response(WorkbenchWorkspaceCloseResumeOutcome::Resumed {
                attempt_generation: generation(2),
            }),
        )
        .map_err(io::Error::other)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::WorkspaceCloseRead(second_read)
        );
        write_workbench_response(
            &mut stream,
            &read_response(available(
                SECOND_REQUEST_ID,
                2,
                WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(8),
                },
            )),
        )
        .map_err(io::Error::other)
    })?;

    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
    let request = WorkbenchWorkspaceCloseRequest::try_from(close_command(7))
        .expect("workspace.close fixture");
    let accepted = match client.submit_workbench_workspace_close(request, deadline) {
        WorkbenchWorkspaceCloseSubmission::Accepted(accepted) => accepted,
        other => panic!("expected the typed accepted result, received {other:?}"),
    };
    assert_eq!(accepted.close_request_id.as_str(), SECOND_REQUEST_ID);
    assert_eq!(accepted.project_id, project_id());
    assert_eq!(accepted.workspace_id, workspace_id());
    assert_eq!(accepted.terminal_context_id, context_id(30));
    assert_eq!(accepted.accepted_revision, WorkbenchRevision::new(7));

    let closing = client.submit_workbench_workspace_close_read(
        close_read(),
        Some(&accepted.close_request_id),
        deadline,
    )?;
    assert!(matches!(
        closing,
        WorkbenchWorkspaceCloseReadOutcome::Available {
            ref close_request_id,
            attempt_generation,
            status: WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            },
            ref counts,
            ref members,
            ..
        } if close_request_id.as_str() == SECOND_REQUEST_ID
            && attempt_generation == generation(1)
            && *counts == WorkbenchWorkspaceCloseCounts::default()
            && members.is_empty()
    ));

    let resumed = client.submit_workbench_workspace_close_resume(resume(1), deadline);
    assert!(matches!(
        resumed,
        WorkbenchWorkspaceCloseResumeSubmission::Returned(
            WorkbenchWorkspaceCloseResumeOutcome::Resumed { attempt_generation }
        ) if attempt_generation == generation(2)
    ));

    let closed = client.submit_workbench_workspace_close_read(
        close_read(),
        Some(&accepted.close_request_id),
        deadline,
    )?;
    assert!(matches!(
        closed,
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { revision },
            ..
        } if revision == WorkbenchRevision::new(8)
    ));
    assert_eq!(client.generation().map(NonZeroU64::get), Some(60));
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn workspace_close_request_is_an_opaque_action_specific_conversion() {
    assert!(WorkbenchWorkspaceCloseRequest::try_from(close_command(7)).is_ok());
    for action in [
        WorkbenchAction::TabClose {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        },
        WorkbenchAction::WorkspaceCreate {
            project_id: project_id(),
            label: workbench_label(),
        },
        WorkbenchAction::WorkspaceRename {
            project_id: project_id(),
            workspace_id: workspace_id(),
            label: workbench_label(),
        },
    ] {
        let command = workbench_command(FIRST_REQUEST_ID, 7, action);
        let expected = command.clone();
        assert_eq!(
            WorkbenchWorkspaceCloseRequest::try_from(command)
                .expect_err("non-workspace-close actions must be returned unchanged"),
            expected
        );
    }
}

#[test]
fn workspace_close_preserves_every_compatible_precommit_on_the_carrier() -> TestResult {
    let mut cases = vec![(
        WorkbenchCommandOutcome::Denied {},
        WorkbenchWorkspaceClosePrecommit::Denied,
    )];
    for reason in [
        WorkbenchRejectionReason::SiteMismatch,
        WorkbenchRejectionReason::RevisionMismatch,
        WorkbenchRejectionReason::ProjectUnavailable,
        WorkbenchRejectionReason::WorkspaceUnavailable,
        WorkbenchRejectionReason::ResourceLimit,
        WorkbenchRejectionReason::RevisionExhausted,
        WorkbenchRejectionReason::RequestIdentityConflict,
    ] {
        cases.push((
            WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(7),
                reason,
            },
            WorkbenchWorkspaceClosePrecommit::Rejected {
                revision: WorkbenchRevision::new(7),
                reason,
            },
        ));
    }
    for reason in [
        WorkbenchCommandUnavailableReason::RequestLedgerFull,
        WorkbenchCommandUnavailableReason::TerminalBusy,
        WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
        WorkbenchCommandUnavailableReason::ServerBusy,
        WorkbenchCommandUnavailableReason::ServiceUnavailable,
    ] {
        cases.push((
            WorkbenchCommandOutcome::Unavailable { reason },
            WorkbenchWorkspaceClosePrecommit::Unavailable { reason },
        ));
    }
    for (outcome, expected) in cases {
        match submit_close_response(command_response(FIRST_REQUEST_ID, outcome), false)? {
            WorkbenchWorkspaceCloseSubmission::DefinitePrecommit(actual) => {
                assert_eq!(actual, expected);
            }
            other => panic!("expected definite precommit result, received {other:?}"),
        }
    }
    Ok(())
}

#[test]
fn workspace_close_accepted_boundary_checks_revision_scope_and_correlation() -> TestResult {
    for (canonical, revision) in [(SECOND_REQUEST_ID, 7), (FIRST_REQUEST_ID, 7)] {
        match submit_close_response(
            command_response(FIRST_REQUEST_ID, accepted(canonical, revision)),
            false,
        )? {
            WorkbenchWorkspaceCloseSubmission::Accepted(accepted) => {
                assert_eq!(accepted.close_request_id.as_str(), canonical);
                assert_eq!(accepted.accepted_revision, WorkbenchRevision::new(revision));
            }
            other => panic!("expected typed accepted result, received {other:?}"),
        }
    }
    // A replay echo whose revision moved, any revision above the admitted
    // base, and the maximum revision all retire the session as unknown.
    for (canonical, revision) in [
        (FIRST_REQUEST_ID, 8),
        (SECOND_REQUEST_ID, 8),
        (SECOND_REQUEST_ID, u64::MAX),
    ] {
        assert!(matches!(
            submit_close_response(
                command_response(FIRST_REQUEST_ID, accepted(canonical, revision)),
                true
            )?,
            WorkbenchWorkspaceCloseSubmission::OutcomeUnknown {
                reason: WorkbenchWorkspaceCloseFailure::AcceptedRevisionMismatch { .. },
                ..
            }
        ));
    }
    // Wrong scope echoes retire the session as unknown.
    for field in 0..2 {
        let mut outcome = accepted(SECOND_REQUEST_ID, 7);
        let WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            project_id: outcome_project_id,
            workspace_id: outcome_workspace_id,
            ..
        } = &mut outcome
        else {
            unreachable!()
        };
        match field {
            0 => *outcome_project_id = WorkbenchProjectId::parse(&id(99))?,
            _ => *outcome_workspace_id = WorkbenchWorkspaceId::parse(&id(99))?,
        }
        match submit_close_response(command_response(FIRST_REQUEST_ID, outcome), true)? {
            WorkbenchWorkspaceCloseSubmission::OutcomeUnknown { reason, .. } => {
                match (field, reason) {
                    (0, WorkbenchWorkspaceCloseFailure::ProjectMismatch { expected, received }) => {
                        assert_eq!(expected, project_id());
                        assert_eq!(received, WorkbenchProjectId::parse(&id(99))?);
                    }
                    (
                        1,
                        WorkbenchWorkspaceCloseFailure::WorkspaceMismatch { expected, received },
                    ) => {
                        assert_eq!(expected, workspace_id());
                        assert_eq!(received, WorkbenchWorkspaceId::parse(&id(99))?);
                    }
                    other => panic!("unexpected workspace.close failure {other:?}"),
                }
            }
            other => panic!("expected outcome-unknown submission, received {other:?}"),
        }
    }
    // Wrong correlation and wrong response envelope retire the session.
    assert!(matches!(
        submit_close_response(
            command_response(SECOND_REQUEST_ID, WorkbenchCommandOutcome::Denied {}),
            true
        )?,
        WorkbenchWorkspaceCloseSubmission::OutcomeUnknown {
            reason: WorkbenchWorkspaceCloseFailure::CorrelationMismatch { .. },
            ..
        }
    ));
    assert!(matches!(
        submit_close_response(
            read_response(WorkbenchWorkspaceCloseReadOutcome::Denied {}),
            true
        )?,
        WorkbenchWorkspaceCloseSubmission::OutcomeUnknown {
            reason: WorkbenchWorkspaceCloseFailure::UnexpectedResponse,
            ..
        }
    ));
    // Action-incompatible outcomes retire the session as unknown.
    for outcome in [
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(7),
            reason: WorkbenchRejectionReason::TabUnavailable,
        },
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(7),
            reason: WorkbenchRejectionReason::TerminalContextUnavailable,
        },
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        },
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id: workbench_id(SECOND_REQUEST_ID),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            accepted_revision: WorkbenchRevision::new(7),
        },
    ] {
        assert!(matches!(
            submit_close_response(command_response(FIRST_REQUEST_ID, outcome), true)?,
            WorkbenchWorkspaceCloseSubmission::OutcomeUnknown {
                reason: WorkbenchWorkspaceCloseFailure::ActionIncompatible,
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn workspace_close_read_classification_and_scope_guards() -> TestResult {
    for outcome in [
        WorkbenchWorkspaceCloseReadOutcome::NotClosing {},
        WorkbenchWorkspaceCloseReadOutcome::Denied {},
        WorkbenchWorkspaceCloseReadOutcome::Unavailable {
            reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
        },
    ] {
        let expected = outcome.clone();
        assert_eq!(
            submit_read_response(read_response(outcome), None, false)??,
            expected
        );
    }
    // An Available read whose scope or canonical identity disagrees with the
    // submitted scope retires the session; `None` admits any canonical id.
    assert!(matches!(
        submit_read_response(
            read_response(available(
                SECOND_REQUEST_ID,
                3,
                WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                },
            )),
            None,
            false
        )?,
        Ok(WorkbenchWorkspaceCloseReadOutcome::Available { .. })
    ));
    assert!(matches!(
        submit_read_response(
            read_response(available(
                SECOND_REQUEST_ID,
                3,
                WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                },
            )),
            Some(workbench_id(FIRST_REQUEST_ID)),
            true
        )?,
        Err(WorkbenchWorkspaceCloseReadError::Failed {
            reason: WorkbenchWorkspaceCloseFailure::OperationMismatch { .. },
            ..
        })
    ));
    for field in 0..3 {
        let WorkbenchResponse::WorkspaceCloseRead(mut result) = read_response(available(
            SECOND_REQUEST_ID,
            3,
            WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            },
        )) else {
            unreachable!()
        };
        let WorkbenchWorkspaceCloseReadOutcome::Available {
            project_id: outcome_project_id,
            workspace_id: outcome_workspace_id,
            ..
        } = &mut result.outcome
        else {
            unreachable!()
        };
        match field {
            0 => {
                result.request_id = workbench_id(SECOND_REQUEST_ID);
            }
            1 => *outcome_project_id = WorkbenchProjectId::parse(&id(99))?,
            _ => *outcome_workspace_id = WorkbenchWorkspaceId::parse(&id(99))?,
        }
        match submit_read_response(WorkbenchResponse::WorkspaceCloseRead(result), None, true)? {
            Err(WorkbenchWorkspaceCloseReadError::Failed { reason, .. }) => match (field, reason) {
                (0, WorkbenchWorkspaceCloseFailure::CorrelationMismatch { expected, received }) => {
                    assert_eq!(expected, workbench_id(FIRST_REQUEST_ID));
                    assert_eq!(received, workbench_id(SECOND_REQUEST_ID));
                }
                (1, WorkbenchWorkspaceCloseFailure::ProjectMismatch { expected, received }) => {
                    assert_eq!(expected, project_id());
                    assert_eq!(received, WorkbenchProjectId::parse(&id(99))?);
                }
                (2, WorkbenchWorkspaceCloseFailure::WorkspaceMismatch { expected, received }) => {
                    assert_eq!(expected, workspace_id());
                    assert_eq!(received, WorkbenchWorkspaceId::parse(&id(99))?);
                }
                other => panic!("unexpected workspace-close.read failure {other:?}"),
            },
            other => panic!("expected failed status read, received {other:?}"),
        }
    }
    // A resume envelope answering a status read retires the session.
    assert!(matches!(
        submit_read_response(
            resume_response(WorkbenchWorkspaceCloseResumeOutcome::Denied {}),
            None,
            true
        )?,
        Err(WorkbenchWorkspaceCloseReadError::Failed {
            reason: WorkbenchWorkspaceCloseFailure::UnexpectedResponse,
            ..
        })
    ));
    Ok(())
}

#[test]
fn workspace_close_resume_checks_every_echo_and_retires_impossible_generations() -> TestResult {
    for outcome in [
        WorkbenchWorkspaceCloseResumeOutcome::Resumed {
            attempt_generation: generation(2),
        },
        WorkbenchWorkspaceCloseResumeOutcome::Stale {
            attempt_generation: generation(2),
        },
        WorkbenchWorkspaceCloseResumeOutcome::Denied {},
        WorkbenchWorkspaceCloseResumeOutcome::Unavailable {
            reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
        },
        WorkbenchWorkspaceCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::NotResumable,
        },
    ] {
        assert!(matches!(
            submit_resume_response(1, resume_response(outcome), false)?,
            WorkbenchWorkspaceCloseResumeSubmission::Returned(_)
        ));
    }
    // Every echoed field must match the submitted request.
    for field in 0..5 {
        let WorkbenchResponse::WorkspaceCloseResume(mut result) =
            resume_response(WorkbenchWorkspaceCloseResumeOutcome::Denied {})
        else {
            unreachable!()
        };
        match field {
            0 => result.request_id = workbench_id(SECOND_REQUEST_ID),
            1 => result.site_id = SiteId::parse(&id(99))?,
            2 => result.project_id = WorkbenchProjectId::parse(&id(99))?,
            3 => result.workspace_id = WorkbenchWorkspaceId::parse(&id(99))?,
            _ => result.close_request_id = workbench_id(FIRST_REQUEST_ID),
        }
        assert!(matches!(
            submit_resume_response(1, WorkbenchResponse::WorkspaceCloseResume(result), true)?,
            WorkbenchWorkspaceCloseResumeSubmission::OutcomeUnknown { .. }
        ));
    }
    // Impossible generations retire the session as unknown.
    for (expected, outcome) in [
        (
            1,
            WorkbenchWorkspaceCloseResumeOutcome::Resumed {
                attempt_generation: generation(1),
            },
        ),
        (
            1,
            WorkbenchWorkspaceCloseResumeOutcome::Resumed {
                attempt_generation: generation(3),
            },
        ),
        (
            u64::MAX,
            WorkbenchWorkspaceCloseResumeOutcome::Resumed {
                attempt_generation: generation(u64::MAX),
            },
        ),
        (
            1,
            WorkbenchWorkspaceCloseResumeOutcome::Stale {
                attempt_generation: generation(1),
            },
        ),
        (
            u64::MAX,
            WorkbenchWorkspaceCloseResumeOutcome::Stale {
                attempt_generation: generation(1),
            },
        ),
    ] {
        assert!(matches!(
            submit_resume_response(expected, resume_response(outcome), true)?,
            WorkbenchWorkspaceCloseResumeSubmission::OutcomeUnknown {
                reason: WorkbenchWorkspaceCloseFailure::AttemptGenerationMismatch { .. },
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn workspace_close_family_prewrite_deadlines_preserve_the_carrier_and_write_nothing() -> TestResult
{
    for family in [Family::Command, Family::Read, Family::Resume] {
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
        match family {
            Family::Command => assert!(matches!(
                client.submit_workbench_workspace_close(
                    WorkbenchWorkspaceCloseRequest::try_from(close_command(7)).unwrap(),
                    deadline
                ),
                WorkbenchWorkspaceCloseSubmission::DefinitelyNotSubmitted(
                    WorkbenchWorkspaceCloseNotSubmitted::RequestNotSubmitted(error)
                ) if error.kind() == io::ErrorKind::TimedOut
            )),
            Family::Read => assert!(matches!(
                client.submit_workbench_workspace_close_read(close_read(), None, deadline),
                Err(WorkbenchWorkspaceCloseReadError::NotSubmitted(
                    WorkbenchWorkspaceCloseNotSubmitted::RequestNotSubmitted(error)
                )) if error.kind() == io::ErrorKind::TimedOut
            )),
            Family::Resume => assert!(matches!(
                client.submit_workbench_workspace_close_resume(resume(1), deadline),
                WorkbenchWorkspaceCloseResumeSubmission::DefinitelyNotSubmitted(
                    WorkbenchWorkspaceCloseNotSubmitted::RequestNotSubmitted(error)
                ) if error.kind() == io::ErrorKind::TimedOut
            )),
        }
        assert_eq!(client.generation().map(NonZeroU64::get), Some(62));
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
fn workspace_close_broken_peer_marks_unknown_and_never_replays() -> TestResult {
    for family in [Family::Command, Family::Read, Family::Resume] {
        let expected = family.request();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 63)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                expected
            );
            // Drop without answering: the exchange outcome is unknowable.
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
        match family {
            Family::Command => assert!(matches!(
                client.submit_workbench_workspace_close(
                    WorkbenchWorkspaceCloseRequest::try_from(close_command(7)).unwrap(),
                    deadline
                ),
                WorkbenchWorkspaceCloseSubmission::OutcomeUnknown { .. }
            )),
            Family::Read => assert!(matches!(
                client.submit_workbench_workspace_close_read(close_read(), None, deadline),
                Err(WorkbenchWorkspaceCloseReadError::Failed { .. })
            )),
            Family::Resume => assert!(matches!(
                client.submit_workbench_workspace_close_resume(resume(1), deadline),
                WorkbenchWorkspaceCloseResumeSubmission::OutcomeUnknown { .. }
            )),
        }
        assert_eq!(client.generation(), None);
        // A retired session never replays an effectful request.
        match family {
            Family::Command => assert!(matches!(
                client.submit_workbench_workspace_close(
                    WorkbenchWorkspaceCloseRequest::try_from(close_command(7)).unwrap(),
                    deadline
                ),
                WorkbenchWorkspaceCloseSubmission::DefinitelyNotSubmitted(
                    WorkbenchWorkspaceCloseNotSubmitted::SessionUnavailable
                )
            )),
            Family::Read => assert!(matches!(
                client.submit_workbench_workspace_close_read(close_read(), None, deadline),
                Err(WorkbenchWorkspaceCloseReadError::NotSubmitted(
                    WorkbenchWorkspaceCloseNotSubmitted::SessionUnavailable
                ))
            )),
            Family::Resume => assert!(matches!(
                client.submit_workbench_workspace_close_resume(resume(1), deadline),
                WorkbenchWorkspaceCloseResumeSubmission::DefinitelyNotSubmitted(
                    WorkbenchWorkspaceCloseNotSubmitted::SessionUnavailable
                )
            )),
        }
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Family {
    Command,
    Read,
    Resume,
}

impl Family {
    fn request(self) -> WorkbenchRequest {
        match self {
            Self::Command => WorkbenchRequest::Command(close_command(7)),
            Self::Read => WorkbenchRequest::WorkspaceCloseRead(close_read()),
            Self::Resume => WorkbenchRequest::WorkspaceCloseResume(resume(1)),
        }
    }
}

fn project_id() -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&id(21)).unwrap()
}
fn workspace_id() -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&id(22)).unwrap()
}
fn close_command(base: u64) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        base,
        WorkbenchAction::WorkspaceClose {
            project_id: project_id(),
            workspace_id: workspace_id(),
        },
    )
}
fn accepted(canonical: &str, revision: u64) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::WorkspaceCloseAccepted {
        close_request_id: workbench_id(canonical),
        project_id: project_id(),
        workspace_id: workspace_id(),
        terminal_context_id: context_id(30),
        accepted_revision: WorkbenchRevision::new(revision),
    }
}
fn close_read() -> WorkbenchWorkspaceCloseRead {
    WorkbenchWorkspaceCloseRead {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(FIRST_REQUEST_ID),
        site_id: SiteId::parse(&id(20)).unwrap(),
        project_id: project_id(),
        workspace_id: workspace_id(),
    }
}
fn generation(value: u64) -> WorkbenchTabCloseAttemptGeneration {
    WorkbenchTabCloseAttemptGeneration::new(value).unwrap()
}
fn available(
    canonical: &str,
    value: u64,
    status: WorkbenchTabCloseStatus,
) -> WorkbenchWorkspaceCloseReadOutcome {
    WorkbenchWorkspaceCloseReadOutcome::Available {
        close_request_id: workbench_id(canonical),
        project_id: project_id(),
        workspace_id: workspace_id(),
        terminal_context_id: context_id(30),
        attempt_generation: generation(value),
        status,
        counts: WorkbenchWorkspaceCloseCounts::default(),
        members: Vec::new(),
    }
}
fn read_response(outcome: WorkbenchWorkspaceCloseReadOutcome) -> WorkbenchResponse {
    WorkbenchResponse::WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(FIRST_REQUEST_ID),
        outcome,
    })
}
fn resume(expected: u64) -> WorkbenchWorkspaceCloseResume {
    let read = close_read();
    WorkbenchWorkspaceCloseResume {
        protocol_version: ProtocolVersion,
        request_id: read.request_id,
        site_id: read.site_id,
        project_id: read.project_id,
        workspace_id: read.workspace_id,
        close_request_id: workbench_id(SECOND_REQUEST_ID),
        expected_attempt_generation: generation(expected),
    }
}
fn resume_response(outcome: WorkbenchWorkspaceCloseResumeOutcome) -> WorkbenchResponse {
    let request = resume(1);
    WorkbenchResponse::WorkspaceCloseResume(WorkbenchWorkspaceCloseResumeResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id,
        site_id: request.site_id,
        project_id: request.project_id,
        workspace_id: request.workspace_id,
        close_request_id: request.close_request_id,
        outcome,
    })
}

fn submit_close_response(
    response: WorkbenchResponse,
    retires: bool,
) -> Result<WorkbenchWorkspaceCloseSubmission, Box<dyn std::error::Error>> {
    exchange(
        WorkbenchRequest::Command(close_command(7)),
        response,
        retires,
        |client| {
            client.submit_workbench_workspace_close(
                WorkbenchWorkspaceCloseRequest::try_from(close_command(7)).unwrap(),
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
    Result<WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceCloseReadError>,
    Box<dyn std::error::Error>,
> {
    exchange(
        WorkbenchRequest::WorkspaceCloseRead(close_read()),
        response,
        retires,
        |client| {
            client.submit_workbench_workspace_close_read(
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
) -> Result<WorkbenchWorkspaceCloseResumeSubmission, Box<dyn std::error::Error>> {
    exchange(
        WorkbenchRequest::WorkspaceCloseResume(resume(expected)),
        response,
        retires,
        |client| {
            client.submit_workbench_workspace_close_resume(
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
