use super::*;
mod reference_completion;
mod tab_move;
use crate::site_coordinator::{
    PostcommitCoordinationError, PreparedTerminalTabCreate, StandaloneTerminal,
};
use orchbee_capability_node_protocol::{
    HostLaunchAccepted, TerminalLaunchFailure, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchRejection, TerminalLaunchResult,
};
use orchbee_client_server_protocol::OpenOutcome;
use orchbee_server::{WorkbenchAuthority, WorkbenchGrant};
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, TerminalId as WorkbenchTerminalId, WorkbenchAction,
    WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    WorkbenchTerminalContextId, WorkbenchTerminalFailureReason, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason, WorkerId as WorkbenchWorkerId,
};

#[test]
fn standalone_composition_helpers_reject_workspace_membership_even_with_a_matching_reference() {
    let mut coordinator = site();
    let context = configure_workspace(&mut coordinator, "/tmp/workspace");
    let empty = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .terminal_membership(&context)
        .unwrap();
    assert!(!composition_matches(
        &empty,
        &coordinator.standalone,
        &coordinator.terminals
    ));
    let request = context_tab_command(&coordinator, 8800, 3, context.clone(), "workspace");
    let _pending = dispatch_tab(&mut coordinator, request, 8801);
    let members = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .terminal_membership(&context)
        .unwrap();
    // Deliberate wrong-owner input: matching authorization must not make a Workspace a bootstrap.
    coordinator.standalone = StandaloneTerminal::Creating(id(8801));
    assert!(!composition_matches(
        &members,
        &coordinator.standalone,
        &coordinator.terminals
    ));
    assert!(
        crate::site_coordinator::standalone::prepare_standalone_close_settlement(
            &mut coordinator.standalone,
            &coordinator.terminals,
            &members,
            &empty,
            &members.members()[0],
        )
        .is_err()
    );
    assert!(matches!(&coordinator.standalone,
        StandaloneTerminal::Creating(reference) if reference == &id(8801)
    ));
}

pub(super) fn context_tab_command(
    coordinator: &SiteCoordinator,
    request: u128,
    revision: u64,
    terminal_context_id: WorkbenchTerminalContextId,
    label: &str,
) -> WorkbenchCommand {
    let workbench = coordinator.workbench.as_ref().unwrap();
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        site_id: workbench.site_id().clone(),
        expected_revision: WorkbenchRevision::new(revision),
        action: WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id,
            label: WorkbenchLabel::parse(label).unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    }
}

pub(super) fn tab_command(
    coordinator: &SiteCoordinator,
    request: u128,
    revision: u64,
    label: &str,
) -> WorkbenchCommand {
    let terminal_context_id = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .standalone_membership()
        .unwrap()
        .terminal_context_id()
        .clone();
    context_tab_command(coordinator, request, revision, terminal_context_id, label)
}

fn submit_navigation(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchCommandResult {
    let WorkbenchResponse::Command(result) = authority.execute(
        Some(grant),
        WorkbenchRequest::Command(command).try_into().unwrap(),
    ) else {
        panic!("Workbench command result expected")
    };
    result
}

pub(super) fn configure_workspace(
    coordinator: &mut SiteCoordinator,
    path: &str,
) -> WorkbenchTerminalContextId {
    let workbench = coordinator.workbench.as_mut().unwrap();
    let site_id = workbench.site_id().clone();
    let navigation = WorkbenchGrant::manage_navigation(site_id.clone());
    let project = submit_navigation(
        workbench,
        &navigation,
        WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 700)).unwrap(),
            site_id: site_id.clone(),
            expected_revision: WorkbenchRevision::new(0),
            action: WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("project").unwrap(),
            },
        },
    );
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = project.outcome else {
        panic!("Project creation expected")
    };
    let workspace = submit_navigation(
        workbench,
        &navigation,
        WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 701)).unwrap(),
            site_id: site_id.clone(),
            expected_revision: WorkbenchRevision::new(1),
            action: WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id.clone()),
                label: WorkbenchLabel::parse("workspace").unwrap(),
            },
        },
    );
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id,
        terminal_context_id,
        ..
    } = workspace.outcome
    else {
        panic!("Workspace creation expected")
    };
    let association = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 702)).unwrap(),
        site_id,
        expected_revision: WorkbenchRevision::new(2),
        action: WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            workspace_id,
            path: WorkbenchDirectoryPath::parse(path).unwrap(),
        },
    };
    let worker_id = WorkbenchWorkerId::parse(&format!(
        "{:032x}",
        u128::from_be_bytes(worker(1).worker_id.as_bytes())
    ))
    .unwrap();
    let association_grant = navigation.with_directory_association();
    assert!(matches!(
        workbench
            .accept_workspace_directory(Some(&association_grant), association, worker_id)
            .outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
            revision,
            ..
        } if revision == WorkbenchRevision::new(3)
    ));
    terminal_context_id
}

pub(super) fn dispatch_tab(
    coordinator: &mut SiteCoordinator,
    command: WorkbenchCommand,
    reference: u128,
) -> crate::site_coordinator::PendingTerminalTabCreate {
    let PreparedTerminalTabCreate::Dispatch(pending) = coordinator.prepare_terminal_tab_create(
        command,
        Some(id(reference)),
        Some(expiry()),
        Some(worker(1)),
    ) else {
        panic!("tab.create dispatch expected")
    };
    pending
}

fn tab_authorization(
    pending: &crate::site_coordinator::PendingTerminalTabCreate,
) -> TerminalOperationAuthorization {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("terminal Launch expected")
    };
    launch.authorization()
}

fn immediate_tab_completion(
    pending: &crate::site_coordinator::PendingTerminalTabCreate,
) -> Completion {
    immediate_completion(pending.command())
}

