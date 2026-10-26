use super::*;
use orchbee_workbench_protocol::{
    SessionId, Snapshot, WorkbenchRejectionReason, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseRead, WorkbenchTabCloseResume,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabId,
};

fn snapshot(service: &WorkbenchService) -> Snapshot {
    let WorkbenchResponse::Snapshot(result) = response(service.execute(
        WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 100)).unwrap(),
        }),
    )) else {
        panic!("snapshot expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available snapshot expected")
    };
    snapshot
}

#[test]
fn close_uses_independent_grants_and_authority_preconditions_without_host_work() {
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let reader = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: true,
        },
        Arc::clone(&terminals),
    );
    let before = snapshot(&reader);
    let request_id = WorkbenchRequestId::parse(&format!("{:032x}", 90)).unwrap();
    for permissions in 0..16 {
        let read_grant = permissions & 1 != 0;
        let lifecycle_grant = permissions & 8 != 0;
        let service = WorkbenchService::new(
            WorkbenchPolicy {
                read: read_grant,
                manage_navigation: permissions & 2 != 0,
                associate_directory: false,
                create_terminal: permissions & 4 != 0,
                manage_terminal_lifecycle: lifecycle_grant,
            },
            Arc::clone(&terminals),
        );
        for session in [
            before.session_id.clone(),
            SessionId::parse(&format!("{:032x}", 222)).unwrap(),
        ] {
            let matches_session = session == before.session_id;
            let close_request_id = WorkbenchRequestId::parse(&format!(
                "{:032x}",
                (permissions + 1) * 1000 + i32::from(matches_session)
            ))
            .unwrap();
            let read = WorkbenchTabCloseRead {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                session_id: session.clone(),
                terminal_context_id: before.navigation.workspaces[0].terminal_context_id.clone(),
                tab_id: WorkbenchTabId::parse(&format!("{:032x}", 20)).unwrap(),
            };
            let close = WorkbenchRequest::Command(WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: close_request_id.clone(),
                session_id: session.clone(),
                expected_revision: WorkbenchRevision::new(u64::MAX),
                action: WorkbenchAction::TabClose {
                    terminal_context_id: read.terminal_context_id.clone(),
                    tab_id: read.tab_id.clone(),
                },
            });
            assert_eq!(
                response(service.execute(close)),
                WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: close_request_id,
                    outcome: if lifecycle_grant {
                        WorkbenchCommandOutcome::Rejected {
                            revision: before.revision,
                            reason: if matches_session {
                                WorkbenchRejectionReason::RevisionMismatch
                            } else {
                                WorkbenchRejectionReason::SessionMismatch
                            },
                        }
                    } else {
                        WorkbenchCommandOutcome::Denied {}
                    },
                })
            );
            assert_eq!(
                response(service.execute(WorkbenchRequest::TabCloseRead(read.clone()))),
                WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
                    protocol_version: ProtocolVersion,
                    request_id: request_id.clone(),
                    outcome: if read_grant && matches_session {
                        WorkbenchTabCloseReadOutcome::NotClosing {}
                    } else {
                        WorkbenchTabCloseReadOutcome::Denied {}
                    },
                })
            );
            let resume = WorkbenchTabCloseResume {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                session_id: session.clone(),
                terminal_context_id: read.terminal_context_id,
                tab_id: read.tab_id,
                close_request_id: WorkbenchRequestId::parse(&format!("{:032x}", 21)).unwrap(),
                expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
            };
            assert_eq!(
                response(service.execute(WorkbenchRequest::TabCloseResume(resume.clone()))),
                WorkbenchResponse::TabCloseResume(WorkbenchTabCloseResumeResult {
                    protocol_version: ProtocolVersion,
                    request_id: request_id.clone(),
                    session_id: session,
                    terminal_context_id: resume.terminal_context_id,
                    tab_id: resume.tab_id,
                    close_request_id: resume.close_request_id,
                    outcome: if lifecycle_grant && matches_session {
                        WorkbenchTabCloseResumeOutcome::Rejected {
                            reason: WorkbenchTabCloseResumeRejectionReason::OperationUnavailable,
                        }
                    } else {
                        WorkbenchTabCloseResumeOutcome::Denied {}
                    },
                })
            );
        }
    }
    assert_eq!(snapshot(&reader), before);
    // Reads and refused Resume controls did not occupy this ordinary request identity.
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id,
        session_id: before.session_id,
        expected_revision: before.revision,
        action: WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("Still usable").unwrap(),
        },
    };
    assert!(matches!(
        response(reader.execute(WorkbenchRequest::Command(command))),
        WorkbenchResponse::Command(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::ProjectCreated { .. },
            ..
        })
    ));
}

#[test]
fn navigation_wrapper_rejects_close_controls_without_losing_any_fields() {
    let read = WorkbenchTabCloseRead {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
            &format!("{:032x}", 3),
        )
        .unwrap(),
        tab_id: WorkbenchTabId::parse(&format!("{:032x}", 4)).unwrap(),
    };
    for request in [
        WorkbenchRequest::TabCloseRead(read.clone()),
        WorkbenchRequest::TabCloseResume(WorkbenchTabCloseResume {
            protocol_version: ProtocolVersion,
            request_id: read.request_id.clone(),
            session_id: read.session_id.clone(),
            terminal_context_id: read.terminal_context_id.clone(),
            tab_id: read.tab_id.clone(),
            close_request_id: WorkbenchRequestId::parse(&format!("{:032x}", 5)).unwrap(),
            expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(u64::MAX).unwrap(),
        }),
        WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: read.request_id,
            session_id: read.session_id,
            expected_revision: WorkbenchRevision::new(99),
            action: WorkbenchAction::TabClose {
                terminal_context_id: read.terminal_context_id,
                tab_id: read.tab_id,
            },
        }),
    ] {
        assert!(
            matches!(WorkbenchReadOrNavigationRequest::try_from(request.clone()), Err(rejected) if rejected == request)
        );
    }
}
