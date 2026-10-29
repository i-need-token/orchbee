//! Restoration plus topology publication must retain every physical member.
use super::*;
use crate::test_support::WorkbenchFixture;
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchPanelRelocateDestination, WorkbenchPanelSplitDirection,
};

#[test]
fn panel_member_creation_dispatches_literal_launch_and_preserves_original_members() {
    use orchbee_workbench_protocol::{
        LaunchEnvironment, LaunchOptions, LaunchSelection, WorkbenchDirectoryPath, WorkbenchLabel,
        WorkbenchPanelTerminalTarget, WorkbenchTerminalGeometry,
    };
    let (mut coordinator, targets, worker) = restored(true);
    let initial = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    let context = &initial.terminal_contexts[0];
    let tab = &context.tabs[0];
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!()
    };
    for (index, target) in [
        WorkbenchPanelTerminalTarget::Child {
            label: WorkbenchLabel::parse("new child").unwrap(),
        },
        WorkbenchPanelTerminalTarget::Unbound {
            presentation_id: targets[0].presentation_id().clone(),
        },
        WorkbenchPanelTerminalTarget::Unbound {
            presentation_id: targets[1].presentation_id().clone(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 4000 + index)).unwrap(),
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            action: WorkbenchAction::PanelCreateTerminal {
                terminal_context_id: context.terminal_context_id.clone(),
                tab_id: tab.tab_id.clone(),
                panel_id: panel_id.clone(),
                target,
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: Some(
                    LaunchSelection::new(
                        "C:/tools/shell.exe".into(),
                        vec!["literal & $HOME".into()],
                    )
                    .unwrap(),
                ),
                profile: None,
                options: Box::new(LaunchOptions {
                    plugin_pane: None,
                    plugin_context: None,
                    cwd: Some(WorkbenchDirectoryPath::parse("C:/work").unwrap()),
                    environment: LaunchEnvironment::new(vec![(
                        "VALUE".into(),
                        "literal $HOME".into(),
                    )])
                    .unwrap(),
                }),
            },
        };
        let reference = id(5000 + index as u128);
        let PreparedTerminalPanelCreate::Dispatch(pending) = coordinator
            .prepare_terminal_panel_create(
                command.clone(),
                CreateInputs::new(
                    Some(reference.clone()),
                    Some(expiry()),
                    Some(worker),
                    LaunchCapture::absent(),
                ),
            )
        else {
            panic!()
        };
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
            panic!()
        };
        assert_eq!(launch.cwd().unwrap().as_str(), "C:/work");
        assert_eq!(launch.launch().unwrap().executable(), "C:/tools/shell.exe");
        assert_eq!(launch.launch().unwrap().arguments(), &["literal & $HOME"]);
        assert!(
            launch
                .environment()
                .entries()
                .iter()
                .any(|(name, value)| name == "VALUE" && value == "literal $HOME")
        );
        let result = coordinator
            .complete_terminal_panel_create(pending, Completion::Unavailable, Some(worker))
            .unwrap();
        assert!(matches!(
            result.result.outcome,
            WorkbenchCommandOutcome::PanelTerminalCreated {
                terminal_outcome:
                    orchbee_workbench_protocol::WorkbenchTerminalOutcome::Unavailable { .. },
                ..
            }
        ));

        let PreparedTerminalPanelCreate::Ready(replay) = coordinator.prepare_terminal_panel_create(
            command,
            CreateInputs::new(
                Some(id(6000 + index as u128)),
                Some(expiry()),
                Some(worker),
                LaunchCapture::absent(),
            ),
        ) else {
            panic!()
        };
        assert_eq!(replay, result.result);
        assert!(
            coordinator
                .terminals
                .retained_authorization(&id(6000 + index as u128))
                .is_none()
        );
    }
    let after = coordinator.workbench.as_ref().unwrap().snapshot();
    assert_eq!(after.terminal_contexts[0].tabs[1], context.tabs[1]);
    assert_eq!(after.terminal_contexts[0].groups, context.groups);
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = &after.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    assert_eq!(
        primary_presentation.presentation_id,
        *targets[0].presentation_id()
    );
    assert_eq!(child_presentations.len(), 2);
    assert_eq!(
        child_presentations[0].presentation_id,
        *targets[1].presentation_id()
    );
    assert!(primary_presentation.binding.is_some());
    assert!(child_presentations.iter().all(|p| p.binding.is_some()));
}

