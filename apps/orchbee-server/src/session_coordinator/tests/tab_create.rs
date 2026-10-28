use super::*;
use crate::test_support::WorkbenchFixture;
mod tab_move;
use crate::session_coordinator::{PostcommitCoordinationError, PreparedTerminalTabCreate};
use orchbee_capability_node_protocol::{
    HostLaunchAccepted, TerminalLaunchFailure, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchRejection, TerminalLaunchResult,
};
use orchbee_server::{WorkbenchAuthority, WorkbenchGrant};
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    WorkbenchTerminalContextId, WorkbenchTerminalFailureReason, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason, WorkerId as WorkbenchWorkerId,
};

pub(super) fn context_tab_command(
    coordinator: &SessionCoordinator,
    request: u128,
    revision: u64,
    terminal_context_id: WorkbenchTerminalContextId,
    label: &str,
) -> WorkbenchCommand {
    let workbench = coordinator.workbench.as_ref().unwrap();
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        session_id: workbench.session_id().clone(),
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
    coordinator: &SessionCoordinator,
    request: u128,
    revision: u64,
    label: &str,
) -> WorkbenchCommand {
    let terminal_context_id = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .fixture_membership()
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
    coordinator: &mut SessionCoordinator,
    path: &str,
) -> WorkbenchTerminalContextId {
    let workbench = coordinator.workbench.as_mut().unwrap();
    let session_id = workbench.session_id().clone();
    let navigation = WorkbenchGrant::manage_navigation(session_id.clone());
    let project = submit_navigation(
        workbench,
        &navigation,
        WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 700)).unwrap(),
            session_id: session_id.clone(),
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
            session_id: session_id.clone(),
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
        session_id,
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
    coordinator: &mut SessionCoordinator,
    command: WorkbenchCommand,
    reference: u128,
) -> crate::session_coordinator::PendingTerminalTabCreate {
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
    pending: &crate::session_coordinator::PendingTerminalTabCreate,
) -> TerminalOperationAuthorization {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("terminal Launch expected")
    };
    launch.authorization()
}

fn immediate_tab_completion(
    pending: &crate::session_coordinator::PendingTerminalTabCreate,
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

pub(super) fn pending_first_tab(
    coordinator: &mut SessionCoordinator,
    request: u128,
    reference: u128,
) -> crate::session_coordinator::PendingTerminalTabCreate {
    let revision = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .snapshot()
        .revision
        .value();
    let command = tab_command(coordinator, request, revision, "1");
    dispatch_tab(coordinator, command, reference)
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
fn workspace_create_uses_canonical_worker_and_cwd_without_touching_initial_workspace() {
    let mut coordinator = session();
    let context_id = configure_workspace(&mut coordinator, "/tmp/exact/../workspace");
    let before = snapshot(&mut coordinator);
    let initial_workspace_before = before
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == before.navigation.workspaces[0].terminal_context_id
        })
        .unwrap()
        .clone();

    let command = context_tab_command(&coordinator, 703, 3, context_id.clone(), "workspace");

    let pending = dispatch_tab(&mut coordinator, command.clone(), 704);

    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("terminal Launch expected")
    };
    assert_eq!(
        launch.cwd().map(|cwd| cwd.as_str()),
        Some("/tmp/exact/../workspace")
    );
    assert_eq!(launch.authorization().worker_id(), worker(1).worker_id);

    let committed = snapshot(&mut coordinator);
    assert_eq!(committed.revision, WorkbenchRevision::new(4));
    assert_eq!(
        committed
            .terminal_contexts
            .iter()
            .find(|context| {
                context.terminal_context_id
                    == committed.navigation.workspaces[0].terminal_context_id
            })
            .unwrap(),
        &initial_workspace_before
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

    assert!(matches!(
        terminal_outcome(&completed.result),
        WorkbenchTerminalOutcome::Started { .. }
    ));

    assert_eq!(
        snapshot(&mut coordinator)
            .terminal_contexts
            .iter()
            .find(|context| {
                context.terminal_context_id
                    == committed.navigation.workspaces[0].terminal_context_id
            })
            .unwrap(),
        &initial_workspace_before
    );
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(command, None, None, None)),
        completed.result
    );
}