pub(super) fn immediate_completion(command: &CapabilityNodeMessage) -> Completion {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = command else {
        panic!("terminal Launch expected")
    };
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        panic!("immediate launch expected")
    };
    Completion::Launch(
        TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity(),
                HostLaunchState::Started(geometry),
            )),
        )
        .unwrap(),
    )
}

pub(super) fn pending_first_open(
    coordinator: &mut SiteCoordinator,
    request: u128,
    reference: u128,
) -> Box<crate::terminal_coordinator::PendingOperation> {
    let request = orchbee_client_server_protocol::TerminalOpenRequest::new(id(request), geometry());
    let PreparedOpen::Dispatch(pending) = coordinator.prepare_open(
        &request,
        Some(id(reference)),
        Some(worker(1)),
        Some(expiry()),
    ) else {
        panic!("first Open dispatch expected")
    };
    pending
}

pub(super) fn ready(
    prepared: PreparedTerminalTabCreate,
) -> orchbee_workbench_protocol::WorkbenchCommandResult {
    let PreparedTerminalTabCreate::Ready(result) = prepared else {
        panic!("ready tab.create result expected")
    };
    result
}

fn terminal_outcome(
    result: &orchbee_workbench_protocol::WorkbenchCommandResult,
) -> &WorkbenchTerminalOutcome {
    let WorkbenchCommandOutcome::TabCreated {
        terminal_outcome, ..
    } = &result.outcome
    else {
        panic!("final tab-created result expected: {:?}", result.outcome)
    };
    terminal_outcome
}

#[test]
fn workspace_create_uses_canonical_worker_and_cwd_without_touching_standalone() {
    let mut coordinator = site();
    let context_id = configure_workspace(&mut coordinator, "/tmp/exact/../workspace");
    let before = snapshot(&mut coordinator);
    let standalone_before = before
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == before.navigation.standalone_terminal_context_id
        })
        .unwrap()
        .clone();
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));
    let command = context_tab_command(&coordinator, 703, 3, context_id.clone(), "workspace");

    let pending = dispatch_tab(&mut coordinator, command.clone(), 704);
    assert!(!pending.settles_first());
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("terminal Launch expected")
    };
    assert_eq!(
        launch.cwd().map(|cwd| cwd.as_str()),
        Some("/tmp/exact/../workspace")
    );
    assert_eq!(launch.authorization().worker_id(), worker(1).worker_id);
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));

    let committed = snapshot(&mut coordinator);
    assert_eq!(committed.revision, WorkbenchRevision::new(4));
    assert_eq!(
        committed
            .terminal_contexts
            .iter()
            .find(|context| {
                context.terminal_context_id == committed.navigation.standalone_terminal_context_id
            })
            .unwrap(),
        &standalone_before
    );
    let workspace = committed
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == context_id)
        .unwrap();
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(workspace.tabs[0].label, "workspace");

    let completion = immediate_tab_completion(&pending);
    let completed = coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    assert!(!completed.standalone_changed);
    assert!(matches!(
        terminal_outcome(&completed.result),
        WorkbenchTerminalOutcome::Started { .. }
    ));
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));
    assert_eq!(
        snapshot(&mut coordinator)
            .terminal_contexts
            .iter()
            .find(|context| {
                context.terminal_context_id == committed.navigation.standalone_terminal_context_id
            })
            .unwrap(),
        &standalone_before
    );
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(command, None, None, None)),
        completed.result
    );
}

#[test]
fn workspace_create_rejects_a_different_current_worker_without_home_fallback() {
    let mut coordinator = site();
    let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
    let before = snapshot(&mut coordinator);
    let command = context_tab_command(&coordinator, 705, 3, context_id, "wrong worker");
    let mut incompatible = worker(1);
    incompatible.worker_id =
        orchbee_capability_node_protocol::WorkerId::from_bytes([8; 16]).unwrap();

    assert!(matches!(
        ready(coordinator.prepare_terminal_tab_create(
            command,
            Some(id(706)),
            Some(expiry()),
            Some(incompatible),
        ))
        .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        }
    ));
    assert_eq!(snapshot(&mut coordinator), before);
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));
    assert_eq!(coordinator.allocate_identity(), Some(1_u128.to_be_bytes()));
}

#[test]
fn workspace_completion_does_not_settle_a_pending_first_open() {
    let mut coordinator = site();
    let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
    let first_open = pending_first_open(&mut coordinator, 710, 711);
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Creating(ref reference) if reference == &id(711)
    ));
    let command = context_tab_command(&coordinator, 712, 4, context_id, "workspace");
    let workspace = dispatch_tab(&mut coordinator, command, 713);

    let completed = coordinator
        .complete_terminal_tab_create(workspace, Completion::Busy, Some(worker(1)))
        .unwrap();
    assert!(!completed.standalone_changed);
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Creating(ref reference) if reference == &id(711)
    ));

    assert!(matches!(
        coordinator
            .complete(first_open, Completion::Unknown, Some(worker(1)))
            .unwrap(),
        ServerMessage::TerminalCreateResult(_)
    ));
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            ref reference,
            outcome: OpenOutcome::OutcomeUnknown {},
        } if reference == &id(711)
    ));
}

#[test]
fn workspace_completion_preserves_a_settled_first_open() {
    let mut coordinator = site();
    let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
    let first_open = pending_first_open(&mut coordinator, 720, 721);
    let first_completion = immediate_completion(first_open.command());
    assert!(matches!(
        coordinator
            .complete(first_open, first_completion, Some(worker(1)))
            .unwrap(),
        ServerMessage::TerminalCreateResult(_)
    ));
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            ref reference,
            outcome: OpenOutcome::Opened {
                ref terminal_create_request_id,
            },
        } if reference == &id(721) && terminal_create_request_id == &id(721)
    ));
    let command = context_tab_command(&coordinator, 722, 4, context_id, "workspace");
    let workspace = dispatch_tab(&mut coordinator, command, 723);

    let completed = coordinator
        .complete_terminal_tab_create(workspace, Completion::Busy, Some(worker(1)))
        .unwrap();
    assert!(!completed.standalone_changed);
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            ref reference,
            outcome: OpenOutcome::Opened {
                ref terminal_create_request_id,
            },
        } if reference == &id(721) && terminal_create_request_id == &id(721)
    ));
}

