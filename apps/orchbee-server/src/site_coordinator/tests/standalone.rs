use super::*;
use crate::site_coordinator::PreparedOpen;
use orchbee_client_server_protocol::{OpenOutcome, TerminalOpenRequest};

fn open(value: u128) -> TerminalOpenRequest {
    TerminalOpenRequest::new(id(value), geometry())
}
fn prepare(
    coordinator: &mut SiteCoordinator,
    request: u128,
    candidate: Option<u128>,
) -> PreparedOpen {
    coordinator.prepare_open(
        &open(request),
        candidate.map(id),
        Some(worker(1)),
        Some(expiry()),
    )
}
fn opened(coordinator: &mut SiteCoordinator, request: u128) -> OpenOutcome {
    let PreparedOpen::Ready(outcome) = prepare(coordinator, request, None) else {
        panic!("settled result expected");
    };
    outcome
}
fn pending(prepared: PreparedOpen) -> Box<PendingOperation> {
    let PreparedOpen::Dispatch(pending) = prepared else {
        panic!("dispatch expected");
    };
    pending
}
fn immediate_completion(pending: &PendingOperation) -> Completion {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(request) = pending.command() else {
        panic!("launch expected");
    };
    let TerminalLaunchMode::Immediate(geometry) = request.mode() else {
        panic!("immediate expected");
    };
    assert!(request.cwd().is_none());
    Completion::Launch(
        TerminalLaunchResult::new(
            request.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity(),
                HostLaunchState::Started(geometry),
            )),
        )
        .unwrap(),
    )
}

#[test]
fn relocation_departure_and_arrival_reconcile_open_without_relaunch() {
    use orchbee_workbench_protocol::{
        WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchLabel,
        WorkbenchPanelRelocateDestination,
    };
    let mut coordinator = site();
    let first = pending(prepare(&mut coordinator, 1, Some(101)));
    let authorization = launch_authorization(&first);
    let completion = immediate_completion(&first);
    coordinator
        .complete(first, completion, Some(worker(1)))
        .unwrap();
    let initial = snapshot(&mut coordinator);
    let project_request = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!("{:032x}", 200))
            .unwrap(),
        site_id: initial.site_id.clone(),
        expected_revision: initial.revision,
        action: WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("transfer").unwrap(),
        },
    };
    let WorkbenchResponse::Command(project_result) = coordinator
        .execute_workbench(
            WorkbenchRequest::Command(project_request)
                .try_into()
                .unwrap(),
            true,
        )
        .unwrap()
    else {
        panic!("project result")
    };
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = project_result.outcome else {
        panic!("project")
    };
    let source_tab = &initial.terminal_contexts[0].tabs[0];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &source_tab.root
    else {
        panic!("leaf")
    };
    let current = snapshot(&mut coordinator);
    let moved = coordinator
        .execute_terminal_panel_relocate(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!(
                "{:032x}",
                201
            ))
            .unwrap(),
            site_id: initial.site_id.clone(),
            expected_revision: current.revision,
            action: WorkbenchAction::PanelRelocate {
                terminal_context_id: initial.navigation.standalone_terminal_context_id.clone(),
                tab_id: source_tab.tab_id.clone(),
                panel_id: panel_id.clone(),
                destination: WorkbenchPanelRelocateDestination::NewWorkspace {
                    project_id,
                    primary_directory_id: None,
                    primary_directory_path: None,
                    label: WorkbenchLabel::parse("retained").unwrap(),
                    tab_label: None,
                },
            },
        })
        .unwrap();
    let WorkbenchCommandOutcome::PanelRelocatedToNewTab {
        new_tab_id,
        transfer: Some(transfer),
        revision,
        ..
    } = moved.outcome
    else {
        panic!("transfer")
    };
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));
    assert!(
        coordinator
            .terminals
            .has_retained_binding(primary_presentation.binding.as_ref().unwrap())
    );
    assert_eq!(
        coordinator.terminals.retained_authorization(&id(101)),
        Some(authorization)
    );
    let returned = coordinator
        .execute_terminal_panel_relocate(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!(
                "{:032x}",
                202
            ))
            .unwrap(),
            site_id: initial.site_id.clone(),
            expected_revision: revision,
            action: WorkbenchAction::PanelRelocate {
                terminal_context_id: transfer.target_terminal_context_id,
                tab_id: new_tab_id,
                panel_id: panel_id.clone(),
                destination: WorkbenchPanelRelocateDestination::NewTab {
                    terminal_context_id: Some(
                        initial.navigation.standalone_terminal_context_id.clone(),
                    ),
                    label: None,
                },
            },
        })
        .unwrap();
    assert!(
        matches!(returned.outcome, WorkbenchCommandOutcome::PanelRelocatedToNewTab { transfer: Some(transfer), .. }
        if transfer.closed_source_workspace_id.is_some())
    );
    assert_eq!(
        opened(&mut coordinator, 203),
        OpenOutcome::Opened {
            terminal_create_request_id: id(101)
        }
    );
    let final_snapshot = snapshot(&mut coordinator);
    assert_eq!(final_snapshot.terminal_contexts.len(), 1);
    assert!(final_snapshot.navigation.projects[0].workspaces.is_empty());
    assert_eq!(
        final_snapshot.terminal_contexts[0].tabs[0].root,
        source_tab.root
    );
}

