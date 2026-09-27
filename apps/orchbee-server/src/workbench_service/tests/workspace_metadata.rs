use super::*;
use orchbee_workbench_protocol::{
    MetadataReportDisposition, MetadataSource, MetadataTokenKey, MetadataTokenPatch,
    MetadataTokenValue, SiteId, Snapshot, WorkbenchRevision, WorkbenchWorkspaceId,
    WorkspaceMetadataAction as Action, WorkspaceMetadataFailure as Failure,
    WorkspaceMetadataOutcome as Outcome, WorkspaceMetadataRequest, WorkspaceMetadataResult,
};

fn service(terminals: &Arc<LocalTerminalService>, grants: u8) -> WorkbenchService {
    WorkbenchService::new(
        WorkbenchPolicy {
            read: grants & 1 != 0,
            manage_navigation: grants & 2 != 0,
            associate_directory: grants & 4 != 0,
            create_terminal: grants & 8 != 0,
            manage_terminal_lifecycle: grants & 16 != 0,
        },
        Arc::clone(terminals),
    )
}
fn snapshot(service: &WorkbenchService) -> Snapshot {
    let WorkbenchResponse::Snapshot(result) = response(service.execute(
        WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 200)).unwrap(),
        }),
    )) else {
        panic!("snapshot result")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("snapshot")
    };
    snapshot
}
fn command(service: &WorkbenchService, n: u64, action: WorkbenchAction) -> WorkbenchCommandOutcome {
    let before = snapshot(service);
    let WorkbenchResponse::Command(result) =
        response(service.execute(WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap(),
            site_id: before.site_id,
            expected_revision: before.revision,
            action,
        })))
    else {
        panic!("command result")
    };
    result.outcome
}
fn execute(
    service: &WorkbenchService,
    request: &WorkspaceMetadataRequest,
) -> WorkspaceMetadataResult {
    let WorkbenchResponse::WorkspaceMetadata(result) =
        response(service.execute(WorkbenchRequest::WorkspaceMetadata(request.clone())))
    else {
        panic!("metadata result")
    };
    assert!(result.answers(request));
    result
}

#[test]
fn workspace_metadata_grants_are_independent_and_reports_do_not_mutate_topology() {
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let admin = service(&terminals, 31);
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = command(
        &admin,
        1,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("reports").unwrap(),
        },
    ) else {
        panic!("project")
    };
    let mut ids = vec![];
    for n in 2..4 {
        let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = command(
            &admin,
            n,
            WorkbenchAction::WorkspaceCreate {
                project_id: project_id.clone(),
                label: WorkbenchLabel::parse(&format!("space{n}")).unwrap(),
            },
        ) else {
            panic!("workspace")
        };
        ids.push(workspace_id);
    }
    let before = snapshot(&admin);
    let mut read = WorkspaceMetadataRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 500)).unwrap(),
        site_id: before.site_id.clone(),
        action: Action::Read {
            workspace_ids: ids.clone(),
        },
    };
    for grants in 0..32 {
        let service = service(&terminals, grants);
        let result = execute(&service, &read);
        if grants & 1 == 0 {
            assert_eq!(
                result.outcome,
                Outcome::Failed {
                    reason: Failure::Denied
                }
            );
        } else {
            assert!(matches!(result.outcome, Outcome::Read { .. }));
        }
        let value = MetadataTokenValue::parse(&format!("value{grants}")).unwrap();
        let mut report = read.clone();
        report.action = Action::Report {
            workspace_id: ids[0].clone(),
            source: MetadataSource::parse("hook").unwrap(),
            tokens: MetadataTokenPatch::new(
                [(
                    MetadataTokenKey::parse("summary").unwrap(),
                    Some(value.clone()),
                )]
                .into(),
            )
            .unwrap(),
            sequence: Some(WorkbenchRevision::new(u64::from(grants))),
            ttl_milliseconds: None,
        };
        let prior = execute(&admin, &read);
        let result = execute(&service, &report);
        if grants & 2 == 0 {
            assert_eq!(
                result.outcome,
                Outcome::Failed {
                    reason: Failure::Denied
                }
            );
            assert_eq!(execute(&admin, &read), prior);
        } else {
            assert!(
                matches!(result.outcome, Outcome::Reported { disposition: MetadataReportDisposition::Applied, workbench_revision, .. } if workbench_revision == before.revision)
            );
            let Outcome::Read { workspaces, .. } = execute(&admin, &read).outcome else {
                panic!("read")
            };
            assert_eq!(
                workspaces
                    .iter()
                    .map(|row| &row.workspace_id)
                    .collect::<Vec<_>>(),
                ids.iter().collect::<Vec<_>>()
            );
            assert_eq!(
                workspaces[0]
                    .tokens
                    .as_map()
                    .get(&MetadataTokenKey::parse("summary").unwrap()),
                Some(&value)
            );
            assert!(workspaces[1].tokens.as_map().is_empty());
            assert!(matches!(
                execute(&service, &report).outcome,
                Outcome::Reported {
                    disposition: MetadataReportDisposition::Stale,
                    ..
                }
            ));
        }
        assert_eq!(snapshot(&admin), before);
    }
    read.site_id = SiteId::parse(&format!("{:032x}", 999)).unwrap();
    assert_eq!(
        execute(&admin, &read).outcome,
        Outcome::Failed {
            reason: Failure::SiteMismatch
        }
    );
    read.site_id = before.site_id.clone();
    read.action = Action::Read {
        workspace_ids: vec![WorkbenchWorkspaceId::parse(&format!("{:032x}", 999)).unwrap()],
    };
    assert_eq!(
        execute(&admin, &read).outcome,
        Outcome::Failed {
            reason: Failure::NotFound
        }
    );
    read.action = Action::Read {
        workspace_ids: vec![ids[0].clone(), ids[0].clone()],
    };
    assert_eq!(
        execute(&admin, &read).outcome,
        Outcome::Failed {
            reason: Failure::InvalidRequest
        }
    );
    read.action = Action::Read {
        workspace_ids: vec![],
    };
    assert!(
        matches!(execute(&admin, &read).outcome, Outcome::Read { workspaces, .. } if workspaces.is_empty())
    );
    assert_eq!(snapshot(&admin), before);
}