#[test]
fn two_pending_tab_creates_publish_exact_bindings_and_settle_in_reverse_order() {
    let mut coordinator = site();
    let first_command = tab_command(&coordinator, 10, 0, "first");
    let first = dispatch_tab(&mut coordinator, first_command.clone(), 101);
    let first_authorization = tab_authorization(&first);
    let second_command = tab_command(&coordinator, 11, 1, "second");
    let second = dispatch_tab(&mut coordinator, second_command.clone(), 102);
    let second_authorization = tab_authorization(&second);

    let committed = snapshot(&mut coordinator);
    let context = &committed.terminal_contexts[0];
    assert_eq!(context.tabs.len(), 2);
    assert_eq!(context.tabs[0].label, "first");
    assert_eq!(context.tabs[1].label, "second");
    for (tab, authorization) in [
        (&context.tabs[0], first_authorization),
        (&context.tabs[1], second_authorization),
    ] {
        let PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } = &tab.root
        else {
            panic!("simple Panel expected")
        };
        assert!(child_presentations.is_empty());
        assert_eq!(primary_presentation.label, tab.label);
        assert_eq!(
            primary_presentation.binding,
            crate::site_coordinator::presentation_binding(authorization)
        );
    }
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Creating(ref reference) if reference == &id(101)
    ));

    let second_result = coordinator
        .complete_terminal_tab_create(second, Completion::Busy, Some(worker(1)))
        .unwrap()
        .result;
    assert_eq!(
        terminal_outcome(&second_result),
        &WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::ServerBusy,
        }
    );
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Creating(ref reference) if reference == &id(101)
    ));

    let first_completion = immediate_tab_completion(&first);
    let first_result = coordinator
        .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
        .unwrap()
        .result;
    assert_eq!(
        terminal_outcome(&first_result),
        &WorkbenchTerminalOutcome::Started {
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        }
    );
    assert!(matches!(
        coordinator.prepare_open(
            &orchbee_client_server_protocol::TerminalOpenRequest::new(id(200), geometry()),
            None,
            Some(worker(1)),
            None,
        ),
        PreparedOpen::Ready(OpenOutcome::Opened {
            terminal_create_request_id,
        }) if terminal_create_request_id == id(101)
    ));

    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(first_command, None, None, None)),
        first_result
    );
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(second_command, None, None, None)),
        second_result
    );
}

#[test]
fn pending_and_final_replay_precede_missing_candidates_worker_and_conflict() {
    let mut coordinator = site();
    let command = tab_command(&coordinator, 20, 0, "replay");
    let pending = dispatch_tab(&mut coordinator, command.clone(), 201);
    let pending_replay =
        ready(coordinator.prepare_terminal_tab_create(command.clone(), None, None, None));
    assert!(matches!(
        pending_replay.outcome,
        WorkbenchCommandOutcome::TabCreatePending { revision, .. }
            if revision == WorkbenchRevision::new(1)
    ));

    let mut conflict = command.clone();
    conflict.action = WorkbenchAction::TabCreate {
        placement: None,
        automatic_label: false,
        terminal_context_id: coordinator
            .workbench
            .as_ref()
            .unwrap()
            .standalone_membership()
            .unwrap()
            .terminal_context_id()
            .clone(),
        label: WorkbenchLabel::parse("different").unwrap(),
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        launch: None,
        profile: None,
        options: Default::default(),
    };
    assert!(matches!(
        ready(coordinator.prepare_terminal_tab_create(conflict, None, None, None)).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));

    let final_result = coordinator
        .complete_terminal_tab_create(pending, Completion::Unknown, Some(worker(1)))
        .unwrap()
        .result;
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(command, None, None, None)),
        final_result
    );
    assert_eq!(coordinator.allocate_identity(), Some(2_u128.to_be_bytes()));
}

