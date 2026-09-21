use super::*;
use orchbee_workbench_protocol::{
    SiteId, Snapshot, WorkbenchProjectId, WorkbenchRejectionReason, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseResumeRejectionReason,
    WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseReadResult, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceCloseResumeResult,
    WorkbenchWorkspaceId,
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
fn workspace_close_uses_independent_grants_and_authority_preconditions_without_host_work() {
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
        for site in [
            before.site_id.clone(),
            SiteId::parse(&format!("{:032x}", 222)).unwrap(),
        ] {
            let matches_site = site == before.site_id;
            let close_request_id = WorkbenchRequestId::parse(&format!(
                "{:032x}",
                (permissions + 1) * 1000 + i32::from(matches_site)
            ))
            .unwrap();
            let read = WorkbenchWorkspaceCloseRead {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                site_id: site.clone(),
                project_id: WorkbenchProjectId::parse(&format!("{:032x}", 20)).unwrap(),
                workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 21)).unwrap(),
            };
            let close = WorkbenchRequest::Command(WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: close_request_id.clone(),
                site_id: site.clone(),
                expected_revision: WorkbenchRevision::new(u64::MAX),
                action: WorkbenchAction::WorkspaceClose {
                    project_id: read.project_id.clone(),
                    workspace_id: read.workspace_id.clone(),
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
                            reason: if matches_site {
                                WorkbenchRejectionReason::RevisionMismatch
                            } else {
                                WorkbenchRejectionReason::SiteMismatch
                            },
                        }
                    } else {
                        WorkbenchCommandOutcome::Denied {}
                    },
                })
            );
            assert_eq!(
                response(service.execute(WorkbenchRequest::WorkspaceCloseRead(read.clone()))),
                WorkbenchResponse::WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult {
                    protocol_version: ProtocolVersion,
                    request_id: request_id.clone(),
                    outcome: if read_grant && matches_site {
                        WorkbenchWorkspaceCloseReadOutcome::NotClosing {}
                    } else {
                        WorkbenchWorkspaceCloseReadOutcome::Denied {}
                    },
                })
            );
            let resume = WorkbenchWorkspaceCloseResume {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                site_id: site.clone(),
                project_id: read.project_id.clone(),
                workspace_id: read.workspace_id.clone(),
                close_request_id: WorkbenchRequestId::parse(&format!("{:032x}", 21)).unwrap(),
                expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
            };
            assert_eq!(
                response(service.execute(WorkbenchRequest::WorkspaceCloseResume(resume.clone()))),
                WorkbenchResponse::WorkspaceCloseResume(WorkbenchWorkspaceCloseResumeResult {
                    protocol_version: ProtocolVersion,
                    request_id: request_id.clone(),
                    site_id: site,
                    project_id: resume.project_id,
                    workspace_id: resume.workspace_id,
                    close_request_id: resume.close_request_id,
                    outcome: if lifecycle_grant && matches_site {
                        WorkbenchWorkspaceCloseResumeOutcome::Rejected {
                            reason: WorkbenchTabCloseResumeRejectionReason::OperationUnavailable,
                        }
                    } else {
                        WorkbenchWorkspaceCloseResumeOutcome::Denied {}
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
        site_id: before.site_id,
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
fn navigation_wrapper_rejects_workspace_close_controls_without_losing_any_fields() {
    let read = WorkbenchWorkspaceCloseRead {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        site_id: SiteId::parse(&format!("{:032x}", 2)).unwrap(),
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 3)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 4)).unwrap(),
    };
    for request in [
        WorkbenchRequest::WorkspaceCloseRead(read.clone()),
        WorkbenchRequest::WorkspaceCloseResume(WorkbenchWorkspaceCloseResume {
            protocol_version: ProtocolVersion,
            request_id: read.request_id.clone(),
            site_id: read.site_id.clone(),
            project_id: read.project_id.clone(),
            workspace_id: read.workspace_id.clone(),
            close_request_id: WorkbenchRequestId::parse(&format!("{:032x}", 5)).unwrap(),
            expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(u64::MAX).unwrap(),
        }),
        WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: read.request_id,
            site_id: read.site_id,
            expected_revision: WorkbenchRevision::new(99),
            action: WorkbenchAction::WorkspaceClose {
                project_id: read.project_id,
                workspace_id: read.workspace_id,
            },
        }),
    ] {
        assert!(
            matches!(WorkbenchReadOrNavigationRequest::try_from(request.clone()), Err(rejected) if rejected == request)
        );
    }
}