#[test]
fn unbound_layout_open_is_effect_free_and_explicit_split_establishes_first_reference() {
    let (mut coordinator, _targets, worker) = restored(true);
    let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    assert!(
        coordinator
            .agent_registration_snapshot()
            .unwrap()
            .is_empty()
    );

    assert_eq!(coordinator.workbench.as_ref().unwrap().snapshot(), &before);
    assert!(
        coordinator
            .terminals
            .retained_authorization(&id(101))
            .is_none()
    );
    let context = &before.terminal_contexts[0];
    let tab = &context.tabs[0];
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!()
    };
    let command = super::super::panel_split::split_command(
        &mut coordinator,
        3000,
        context.terminal_context_id.clone(),
        tab.tab_id.clone(),
        panel_id.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let PreparedTerminalPanelCreate::Dispatch(pending) = coordinator.prepare_terminal_panel_create(
        command,
        CreateInputs::new(
            Some(id(102)),
            Some(expiry()),
            Some(worker),
            LaunchCapture::absent(),
        ),
    ) else {
        panic!("unbound layout split must dispatch")
    };

    coordinator
        .complete_terminal_panel_create(pending, Completion::Unavailable, Some(worker))
        .unwrap();

    let after = coordinator.workbench.as_ref().unwrap().snapshot();
    let PanelNode::Split { children, .. } = &after.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    assert_eq!(children[0].node, tab.root);
    assert_eq!(after.terminal_contexts[0].tabs[1], context.tabs[1]);
    assert_eq!(after.terminal_contexts[0].groups, context.groups);
    assert_eq!(
        coordinator
            .workbench
            .as_ref()
            .unwrap()
            .fixture_presentation_membership()
            .unwrap()
            .members()
            .len(),
        1
    );
}

#[test]
fn child_only_restore_registers_and_opens_while_unbound_tab_close_preserves_reference() {
    let (mut coordinator, targets, worker) = restored(true);
    let child = &targets[1];
    let pending = coordinator
        .prepare_checkpoint_terminal(
            child,
            CreateInputs::new(
                Some(id(100)),
                Some(expiry()),
                Some(worker),
                LaunchCapture::absent(),
            ),
            None,
        )
        .unwrap();
    let authorization = launch_authorization(pending.command());
    let completion = Completion::Launch(
        TerminalLaunchResult::new(
            authorization,
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity(),
                HostLaunchState::Started(
                    orchbee_capability_node_protocol::TerminalPixelGeometry::new(80, 24, 8, 16)
                        .unwrap(),
                ),
            )),
        )
        .unwrap(),
    );
    coordinator
        .complete(pending, completion, Some(worker))
        .unwrap();
    let facts = coordinator.agent_registration_snapshot().unwrap();
    assert_eq!(facts.len(), 1);
    let membership = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .fixture_presentation_membership()
        .unwrap();
    assert_eq!(
        membership.members()[0].presentation_id(),
        child.presentation_id()
    );
    assert!(
        membership
            .member_for_panel(child.tab_id(), child.panel_id())
            .is_none()
    );
    let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    let tab = &before.terminal_contexts[0].tabs[1];
    let close = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3000)).unwrap(),
        session_id: before.session_id.clone(),
        expected_revision: before.revision,
        action: WorkbenchAction::TabClose {
            terminal_context_id: before.terminal_contexts[0].terminal_context_id.clone(),
            tab_id: tab.tab_id.clone(),
        },
    };
    let now = std::time::Instant::now();
    let result = coordinator
        .execute_terminal_tab_close(close.clone(), None, false, now)
        .unwrap();
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    coordinator
        .publish_terminal_tab_close(&close.request_id, now)
        .unwrap();
    assert_eq!(
        coordinator
            .workbench
            .as_ref()
            .unwrap()
            .snapshot()
            .terminal_contexts[0]
            .tabs,
        vec![before.terminal_contexts[0].tabs[0].clone()]
    );
    assert_eq!(coordinator.agent_registration_snapshot().unwrap(), facts);
}