#[test]
fn launch_selection_reaches_the_worker_command_and_replay_identity() {
    let mut coordinator = site();
    let mut command = tab_command(&coordinator, 21, 0, "launch");
    let WorkbenchAction::TabCreate {
        launch, options, ..
    } = &mut command.action
    else {
        panic!("tab.create action expected")
    };
    // A maximum-size selection exercises the cross-layer conversion at the
    // shared 2,048-byte bound: executable plus arguments total exactly 2,048.
    let long = "x".repeat(2_048 - "/bin/sh".len() - "-c".len() - "漢字".len());
    options.cwd = Some(WorkbenchDirectoryPath::parse("C:\\launch space").unwrap());
    options.environment = orchbee_workbench_protocol::LaunchEnvironment::new(vec![
        ("KEY".into(), "literal=值".into()),
        ("EMPTY".into(), String::new()),
    ])
    .unwrap();
    *launch = Some(
        orchbee_workbench_protocol::LaunchSelection::new(
            "/bin/sh".to_owned(),
            vec!["-c".to_owned(), "漢字".to_owned(), long.clone()],
        )
        .unwrap(),
    );

    // The pending OBCN command carries the selection verbatim.
    let pending = dispatch_tab(&mut coordinator, command.clone(), 211);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(authorized) = pending.command() else {
        panic!("terminal Launch expected")
    };
    assert_eq!(authorized.cwd().unwrap().as_str(), "C:\\launch space");
    assert_eq!(
        authorized.environment().entries(),
        &[
            ("KEY".into(), "literal=值".into()),
            ("EMPTY".into(), String::new())
        ]
    );
    let selected = authorized.launch().expect("explicit launch selection");
    assert_eq!(selected.executable(), "/bin/sh");
    assert_eq!(
        selected.arguments(),
        ["-c".to_owned(), "漢字".to_owned(), long].as_slice()
    );

    // An identical replay returns the pending outcome without dispatching a
    // second launch.
    let replay = ready(coordinator.prepare_terminal_tab_create(command.clone(), None, None, None));
    assert!(matches!(
        replay.outcome,
        WorkbenchCommandOutcome::TabCreatePending { revision, .. }
            if revision == WorkbenchRevision::new(1)
    ));
    // The probe consumes the identity it returns, so the next free value is 3.
    assert_eq!(coordinator.allocate_identity(), Some(2_u128.to_be_bytes()));

    // The same request id with a different selection conflicts before effect.
    let mut conflict = command.clone();
    let WorkbenchAction::TabCreate { launch, .. } = &mut conflict.action else {
        panic!("tab.create action expected")
    };
    // The conflicting selection also exercises the 128-argument count bound.
    *launch = Some(
        orchbee_workbench_protocol::LaunchSelection::new(
            "/bin/zsh".to_owned(),
            vec![String::new(); 128],
        )
        .unwrap(),
    );
    assert!(matches!(
        ready(coordinator.prepare_terminal_tab_create(conflict, None, None, None)).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    assert_eq!(coordinator.allocate_identity(), Some(3_u128.to_be_bytes()));

    for change_cwd in [false, true] {
        let mut changed = command.clone();
        let WorkbenchAction::TabCreate { options, .. } = &mut changed.action else {
            panic!("launch action");
        };
        if change_cwd {
            options.cwd = Some(WorkbenchDirectoryPath::parse("C:\\other").unwrap());
        } else {
            options.environment = orchbee_workbench_protocol::LaunchEnvironment::empty();
        }
        assert!(matches!(
            ready(coordinator.prepare_terminal_tab_create(changed, None, None, None)).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
    }
}

#[test]
fn workbench_and_terminal_preflight_refusals_leave_all_three_site_owners_unchanged() {
    let mut coordinator = site();
    let initial = snapshot(&mut coordinator);
    let command = tab_command(&coordinator, 30, 0, "preflight");

    assert!(matches!(
        ready(coordinator.prepare_terminal_tab_create(
            command.clone(),
            Some(id(301)),
            Some(expiry()),
            None,
        ))
        .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        }
    ));
    for (candidate, expiry) in [(None, Some(super::expiry())), (Some(id(301)), None)] {
        assert!(matches!(
            ready(coordinator.prepare_terminal_tab_create(
                command.clone(),
                candidate,
                expiry,
                Some(worker(1)),
            ))
            .outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServerBusy,
            }
        ));
    }
    coordinator.terminals.set_next_identity_for_test(None);
    assert!(matches!(
        ready(coordinator.prepare_terminal_tab_create(
            command.clone(),
            Some(id(301)),
            Some(expiry()),
            Some(worker(1)),
        ))
        .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServerBusy,
        }
    ));
    assert_eq!(snapshot(&mut coordinator), initial);
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));

    coordinator.terminals.set_next_identity_for_test(Some(1));
    let _pending = dispatch_tab(&mut coordinator, command, 301);
    assert_membership(&mut coordinator, 1);
}

#[test]
fn missing_retired_and_inconsistent_workbench_use_service_unavailable_without_terminal_effects() {
    let command_for = |site_id| WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 40)).unwrap(),
        site_id,
        expected_revision: WorkbenchRevision::new(0),
        action: WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                &format!("{:032x}", 1),
            )
            .unwrap(),
            label: WorkbenchLabel::parse("missing").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    let mut missing = SiteCoordinator::new(None);
    assert!(matches!(
        ready(missing.prepare_terminal_tab_create(
            command_for(SiteId::parse(&format!("{:032x}", 999)).unwrap()),
            None,
            None,
            None,
        ))
        .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
        }
    ));
    assert_eq!(missing.allocate_identity(), Some(1_u128.to_be_bytes()));

    let mut retired = site();
    retired
        .workbench
        .as_mut()
        .unwrap()
        .retire_standalone_composition();
    let retired_command = tab_command(&retired, 41, 0, "retired");
    assert!(matches!(
        ready(retired.prepare_terminal_tab_create(retired_command, None, None, None)).outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
        }
    ));
    assert_eq!(retired.allocate_identity(), Some(1_u128.to_be_bytes()));

    let mut inconsistent = site();
    let _metadata = inconsistent
        .workbench
        .as_mut()
        .unwrap()
        .prepare_standalone_intent(PresentationBinding {
            terminal_id: WorkbenchTerminalId::parse(&format!("{:032x}", 501)).unwrap(),
            worker_id: WorkbenchWorkerId::parse(&format!("{:032x}", 502)).unwrap(),
        })
        .unwrap()
        .commit();
    let inconsistent_command = tab_command(&inconsistent, 42, 1, "inconsistent");
    assert!(matches!(
        ready(inconsistent.prepare_terminal_tab_create(
            inconsistent_command,
            Some(id(401)),
            Some(expiry()),
            Some(worker(1)),
        ))
        .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
        }
    ));
    assert_eq!(inconsistent.allocate_identity(), Some(1_u128.to_be_bytes()));
    assert!(matches!(inconsistent.standalone, StandaloneTerminal::Empty));
    assert!(matches!(
        inconsistent.prepare_open(
            &orchbee_client_server_protocol::TerminalOpenRequest::new(id(402), geometry()),
            Some(id(403)),
            Some(worker(1)),
            Some(expiry()),
        ),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy,
        })
    ));
}

