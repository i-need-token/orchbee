use super::*;

fn rename_command(workspace: bool, baseline: u64) -> WorkbenchCommand {
    let project_id = WorkbenchProjectId::parse(&id(21)).unwrap();
    let label = WorkbenchLabel::parse("  exact label \u{6807}\u{7b7e}  ").unwrap();
    workbench_command(
        FIRST_REQUEST_ID,
        baseline,
        if workspace {
            WorkbenchAction::WorkspaceRename {
                project_id,
                workspace_id: WorkbenchWorkspaceId::parse(&id(22)).unwrap(),
                label,
            }
        } else {
            WorkbenchAction::ProjectRename { project_id, label }
        },
    )
}

fn exchange_rename(
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
                WorkbenchSetupNotSubmitted::SessionUnavailable
            )
        ));
    }
    drop(client);
    server.finish()?;
    Ok(actual)
}

#[test]
fn location_rename_receipts_bind_exact_target_and_noop_revision() -> TestResult {
    for workspace in [false, true] {
        for (baseline, unchanged) in [(7, false), (7, true), (u64::MAX, true)] {
            let command = rename_command(workspace, baseline);
            let request = WorkbenchSetupRequest::try_from(command.clone()).unwrap();
            assert_eq!(request.request_id(), &command.request_id);
            let revision = WorkbenchRevision::new(if unchanged { baseline } else { baseline + 1 });
            let outcome = if unchanged {
                WorkbenchCommandOutcome::Unchanged { revision }
            } else {
                WorkbenchCommandOutcome::Renamed { revision }
            };
            let expected = if workspace {
                WorkbenchSetupCommit::WorkspaceRenamed {
                    revision,
                    project_id: WorkbenchProjectId::parse(&id(21))?,
                    workspace_id: WorkbenchWorkspaceId::parse(&id(22))?,
                    unchanged,
                }
            } else {
                WorkbenchSetupCommit::ProjectRenamed {
                    revision,
                    project_id: WorkbenchProjectId::parse(&id(21))?,
                    unchanged,
                }
            };
            assert!(
                matches!(exchange_rename(command, command_response(FIRST_REQUEST_ID, outcome), true)?, WorkbenchSetupSubmission::Committed(actual) if actual == expected)
            );
        }
    }
    Ok(())
}

