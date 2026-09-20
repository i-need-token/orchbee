use super::*;
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState,
    HostTerminalId, HostTerminalIdentity, TerminalLaunchFailure, TerminalLaunchMode,
    TerminalLaunchOutcome, TerminalLaunchRejection, TerminalLaunchResult,
    TerminalOperationAuthorization, WorkerGeneration, WorkerId,
};
use orchbee_client_server_protocol::{
    CreateFailureReason, CreateUnavailableReason, Geometry, StartMode, TerminalCreateRequest,
};
use orchbee_workbench_protocol::{
    PanelNode, ProtocolVersion, SiteId, Snapshot, SnapshotReadIntent, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchSnapshotOutcome,
};
use std::num::NonZeroU64;

mod launch_read;
mod multileaf_membership;
mod panel_split;
mod standalone;
mod tab_create;
mod terminal_selection;

fn id(value: u128) -> RequestId {
    RequestId::parse(&format!("{value:032x}")).unwrap()
}
fn site() -> SiteCoordinator {
    SiteCoordinator::new(Some(
        WorkbenchAuthority::new(SiteId::parse(&format!("{:032x}", 999)).unwrap()).unwrap(),
    ))
}
fn worker(generation: u64) -> WorkerPlacement {
    WorkerPlacement {
        worker_id: WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
    }
}
fn expiry() -> LeaseExpiresAtUnixMilliseconds {
    LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(10_000).unwrap())
}
fn geometry() -> Geometry {
    Geometry::new(80, 24, 8, 16).unwrap()
}
fn create(value: u128) -> ClientMessage {
    ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        id(value),
        None,
        None,
        StartMode::Deferred,
        None,
    ))
}
fn dispatch(prepared: PreparedOperation) -> Box<PendingOperation> {
    match prepared {
        PreparedOperation::Dispatch(pending) => pending,
        other => panic!("expected dispatch: {other:?}"),
    }
}
fn identity() -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([3; 16]).unwrap(),
        HostEpoch::from_bytes([4; 16]).unwrap(),
        HostTerminalId::from_bytes([5; 16]).unwrap(),
    )
}
fn launch_authorization(pending: &PendingOperation) -> TerminalOperationAuthorization {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(request) = pending.command() else {
        panic!("expected launch")
    };
    request.authorization()
}
fn snapshot(site: &mut SiteCoordinator) -> Snapshot {
    let response = site
        .execute_workbench(
            WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 888)).unwrap(),
            })
            .try_into()
            .unwrap(),
            false,
        )
        .unwrap();
    let WorkbenchResponse::Snapshot(result) = response else {
        panic!("expected snapshot result")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("expected available snapshot")
    };
    snapshot
}
fn assert_membership(site: &mut SiteCoordinator, count: usize) {
    let snapshot = snapshot(site);
    assert_eq!(snapshot.terminal_contexts.len(), 1);
    assert_eq!(snapshot.terminal_contexts[0].tabs.len(), count);
    assert_eq!(snapshot.revision.value(), u64::try_from(count).unwrap());
}

#[test]
fn missing_workbench_refuses_open_without_consuming_terminal_identity_or_reference() {
    let mut site = SiteCoordinator::new(None);
    let request = orchbee_client_server_protocol::TerminalOpenRequest::new(id(10), geometry());
    for _ in 0..2 {
        assert!(matches!(
            site.prepare_open(&request, Some(id(11)), Some(worker(1)), Some(expiry())),
            PreparedOpen::Ready(orchbee_client_server_protocol::OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy
            })
        ));
    }
    let planned = dispatch(site.prepare(create(11), Some(worker(1)), Some(expiry())));
    assert_eq!(
        u128::from_be_bytes(launch_authorization(&planned).terminal_id().as_bytes()),
        1
    );
}

#[test]
fn membership_and_terminal_intent_match_before_completion_and_settle_once() {
    let mut site = site();
    let request = orchbee_client_server_protocol::TerminalOpenRequest::new(id(10), geometry());
    let PreparedOpen::Dispatch(pending) =
        site.prepare_open(&request, Some(id(11)), Some(worker(1)), Some(expiry()))
    else {
        panic!("expected launch")
    };
    let before = snapshot(&mut site);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &before.terminal_contexts[0].tabs[0].root
    else {
        panic!("expected panel")
    };
    let binding = primary_presentation.binding.as_ref().unwrap();
    let authorization = launch_authorization(&pending);
    assert_eq!(
        binding.terminal_id.as_str(),
        format!(
            "{:032x}",
            u128::from_be_bytes(authorization.terminal_id().as_bytes())
        )
    );
    assert_eq!(
        binding.worker_id.as_str(),
        format!(
            "{:032x}",
            u128::from_be_bytes(authorization.worker_id().as_bytes())
        )
    );
    assert_eq!(site.viewer_target(&id(11), worker(1)), None);
    site.complete(pending, Completion::Unknown, Some(worker(1)))
        .unwrap();
    assert_eq!(snapshot(&mut site), before);
    assert!(matches!(
        site.prepare_open(&request, None, Some(worker(1)), None),
        PreparedOpen::Ready(orchbee_client_server_protocol::OpenOutcome::OutcomeUnknown {})
    ));
    let next = dispatch(site.prepare(create(12), Some(worker(1)), Some(expiry())));
    assert_eq!(
        u128::from_be_bytes(launch_authorization(&next).terminal_id().as_bytes()),
        2
    );
}