#[test]
fn composition_guard_checks_both_empty_directions_and_the_exact_bootstrap_binding() {
    let mut slot_without_membership = site();
    let retained =
        dispatch(slot_without_membership.prepare(create(430), Some(worker(1)), Some(expiry())));
    slot_without_membership.standalone = StandaloneTerminal::Creating(id(430));
    let command = tab_command(&slot_without_membership, 431, 0, "reverse mismatch");
    assert!(matches!(
        ready(slot_without_membership.prepare_terminal_tab_create(
            command,
            Some(id(432)),
            Some(expiry()),
            Some(worker(1)),
        ))
        .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
        }
    ));
    assert_eq!(retained.create_reference(), &id(430));
    assert_eq!(
        slot_without_membership.allocate_identity(),
        Some(2_u128.to_be_bytes())
    );

    let mut wrong_binding = site();
    let first = dispatch(wrong_binding.prepare(create(440), Some(worker(1)), Some(expiry())));
    let first_authorization = launch_authorization(&first);
    let second = dispatch(wrong_binding.prepare(create(441), Some(worker(1)), Some(expiry())));
    let _metadata = wrong_binding
        .workbench
        .as_mut()
        .unwrap()
        .prepare_standalone_intent(
            crate::site_coordinator::presentation_binding(first_authorization).unwrap(),
        )
        .unwrap()
        .commit();
    wrong_binding.standalone = StandaloneTerminal::Creating(id(441));
    let command = tab_command(&wrong_binding, 442, 1, "wrong bootstrap binding");
    assert!(matches!(
        ready(wrong_binding.prepare_terminal_tab_create(
            command,
            Some(id(443)),
            Some(expiry()),
            Some(worker(1)),
        ))
        .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
        }
    ));
    assert_eq!(first.create_reference(), &id(440));
    assert_eq!(second.create_reference(), &id(441));
    assert_eq!(
        wrong_binding.allocate_identity(),
        Some(3_u128.to_be_bytes())
    );
}

#[test]
fn open_and_tab_create_share_the_first_slot_in_both_orders() {
    let mut open_wins = site();
    let old_revision = tab_command(&open_wins, 50, 0, "loses");
    let PreparedOpen::Dispatch(open_pending) = open_wins.prepare_open(
        &orchbee_client_server_protocol::TerminalOpenRequest::new(id(501), geometry()),
        Some(id(502)),
        Some(worker(1)),
        Some(expiry()),
    ) else {
        panic!("Open dispatch expected")
    };
    assert!(matches!(
        ready(open_wins.prepare_terminal_tab_create(
            old_revision,
            Some(id(503)),
            Some(expiry()),
            Some(worker(1)),
        ))
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
    let completion = {
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = open_pending.command() else {
            panic!("Launch expected")
        };
        let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
            panic!("immediate expected")
        };
        Completion::Launch(
            TerminalLaunchResult::new(
                launch.authorization(),
                TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                    identity(),
                    HostLaunchState::Started(geometry),
                )),
            )
            .unwrap(),
        )
    };
    open_wins
        .complete(open_pending, completion, Some(worker(1)))
        .unwrap();
    assert_membership(&mut open_wins, 1);

    let mut tab_wins = site();
    let command = tab_command(&tab_wins, 51, 0, "wins");
    let tab_pending = dispatch_tab(&mut tab_wins, command, 504);
    assert!(matches!(
        tab_wins.prepare_open(
            &orchbee_client_server_protocol::TerminalOpenRequest::new(id(505), geometry()),
            Some(id(506)),
            Some(worker(1)),
            Some(expiry()),
        ),
        PreparedOpen::Wait
    ));
    tab_wins
        .complete_terminal_tab_create(tab_pending, Completion::Busy, Some(worker(1)))
        .unwrap();
    assert!(matches!(
        tab_wins.prepare_open(
            &orchbee_client_server_protocol::TerminalOpenRequest::new(id(507), geometry()),
            None,
            Some(worker(1)),
            None,
        ),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy,
        })
    ));
    assert_membership(&mut tab_wins, 1);
}

#[test]
fn unrelated_workbench_retirement_does_not_cancel_pending_settlement_or_settled_open() {
    let mut coordinator = site();
    let command = tab_command(&coordinator, 52, 0, "retained");
    let pending = dispatch_tab(&mut coordinator, command, 520);
    coordinator
        .workbench
        .as_mut()
        .unwrap()
        .retire_standalone_composition();

    let completion = immediate_tab_completion(&pending);
    let result = coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap()
        .result;
    assert!(matches!(
        terminal_outcome(&result),
        WorkbenchTerminalOutcome::Started { .. }
    ));
    assert!(matches!(
        coordinator.prepare_open(
            &orchbee_client_server_protocol::TerminalOpenRequest::new(id(521), geometry()),
            None,
            Some(worker(1)),
            None,
        ),
        PreparedOpen::Ready(OpenOutcome::Opened {
            terminal_create_request_id,
        }) if terminal_create_request_id == id(520)
    ));
}