#[test]
fn entirely_unbound_tabs_close_without_a_worker() {
    let (mut coordinator, _targets, _worker) = restored(true);
    for index in 0..2 {
        let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
        let context = &before.terminal_contexts[0];
        let tab = &context.tabs[0];
        let PanelNode::Panel { panel_id, .. } = &tab.root else {
            panic!()
        };
        let close = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3000 + index)).unwrap(),
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            action: WorkbenchAction::PanelClose {
                terminal_context_id: context.terminal_context_id.clone(),
                tab_id: tab.tab_id.clone(),
                panel_id: panel_id.clone(),
            },
        };
        let now = std::time::Instant::now();
        let result = coordinator
            .execute_terminal_tab_close(close.clone(), None, false, now)
            .unwrap();
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::PanelCloseAccepted { .. }
        ));
        coordinator
            .publish_terminal_tab_close(&close.request_id, now)
            .unwrap();
    }
    assert!(
        coordinator
            .workbench
            .as_ref()
            .unwrap()
            .snapshot()
            .terminal_contexts[0]
            .tabs
            .is_empty()
    );
}

#[test]
fn relocating_restored_children_preserves_all_bindings_and_the_open_reference() {
    let (mut coordinator, targets, worker) = restored(true);
    for (index, target) in targets.iter().enumerate() {
        let pending = coordinator
            .prepare_checkpoint_terminal(
                target,
                CreateInputs::new(
                    Some(id(100 + index as u128)),
                    Some(expiry()),
                    Some(worker),
                    LaunchCapture::absent(),
                ),
                None,
            )
            .unwrap();
        let authorization = launch_authorization(pending.command());
        let completion = Completion::Launch(
            TerminalLaunchResult::new(
                authorization,
                TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                    HostTerminalIdentity::new(
                        HostCreationId::from_bytes([3; 16]).unwrap(),
                        HostEpoch::from_bytes([4; 16]).unwrap(),
                        HostTerminalId::from_bytes([(index + 1) as u8; 16]).unwrap(),
                    ),
                    HostLaunchState::Started(
                        orchbee_capability_node_protocol::TerminalPixelGeometry::new(80, 24, 8, 16)
                            .unwrap(),
                    ),
                )),
            )
            .unwrap(),
        );
        coordinator
            .complete(pending, completion, Some(worker))
            .unwrap();
    }
    let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    let context = &before.terminal_contexts[0];
    let source = &context.tabs[0];
    let target = &context.tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        child_presentations,
        ..
    } = &source.root
    else {
        panic!("restored Panel")
    };
    assert_eq!(child_presentations.len(), 1);
    let authorizations: Vec<_> = (100..103)
        .map(|n| {
            coordinator
                .terminals
                .retained_authorization(&id(n))
                .unwrap()
        })
        .collect();
    let request = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3000)).unwrap(),
        session_id: before.session_id.clone(),
        expected_revision: before.revision,
        action: WorkbenchAction::PanelRelocate {
            terminal_context_id: context.terminal_context_id.clone(),
            tab_id: source.tab_id.clone(),
            panel_id: panel_id.clone(),
            destination: WorkbenchPanelRelocateDestination::Tab {
                terminal_context_id: None,
                tab_id: target.tab_id.clone(),
                target_panel_id: None,
                direction: WorkbenchPanelSplitDirection::Below,
            },
        },
    };
    let result = coordinator
        .execute_terminal_panel_relocate(request.clone())
        .unwrap();
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::PanelRelocated { .. }
    ));
    let after = coordinator.workbench.as_ref().unwrap().snapshot();
    assert_eq!(after.terminal_contexts[0].tabs.len(), 1);
    assert_eq!(after.terminal_contexts[0].groups, context.groups);
    assert_eq!(after.terminal_contexts[0].tabs[0].group_id, target.group_id);
    assert!(!after.terminal_contexts[0].tabs[0].pinned);
    let PanelNode::Split { children, .. } = &after.terminal_contexts[0].tabs[0].root else {
        panic!("grafted Panel")
    };
    assert_eq!(children[0].node, target.root);
    assert_eq!(children[1].node, source.root);
    for presentation in std::iter::once(primary_presentation).chain(child_presentations) {
        assert!(
            coordinator
                .terminals
                .has_retained_binding(presentation.binding.as_ref().unwrap())
        );
    }
    for (index, authorization) in authorizations.into_iter().enumerate() {
        assert_eq!(
            coordinator
                .terminals
                .retained_authorization(&id(100 + index as u128)),
            Some(authorization)
        );
    }
    assert_eq!(
        coordinator
            .execute_terminal_panel_relocate(request)
            .unwrap(),
        result
    );
}