#[test]
fn workbench_capacity_refusal_never_commits_the_prepared_terminal_plan() {
    use orchbee_workbench_protocol::{
        WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchLabel,
        WorkbenchRejectionReason, WorkbenchRevision,
    };
    let mut site = site();
    let initial = snapshot(&mut site);
    let mut revision = WorkbenchRevision::new(0);
    let mut request = 0_u128;
    let mut submit = |site: &mut SiteCoordinator, action| {
        request += 1;
        let result = site
            .execute_workbench(
                WorkbenchRequest::Command(WorkbenchCommand {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
                    site_id: initial.site_id.clone(),
                    expected_revision: revision,
                    action,
                })
                .try_into()
                .unwrap(),
                true,
            )
            .unwrap();
        let WorkbenchResponse::Command(result) = result else {
            panic!("expected command")
        };
        match &result.outcome {
            WorkbenchCommandOutcome::ProjectCreated { revision: next, .. }
            | WorkbenchCommandOutcome::WorkspaceCreated { revision: next, .. } => revision = *next,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::ResourceLimit,
                ..
            } => {}
            outcome => panic!("unexpected population outcome {outcome:?}"),
        }
        result.outcome
    };
    let mut projects = Vec::new();
    for _ in 0..256 {
        let result = submit(
            &mut site,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse(&"\u{1f41d}".repeat(512)).unwrap(),
            },
        );
        let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = result else {
            panic!("project population failed")
        };
        projects.push(project_id);
    }
    // Fill the remaining byte capacity, then use minimal labels so the last refused Workspace
    // is smaller than the one-Tab aggregate. Collection limits are explicitly checked below.
    for label in ["\u{1f41d}".repeat(512), "1".to_owned()] {
        let mut refused = false;
        for index in 0..1024 {
            if matches!(
                submit(
                    &mut site,
                    WorkbenchAction::WorkspaceCreate {
                        project_id: projects[index % projects.len()].clone(),
                        label: WorkbenchLabel::parse(&label).unwrap(),
                    }
                ),
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::ResourceLimit,
                    ..
                }
            ) {
                refused = true;
                break;
            }
        }
        assert!(refused, "population did not reach byte capacity");
    }
    let before = snapshot(&mut site);
    assert!(before.terminal_contexts.len() < orchbee_workbench_protocol::MAXIMUM_TERMINAL_CONTEXTS);
    assert!(
        before.navigation.projects.iter().all(
            |p| p.workspaces.len() < orchbee_workbench_protocol::MAXIMUM_WORKSPACES_PER_PROJECT
        )
    );
    let request = orchbee_client_server_protocol::TerminalOpenRequest::new(id(10_000), geometry());
    assert!(matches!(
        site.prepare_open(&request, Some(id(10_001)), Some(worker(1)), Some(expiry())),
        PreparedOpen::Ready(orchbee_client_server_protocol::OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy
        })
    ));
    assert_eq!(snapshot(&mut site), before);
    assert!(matches!(site.standalone, super::StandaloneTerminal::Empty));
    let next = dispatch(site.prepare(create(10_001), Some(worker(1)), Some(expiry())));
    assert_eq!(
        u128::from_be_bytes(launch_authorization(&next).terminal_id().as_bytes()),
        1
    );
}

#[test]
fn close_site_identity_remains_observable_after_publication_retirement() {
    use orchbee_workbench_protocol::{
        WorkbenchAction, WorkbenchCommand, WorkbenchTabMoveDirection,
    };
    let mut coordinator = site();
    coordinator
        .workbench
        .as_mut()
        .unwrap()
        .commit_standalone_intent(PresentationBinding {
            terminal_id: TerminalId::parse("11111111111111111111111111111111").unwrap(),
            worker_id: orchbee_workbench_protocol::WorkerId::parse(
                "22222222222222222222222222222222",
            )
            .unwrap(),
        })
        .unwrap();
    let before = snapshot(&mut coordinator);
    let authority = coordinator.workbench.as_mut().unwrap();
    let grant = WorkbenchGrant::manage_navigation(before.site_id.clone());
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 500)).unwrap(),
        site_id: before.site_id.clone(),
        expected_revision: before.revision,
        action: WorkbenchAction::TabMove {
            terminal_context_id: before.navigation.standalone_terminal_context_id.clone(),
            tab_id: before.terminal_contexts[0].tabs[0].tab_id.clone(),
            direction: WorkbenchTabMoveDirection::First,
        },
    };
    let orchbee_server::WorkbenchTabMoveBegin::Admitted(admission) =
        authority.begin_terminal_tab_move(Some(&grant), command)
    else {
        panic!("Move admission expected")
    };
    assert!(matches!(
        admission.retire_composition_failure().outcome,
        orchbee_workbench_protocol::WorkbenchCommandOutcome::Unavailable { .. }
    ));
    assert_eq!(
        coordinator.workbench_site_matches(&before.site_id),
        Some(true)
    );
    assert_eq!(
        coordinator.workbench_site_matches(&SiteId::parse(&format!("{:032x}", 501)).unwrap()),
        Some(false)
    );
    let response = coordinator
        .execute_workbench(
            WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 502)).unwrap(),
            })
            .try_into()
            .unwrap(),
            false,
        )
        .unwrap();
    assert!(matches!(
        response,
        WorkbenchResponse::Snapshot(orchbee_workbench_protocol::WorkbenchSnapshotResult {
            outcome: WorkbenchSnapshotOutcome::Unavailable { .. },
            ..
        })
    ));
}