#[test]
fn completion_maps_every_terminal_reason_and_worker_generation_fence() {
    let cases = [
        (
            CompletionCase::Busy,
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::ServerBusy,
            },
        ),
        (
            CompletionCase::Unavailable,
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
        (
            CompletionCase::Capacity,
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::ServerBusy,
            },
        ),
        (
            CompletionCase::HostUnavailable,
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
        (
            CompletionCase::HostRejected,
            WorkbenchTerminalOutcome::Failed {
                reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
            },
        ),
        (
            CompletionCase::Unknown,
            WorkbenchTerminalOutcome::OutcomeUnknown {},
        ),
        (
            CompletionCase::AcceptedCurrent,
            WorkbenchTerminalOutcome::Started {
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            },
        ),
        (
            CompletionCase::AcceptedAfterReplacement,
            WorkbenchTerminalOutcome::OutcomeUnknown {},
        ),
    ];
    let executable = if cfg!(windows) {
        r"C:\missing\program.exe"
    } else {
        "/missing/program"
    };
    for (index, (case, expected)) in cases.into_iter().enumerate() {
        let mut coordinator = site();
        let mut command = tab_command(&coordinator, 600 + index as u128, 0, "mapping");
        if let WorkbenchAction::TabCreate { launch, .. } = &mut command.action {
            *launch = Some(
                orchbee_workbench_protocol::LaunchSelection::new(
                    executable.to_owned(),
                    vec!["--token=secret".to_owned()],
                )
                .unwrap(),
            );
        }
        let pending = dispatch_tab(&mut coordinator, command, 700 + index as u128);
        let authorization = tab_authorization(&pending);
        let completion = completion(case, &pending, authorization);
        let current = if matches!(case, CompletionCase::AcceptedAfterReplacement) {
            Some(worker(2))
        } else {
            Some(worker(1))
        };
        let result = coordinator
            .complete_terminal_tab_create(pending, completion, current)
            .unwrap()
            .result;
        assert_eq!(terminal_outcome(&result), &expected, "case {case:?}");
        assert!(matches!(
            coordinator.standalone,
            StandaloneTerminal::Settled { .. }
        ));
        // Only a definite failure publishes one more revision naming the executable, never its
        // arguments; pending, unavailable and unknown outcomes keep the committed projection.
        let snapshot = coordinator.workbench.as_ref().unwrap().snapshot();
        let failed = matches!(expected, WorkbenchTerminalOutcome::Failed { .. });
        assert_eq!(
            snapshot.revision.value(),
            1 + u64::from(failed),
            "case {case:?}"
        );
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &snapshot.terminal_contexts[0].tabs[0].root
        else {
            panic!("created Panel expected")
        };
        assert_eq!(
            primary_presentation.launch_failure,
            failed.then(|| orchbee_workbench_protocol::PresentationLaunchFailure {
                executable: Some(executable.to_owned()),
                reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
            }),
            "case {case:?}"
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum CompletionCase {
    Busy,
    Unavailable,
    Capacity,
    HostUnavailable,
    HostRejected,
    Unknown,
    AcceptedCurrent,
    AcceptedAfterReplacement,
}

fn completion(
    case: CompletionCase,
    pending: &crate::site_coordinator::PendingTerminalTabCreate,
    authorization: TerminalOperationAuthorization,
) -> Completion {
    match case {
        CompletionCase::Busy => Completion::Busy,
        CompletionCase::Unavailable => Completion::Unavailable,
        CompletionCase::Unknown => Completion::Unknown,
        CompletionCase::Capacity => Completion::Launch(
            TerminalLaunchResult::new(
                authorization,
                TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::CapacityExceeded),
            )
            .unwrap(),
        ),
        CompletionCase::HostUnavailable => Completion::Launch(
            TerminalLaunchResult::new(
                authorization,
                TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff),
            )
            .unwrap(),
        ),
        CompletionCase::HostRejected => Completion::Launch(
            TerminalLaunchResult::new(
                authorization,
                TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
                    host_epoch: HostEpoch::from_bytes([4; 16]).unwrap(),
                }),
            )
            .unwrap(),
        ),
        CompletionCase::AcceptedCurrent | CompletionCase::AcceptedAfterReplacement => {
            immediate_tab_completion(pending)
        }
    }
}

#[test]
fn foreign_terminal_and_workbench_tokens_or_wrapper_mismatch_preserve_both_owners() {
    let command = tab_command(&site(), 80, 0, "token");

    let mut terminal_owner = site();
    let terminal_pending = dispatch_tab(&mut terminal_owner, command.clone(), 801);
    let before = snapshot(&mut terminal_owner);
    let mut foreign_terminal_owner = site();
    let foreign_terminal = dispatch_tab(&mut foreign_terminal_owner, command.clone(), 801);
    assert_eq!(
        terminal_owner.complete_terminal_tab_create(
            foreign_terminal,
            Completion::Unknown,
            Some(worker(1)),
        ),
        Err(PostcommitCoordinationError)
    );
    assert_eq!(snapshot(&mut terminal_owner), before);
    assert!(matches!(
        ready(terminal_owner.prepare_terminal_tab_create(command.clone(), None, None, None,))
            .outcome,
        WorkbenchCommandOutcome::TabCreatePending { .. }
    ));
    drop(terminal_pending);

    let mut workbench_owner = site();
    let mut workbench_pending = dispatch_tab(&mut workbench_owner, command.clone(), 801);
    let mut foreign_workbench_owner = site();
    let mut foreign_workbench = dispatch_tab(&mut foreign_workbench_owner, command.clone(), 801);
    workbench_pending.swap_workbench_token_for_test(&mut foreign_workbench);
    let before = snapshot(&mut workbench_owner);
    assert_eq!(
        workbench_owner.complete_terminal_tab_create(
            workbench_pending,
            Completion::Unknown,
            Some(worker(1)),
        ),
        Err(PostcommitCoordinationError)
    );
    assert_eq!(snapshot(&mut workbench_owner), before);
    assert!(matches!(
        ready(workbench_owner.prepare_terminal_tab_create(command.clone(), None, None, None,))
            .outcome,
        WorkbenchCommandOutcome::TabCreatePending { .. }
    ));
    drop(foreign_workbench);

    let mut association_owner = site();
    let mut association = dispatch_tab(&mut association_owner, command.clone(), 801);
    association.replace_reference_for_test(id(999));
    let before = snapshot(&mut association_owner);
    assert_eq!(
        association_owner.complete_terminal_tab_create(
            association,
            Completion::Unknown,
            Some(worker(1)),
        ),
        Err(PostcommitCoordinationError)
    );
    assert_eq!(snapshot(&mut association_owner), before);
    assert!(matches!(
        ready(association_owner.prepare_terminal_tab_create(command, None, None, None)).outcome,
        WorkbenchCommandOutcome::TabCreatePending { .. }
    ));
}