#[test]
fn workspace_create_rejects_a_different_current_worker_without_home_fallback() {
    let mut coordinator = session();
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

    assert_eq!(coordinator.allocate_identity(), Some(1_u128.to_be_bytes()));
}

#[test]
fn two_pending_tab_creates_publish_exact_bindings_and_settle_in_reverse_order() {
    let mut coordinator = session();
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
            crate::session_coordinator::presentation_binding(authorization)
        );
    }

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
    let mut coordinator = session();
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
            .fixture_membership()
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
    let mut coordinator = session();
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
fn workbench_and_terminal_preflight_refusals_leave_all_three_session_owners_unchanged() {
    let mut coordinator = session();
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

    coordinator.terminals.set_next_identity_for_test(Some(1));
    let _pending = dispatch_tab(&mut coordinator, command, 301);
    assert_membership(&mut coordinator, 1);
}

#[test]
fn missing_and_retired_workbench_use_service_unavailable_without_terminal_effects() {
    let command_for = |session_id| WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 40)).unwrap(),
        session_id,
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
    let mut missing = SessionCoordinator::new(None);
    assert!(matches!(
        ready(missing.prepare_terminal_tab_create(
            command_for(SessionId::parse(&format!("{:032x}", 999)).unwrap()),
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

    let mut retired = session();
    retired
        .workbench
        .as_mut()
        .unwrap()
        .retire_terminal_membership();
    let retired_command = tab_command(&retired, 41, 0, "retired");
    assert!(matches!(
        ready(retired.prepare_terminal_tab_create(retired_command, None, None, None)).outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
        }
    ));
    assert_eq!(retired.allocate_identity(), Some(1_u128.to_be_bytes()));
}

#[test]
fn unrelated_workbench_retirement_does_not_cancel_pending_settlement_or_settled_open() {
    let mut coordinator = session();
    let command = tab_command(&coordinator, 52, 0, "retained");
    let pending = dispatch_tab(&mut coordinator, command, 520);
    coordinator
        .workbench
        .as_mut()
        .unwrap()
        .retire_terminal_membership();

    let completion = immediate_tab_completion(&pending);
    let result = coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap()
        .result;
    assert!(matches!(
        terminal_outcome(&result),
        WorkbenchTerminalOutcome::Started { .. }
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
        let mut coordinator = session();
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
    pending: &crate::session_coordinator::PendingTerminalTabCreate,
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
    let command = tab_command(&session(), 80, 0, "token");

    let mut terminal_owner = session();
    let terminal_pending = dispatch_tab(&mut terminal_owner, command.clone(), 801);
    let before = snapshot(&mut terminal_owner);
    let mut foreign_terminal_owner = session();
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

    let mut workbench_owner = session();
    let mut workbench_pending = dispatch_tab(&mut workbench_owner, command.clone(), 801);
    let mut foreign_workbench_owner = session();
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

    let mut association_owner = session();
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

#[test]
fn first_create_settles_before_later_create_and_retirement_preserves_its_open_outcome() {
    let mut coordinator = session();
    let first_command = tab_command(&coordinator, 990, 0, "first");
    let first = dispatch_tab(&mut coordinator, first_command, 991);
    let later_command = tab_command(&coordinator, 992, 1, "later");
    let later = dispatch_tab(&mut coordinator, later_command, 993);

    let first_completion = immediate_tab_completion(&first);
    coordinator
        .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
        .unwrap();
    coordinator
        .workbench
        .as_mut()
        .unwrap()
        .retire_terminal_membership();
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

    assert_eq!(coordinator.allocate_identity(), Some(3_u128.to_be_bytes()));
}

#[test]
fn per_launch_cwd_overrides_workspace_without_changing_placement_or_directory() {
    let mut coordinator = session();
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