#[test]
fn concurrent_standalone_opens_share_one_committed_reference_and_preserve_geometry() {
    let mut coordinator = site();
    let first = pending(prepare(&mut coordinator, 1, Some(101)));
    assert!(matches!(
        prepare(&mut coordinator, 2, Some(102)),
        PreparedOpen::Wait
    ));
    let completion = immediate_completion(&first);
    coordinator
        .complete(first, completion, Some(worker(1)))
        .unwrap();
    for request in [1, 2, 3] {
        assert_eq!(
            opened(&mut coordinator, request),
            OpenOutcome::Opened {
                terminal_create_request_id: id(101)
            }
        );
    }
    let different = TerminalOpenRequest::new(id(4), Geometry::new(100, 30, 10, 20).unwrap());
    assert!(matches!(
        coordinator.prepare_open(&different, Some(id(104)), Some(worker(1)), Some(expiry())),
        PreparedOpen::Ready(OpenOutcome::Opened { .. })
    ));
    assert_membership(&mut coordinator, 1);
    assert_eq!(
        coordinator
            .viewer_target(&id(101), worker(1))
            .map(|target| target.identity),
        Some(identity())
    );
}

#[test]
fn standalone_preflight_failure_does_not_reserve_or_attach_an_existing_create() {
    let mut coordinator = site();
    for candidate in [None, Some(id(1))] {
        assert!(matches!(
            coordinator.prepare_open(&open(1), candidate, Some(worker(1)), Some(expiry())),
            PreparedOpen::Ready(OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy
            })
        ));
        assert_membership(&mut coordinator, 0);
    }
    assert!(matches!(
        coordinator.prepare_open(&open(1), Some(id(101)), None, Some(expiry())),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::NoCompatibleWorker
        })
    ));
    let _existing = dispatch(coordinator.prepare(create(101), Some(worker(1)), Some(expiry())));
    assert!(matches!(
        prepare(&mut coordinator, 1, Some(101)),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy
        })
    ));
    assert!(matches!(
        coordinator.prepare_open(&open(1), Some(id(102)), Some(worker(1)), None),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy
        })
    ));
    let _fresh = pending(prepare(&mut coordinator, 1, Some(102)));
    assert_membership(&mut coordinator, 1);
}

#[test]
fn standalone_capacity_failure_leaves_empty_without_an_extra_binding() {
    let mut coordinator = site();
    for value in 1..=1024 {
        let _pending =
            dispatch(coordinator.prepare(create(value), Some(worker(1)), Some(expiry())));
    }
    assert!(matches!(
        prepare(&mut coordinator, 2000, Some(2001)),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy
        })
    ));
    assert_membership(&mut coordinator, 0);
    assert!(matches!(
        coordinator.standalone,
        super::super::StandaloneTerminal::Empty
    ));
}

#[test]
fn standalone_every_dispatched_failure_is_final_and_unknown_is_never_replayed() {
    for (completion, expected) in [
        (
            Completion::Busy,
            OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            },
        ),
        (
            Completion::Unavailable,
            OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
        (Completion::Unknown, OpenOutcome::OutcomeUnknown {}),
    ] {
        let mut coordinator = site();
        let first = pending(prepare(&mut coordinator, 1, Some(101)));
        coordinator
            .complete(first, completion, Some(worker(1)))
            .unwrap();
        for request in 2..=4 {
            assert_eq!(opened(&mut coordinator, request), expected);
        }
        assert_membership(&mut coordinator, 1);
    }
}

#[test]
fn standalone_stale_success_and_worker_replacement_never_open_or_recreate() {
    let mut coordinator = site();
    let first = pending(prepare(&mut coordinator, 1, Some(101)));
    let completion = immediate_completion(&first);
    coordinator
        .complete(first, completion, Some(worker(2)))
        .unwrap();
    assert_eq!(opened(&mut coordinator, 2), OpenOutcome::OutcomeUnknown {});

    let mut coordinator = site();
    let first = pending(prepare(&mut coordinator, 1, Some(101)));
    let completion = immediate_completion(&first);
    coordinator
        .complete(first, completion, Some(worker(1)))
        .unwrap();
    for replacement in [None, Some(worker(2))] {
        assert!(matches!(
            coordinator.prepare_open(&open(2), Some(id(102)), replacement, Some(expiry())),
            PreparedOpen::Ready(OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable
            })
        ));
        assert_membership(&mut coordinator, 1);
    }
}

#[test]
fn standalone_host_failure_reasons_are_retained_for_every_later_caller() {
    let epoch = HostEpoch::from_bytes([4; 16]).unwrap();
    for (observed, expected) in [
        (
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::CapacityExceeded),
            OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            },
        ),
        (
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::LeaseExpired),
            OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
        (
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
                host_epoch: epoch,
            }),
            OpenOutcome::Failed {
                reason: CreateFailureReason::TerminalCreateFailed,
            },
        ),
        (
            TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostReportedFailure {
                host_epoch: epoch,
            }),
            OpenOutcome::Failed {
                reason: CreateFailureReason::TerminalCreateFailed,
            },
        ),
        (
            TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff),
            OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
        (
            TerminalLaunchOutcome::OutcomeUnknown { host_epoch: epoch },
            OpenOutcome::OutcomeUnknown {},
        ),
    ] {
        let mut coordinator = site();
        let first = pending(prepare(&mut coordinator, 1, Some(101)));
        let result = TerminalLaunchResult::new(launch_authorization(&first), observed).unwrap();
        coordinator
            .complete(first, Completion::Launch(result), Some(worker(1)))
            .unwrap();
        for request in [2, 3] {
            assert_eq!(opened(&mut coordinator, request), expected);
        }
        assert_membership(&mut coordinator, 1);
    }
}