#[test]
fn location_rename_rejects_creation_results_and_impossible_revisions() -> TestResult {
    for workspace in [false, true] {
        for (baseline, outcome) in [
            (
                7,
                WorkbenchCommandOutcome::Unchanged {
                    revision: WorkbenchRevision::new(8),
                },
            ),
            (
                7,
                WorkbenchCommandOutcome::Unchanged {
                    revision: WorkbenchRevision::new(6),
                },
            ),
            (
                7,
                WorkbenchCommandOutcome::Renamed {
                    revision: WorkbenchRevision::new(7),
                },
            ),
            (
                7,
                WorkbenchCommandOutcome::Renamed {
                    revision: WorkbenchRevision::new(9),
                },
            ),
            (
                u64::MAX,
                WorkbenchCommandOutcome::Renamed {
                    revision: WorkbenchRevision::new(u64::MAX),
                },
            ),
        ] {
            let actual = exchange_rename(
                rename_command(workspace, baseline),
                command_response(FIRST_REQUEST_ID, outcome),
                false,
            )?;
            if baseline == u64::MAX {
                assert!(
                    matches!(actual, WorkbenchSetupSubmission::OutcomeUnknown { reason: WorkbenchSetupOutcomeUnknownReason::ExpectedRevisionOverflow { expected }, .. } if expected.value() == baseline)
                );
            } else {
                assert!(matches!(
                    actual,
                    WorkbenchSetupSubmission::OutcomeUnknown {
                        reason: WorkbenchSetupOutcomeUnknownReason::RevisionMismatch { .. },
                        ..
                    }
                ));
            }
        }
        for kind in 0..3 {
            assert!(matches!(
                exchange_rename(
                    rename_command(workspace, 7),
                    command_response(FIRST_REQUEST_ID, setup_outcome(kind, 8)),
                    false
                )?,
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
fn location_rename_refusals_follow_exact_project_or_workspace_authority() -> TestResult {
    for workspace in [false, true] {
        assert!(matches!(
            exchange_rename(
                rename_command(workspace, 7),
                command_response(FIRST_REQUEST_ID, WorkbenchCommandOutcome::Denied {}),
                true
            )?,
            WorkbenchSetupSubmission::DefinitePrecommit(WorkbenchSetupPrecommit::Denied)
        ));
        for (reason, accepted) in [
            (WorkbenchRejectionReason::SiteMismatch, true),
            (WorkbenchRejectionReason::RevisionMismatch, true),
            (WorkbenchRejectionReason::ProjectUnavailable, true),
            (WorkbenchRejectionReason::WorkspaceUnavailable, workspace),
            (WorkbenchRejectionReason::ResourceLimit, true),
            (WorkbenchRejectionReason::RevisionExhausted, true),
            (WorkbenchRejectionReason::RequestIdentityConflict, true),
            (WorkbenchRejectionReason::IdentityExhausted, false),
            (WorkbenchRejectionReason::WorkspaceDirectoryConflict, false),
            (
                WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                false,
            ),
            (WorkbenchRejectionReason::TerminalContextUnavailable, false),
            (WorkbenchRejectionReason::TabUnavailable, false),
        ] {
            let revision = WorkbenchRevision::new(99);
            let result = exchange_rename(
                rename_command(workspace, 7),
                command_response(
                    FIRST_REQUEST_ID,
                    WorkbenchCommandOutcome::Rejected { revision, reason },
                ),
                accepted,
            )?;
            if accepted {
                assert!(
                    matches!(result, WorkbenchSetupSubmission::DefinitePrecommit(WorkbenchSetupPrecommit::Rejected { revision: actual_revision, reason: actual_reason }) if actual_revision == revision && actual_reason == reason)
                );
            } else {
                assert!(matches!(
                    result,
                    WorkbenchSetupSubmission::OutcomeUnknown {
                        reason: WorkbenchSetupOutcomeUnknownReason::ActionIncompatible,
                        ..
                    }
                ));
            }
        }
        for (reason, accepted) in [
            (WorkbenchCommandUnavailableReason::RequestLedgerFull, true),
            (WorkbenchCommandUnavailableReason::ServiceUnavailable, true),
            (
                WorkbenchCommandUnavailableReason::DirectoryUnavailable,
                false,
            ),
            (WorkbenchCommandUnavailableReason::NoCompatibleWorker, false),
            (WorkbenchCommandUnavailableReason::ServerBusy, false),
            (WorkbenchCommandUnavailableReason::TerminalBusy, false),
            (
                WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
                false,
            ),
        ] {
            let result = exchange_rename(
                rename_command(workspace, 7),
                command_response(
                    FIRST_REQUEST_ID,
                    WorkbenchCommandOutcome::Unavailable { reason },
                ),
                accepted,
            )?;
            if accepted {
                assert!(
                    matches!(result, WorkbenchSetupSubmission::DefinitePrecommit(WorkbenchSetupPrecommit::Unavailable { reason: actual }) if actual == reason)
                );
            } else {
                assert!(matches!(
                    result,
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

#[test]
fn location_rename_retires_wrong_correlation_and_response_family() -> TestResult {
    for workspace in [false, true] {
        assert!(
            matches!(exchange_rename(rename_command(workspace, 7), command_response(SECOND_REQUEST_ID, WorkbenchCommandOutcome::Renamed { revision: WorkbenchRevision::new(8) }), false)?, WorkbenchSetupSubmission::OutcomeUnknown { request_id, reason: WorkbenchSetupOutcomeUnknownReason::CorrelationMismatch { expected, received } } if request_id.as_str() == FIRST_REQUEST_ID && expected.as_str() == FIRST_REQUEST_ID && received.as_str() == SECOND_REQUEST_ID)
        );
        for response in close_control_responses()
            .into_iter()
            .chain([WorkbenchResponse::Snapshot(snapshot_result(
                FIRST_REQUEST_ID,
            ))])
        {
            assert!(matches!(
                exchange_rename(rename_command(workspace, 7), response, false)?,
                WorkbenchSetupSubmission::OutcomeUnknown {
                    reason: WorkbenchSetupOutcomeUnknownReason::UnexpectedResponse,
                    ..
                }
            ));
        }
    }
    Ok(())
}