// Construct already committed membership through the Workbench owner. These are composition
// fixtures, not Move traces: production reordering and its pending-token history belong to Move.
fn bootstrap_at_position(position: usize, settled: bool) -> SiteCoordinator {
    use orchbee_server::{WorkbenchTabCreateBegin, WorkbenchTabCreatePreparation};

    let mut coordinator = site();
    let mut bootstrap = None;
    let bindings: Vec<_> = (0..4)
        .map(|index| {
            let pending =
                dispatch(coordinator.prepare(create(900 + index), Some(worker(1)), Some(expiry())));
            let binding =
                crate::site_coordinator::presentation_binding(launch_authorization(&pending))
                    .unwrap();
            if index == 0 {
                bootstrap = Some(pending);
            }
            binding
        })
        .collect();
    let mut order = [1, 2, 3];
    if let Some(member) = order.get_mut(position) {
        *member = 0;
    }
    for (index, binding_index) in order.into_iter().enumerate() {
        let command = tab_command(&coordinator, 910 + index as u128, index as u64, "fixture");
        let workbench = coordinator.workbench.as_mut().unwrap();
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(workbench.site_id().clone());
        let WorkbenchTabCreateBegin::Admitted(admission) =
            workbench.begin_terminal_tab_create(Some(&grant), command)
        else {
            panic!("fixture admission expected")
        };
        let WorkbenchTabCreatePreparation::Prepared(plan) =
            admission.finish(bindings[binding_index].clone())
        else {
            panic!("fixture publication expected")
        };
        let _pending = plan.commit().into_pending();
    }
    coordinator.standalone = StandaloneTerminal::Creating(id(900));
    if settled {
        coordinator
            .complete(bootstrap.unwrap(), Completion::Unknown, Some(worker(1)))
            .unwrap();
    }
    coordinator
}

#[test]
fn bootstrap_binding_at_every_position_preserves_open_join_and_later_exact_settlement() {
    for position in 0..3 {
        for settled in [false, true] {
            let mut coordinator = bootstrap_at_position(position, settled);
            let before = snapshot(&mut coordinator);
            let open = coordinator.prepare_open(
                &orchbee_client_server_protocol::TerminalOpenRequest::new(id(950), geometry()),
                None,
                Some(worker(1)),
                None,
            );
            if settled {
                assert!(matches!(
                    open,
                    PreparedOpen::Ready(OpenOutcome::OutcomeUnknown {})
                ));
            } else {
                assert!(
                    matches!(open, PreparedOpen::Wait),
                    "bootstrap position {position}"
                );
            }
            assert_eq!(snapshot(&mut coordinator), before);
            let command = tab_command(&coordinator, 951, 3, "later");
            let pending = dispatch_tab(&mut coordinator, command.clone(), 952);
            assert!(!pending.settles_first());
            let committed = coordinator
                .workbench
                .as_ref()
                .unwrap()
                .standalone_membership()
                .unwrap();
            coordinator
                .workbench
                .as_mut()
                .unwrap()
                .retire_standalone_composition();
            let result = coordinator
                .complete_terminal_tab_create(pending, Completion::Unknown, Some(worker(1)))
                .unwrap()
                .result;
            assert!(
                matches!(result.outcome, WorkbenchCommandOutcome::TabCreated { revision, terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {}, .. } if revision == WorkbenchRevision::new(4))
            );
            assert_eq!(
                coordinator
                    .workbench
                    .as_ref()
                    .unwrap()
                    .standalone_membership()
                    .unwrap(),
                committed
            );
            assert!(
                matches!(&coordinator.standalone,
                    StandaloneTerminal::Creating(reference) if !settled && reference == &id(900)
                ) || matches!(&coordinator.standalone,
                    StandaloneTerminal::Settled { reference, outcome: OpenOutcome::OutcomeUnknown {} } if settled && reference == &id(900)
                )
            );
            assert!(matches!(
                ready(coordinator.prepare_terminal_tab_create(command, None, None, None)).outcome,
                WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
                }
            ));
            assert_eq!(coordinator.allocate_identity(), Some(6_u128.to_be_bytes()));
        }
    }
}

#[test]
fn absent_bootstrap_binding_or_authorization_retires_without_consuming_terminal_intent() {
    for missing_authorization in [false, true] {
        let mut coordinator =
            bootstrap_at_position(if missing_authorization { 1 } else { 3 }, false);
        let reference = if missing_authorization {
            id(999)
        } else {
            id(900)
        };
        coordinator.standalone = StandaloneTerminal::Creating(reference.clone());
        let before = coordinator
            .workbench
            .as_ref()
            .unwrap()
            .standalone_membership()
            .unwrap();
        let command = tab_command(&coordinator, 960, 3, "refused");
        assert!(matches!(
            ready(coordinator.prepare_terminal_tab_create(
                command,
                Some(id(961)),
                Some(expiry()),
                Some(worker(1))
            ))
            .outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
            }
        ));
        assert_eq!(
            coordinator
                .workbench
                .as_ref()
                .unwrap()
                .standalone_membership()
                .unwrap(),
            before
        );
        assert!(
            matches!(&coordinator.standalone, StandaloneTerminal::Creating(current) if current == &reference)
        );
        assert!(
            coordinator
                .terminals
                .retained_authorization(&id(961))
                .is_none()
        );
        assert_eq!(coordinator.allocate_identity(), Some(5_u128.to_be_bytes()));
    }
}

