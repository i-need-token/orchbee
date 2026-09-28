//! Restoration plus topology publication must retain every physical member.
use super::*;
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchPanelRelocateDestination, WorkbenchPanelSplitDirection,
};

#[test]
fn relocating_restored_children_preserves_all_bindings_and_the_open_reference() {
    let (mut coordinator, targets, worker) = restored(true);
    for (index, target) in targets.iter().enumerate() {
        let pending = coordinator
            .prepare_checkpoint_terminal(
                target,
                id(100 + index as u128),
                worker,
                expiry(),
                None,
                None,
            )
            .unwrap();
        let authorization = launch_authorization(&pending);
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
        site_id: before.site_id.clone(),
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
    assert!(matches!(coordinator.prepare_open(
        &TerminalOpenRequest::new(id(61), geometry()), None, Some(worker), None,
    ), PreparedOpen::Ready(OpenOutcome::Opened { terminal_create_request_id }) if terminal_create_request_id == id(100)));
}