#[test]
fn replay_conflict_and_revision_refusal_precede_bootstrap_composition_failure() {
    for settled in [false, true] {
        let mut coordinator = site();
        let command = tab_command(&coordinator, 970, 0, "retained");
        let pending = dispatch_tab(&mut coordinator, command.clone(), 971);
        let retained = if settled {
            coordinator
                .complete_terminal_tab_create(pending, Completion::Unknown, Some(worker(1)))
                .unwrap()
                .result
        } else {
            ready(coordinator.prepare_terminal_tab_create(command.clone(), None, None, None))
        };
        coordinator.standalone = StandaloneTerminal::Creating(id(999));
        let before = snapshot(&mut coordinator);
        assert_eq!(
            ready(coordinator.prepare_terminal_tab_create(command.clone(), None, None, None)),
            retained
        );
        let mut conflict = command.clone();
        conflict.expected_revision = WorkbenchRevision::new(1);
        assert!(matches!(
            ready(coordinator.prepare_terminal_tab_create(conflict, None, None, None)).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
        let stale = tab_command(&coordinator, 972, 0, "stale");
        assert!(matches!(
            ready(coordinator.prepare_terminal_tab_create(stale, None, None, None)).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RevisionMismatch,
                ..
            }
        ));
        assert_eq!(snapshot(&mut coordinator), before);
        let admitted = tab_command(&coordinator, 973, 1, "composition");
        assert!(matches!(
            ready(coordinator.prepare_terminal_tab_create(admitted, None, None, None)).outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
            }
        ));
        assert!(matches!(
            ready(coordinator.prepare_terminal_tab_create(command, None, None, None)).outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
            }
        ));
        let read = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 974)).unwrap(),
        });
        assert!(
            matches!(coordinator.execute_workbench(read.try_into().unwrap(), false),
                Some(WorkbenchResponse::Snapshot(result)) if matches!(result.outcome, WorkbenchSnapshotOutcome::Unavailable { .. })
            )
        );
        assert!(
            matches!(&coordinator.standalone, StandaloneTerminal::Creating(reference) if reference == &id(999))
        );
        assert_eq!(coordinator.allocate_identity(), Some(2_u128.to_be_bytes()));
    }
}

#[test]
fn missing_bootstrap_authorization_rejects_postcommit_completion_without_partial_settlement() {
    let mut coordinator = site();
    let command = tab_command(&coordinator, 980, 0, "pending");
    let pending = dispatch_tab(&mut coordinator, command.clone(), 981);
    let completion = immediate_tab_completion(&pending);
    coordinator.standalone = StandaloneTerminal::Creating(id(999));
    let before = snapshot(&mut coordinator);
    assert_eq!(
        coordinator.complete_terminal_tab_create(pending, completion, Some(worker(1))),
        Err(PostcommitCoordinationError)
    );
    assert_eq!(snapshot(&mut coordinator), before);
    assert_eq!(coordinator.viewer_target(&id(981), worker(1)), None);
    assert!(matches!(
        ready(coordinator.prepare_terminal_tab_create(command, None, None, None)).outcome,
        WorkbenchCommandOutcome::TabCreatePending { .. }
    ));
    assert!(
        matches!(&coordinator.standalone, StandaloneTerminal::Creating(reference) if reference == &id(999))
    );
}

#[test]
fn first_create_settles_before_later_create_and_retirement_preserves_its_open_outcome() {
    let mut coordinator = site();
    let first_command = tab_command(&coordinator, 990, 0, "first");
    let first = dispatch_tab(&mut coordinator, first_command, 991);
    let later_command = tab_command(&coordinator, 992, 1, "later");
    let later = dispatch_tab(&mut coordinator, later_command, 993);
    assert!(first.settles_first());
    assert!(!later.settles_first());
    let first_completion = immediate_tab_completion(&first);
    coordinator
        .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
        .unwrap();
    coordinator
        .workbench
        .as_mut()
        .unwrap()
        .retire_standalone_composition();
    let later_result = coordinator
        .complete_terminal_tab_create(later, Completion::Busy, Some(worker(1)))
        .unwrap()
        .result;
    assert!(matches!(
        terminal_outcome(&later_result),
        WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::ServerBusy
        }
    ));
    assert!(matches!(coordinator.prepare_open(
        &orchbee_client_server_protocol::TerminalOpenRequest::new(id(994), geometry()), None, Some(worker(1)), None,
    ), PreparedOpen::Ready(OpenOutcome::Opened { terminal_create_request_id }) if terminal_create_request_id == id(991)));
    assert_eq!(coordinator.allocate_identity(), Some(3_u128.to_be_bytes()));
}

#[test]
fn per_launch_cwd_overrides_workspace_without_changing_placement_or_directory() {
    let mut coordinator = site();
    let context = configure_workspace(&mut coordinator, "/tmp/workspace");
    let mut command = context_tab_command(&coordinator, 9901, 3, context.clone(), "override");
    let WorkbenchAction::TabCreate { options, .. } = &mut command.action else {
        panic!("tab");
    };
    options.cwd = Some(WorkbenchDirectoryPath::parse("C:\\other").unwrap());
    options.environment =
        orchbee_workbench_protocol::LaunchEnvironment::new(vec![("KEY".into(), "value".into())])
            .unwrap();
    let pending = dispatch_tab(&mut coordinator, command, 9902);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch");
    };
    assert_eq!(launch.cwd().unwrap().as_str(), "C:\\other");
    assert!(launch.launch().is_none());
    assert_eq!(launch.authorization().worker_id(), worker(1).worker_id);
    let completion = immediate_completion(pending.command());
    coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    let revision = snapshot(&mut coordinator).revision.value();
    let next = context_tab_command(&coordinator, 9903, revision, context, "inherited");
    let pending = dispatch_tab(&mut coordinator, next, 9904);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch");
    };
    assert_eq!(launch.cwd().unwrap().as_str(), "/tmp/workspace");
    assert!(launch.environment().entries().is_empty());
}
