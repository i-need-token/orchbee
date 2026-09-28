use super::*;
use orchbee_client_server_protocol::{OpenOutcome, TerminalOpenRequest};
use orchbee_server::{WorkbenchCheckpoint, WorkbenchRestoreTarget};
use serde_json::{Value, json};

mod presentations;

fn restored(
    with_tab_metadata: bool,
) -> (
    SiteCoordinator,
    Vec<WorkbenchRestoreTarget>,
    WorkerPlacement,
) {
    let mut source = site();
    let PreparedOpen::Dispatch(pending) = source.prepare_open(
        &TerminalOpenRequest::new(id(50), geometry()),
        Some(id(51)),
        Some(worker(1)),
        Some(expiry()),
    ) else {
        panic!("source standalone")
    };
    source
        .complete(pending, Completion::Unavailable, Some(worker(1)))
        .unwrap();
    let old_owner = orchbee_workbench_protocol::WorkerId::parse(&format!(
        "{:032x}",
        u128::from_be_bytes(worker(1).worker_id.as_bytes())
    ))
    .unwrap();
    let bytes = source
        .workbench
        .as_ref()
        .unwrap()
        .checkpoint(&old_owner)
        .unwrap();
    let mut document: Value = serde_json::from_slice(&bytes).unwrap();
    document["lastIdentity"] = json!(format!("{:032x}", 1024));
    let context = &mut document["terminalContexts"][0];
    context["groups"] =
        json!([{ "groupId": format!("{:032x}", 1000), "label": "group", "color": "cyan" }]);
    context["tabs"][0]["pinned"] = json!(true);
    let mut child = context["tabs"][0]["root"]["primaryPresentation"].clone();
    child["presentationId"] = json!(format!("{:032x}", 1001));
    context["tabs"][0]["root"]["childPresentations"] = json!([child]);
    let mut tab = context["tabs"][0].clone();
    tab["tabId"] = json!(format!("{:032x}", 1002));
    tab["groupId"] = json!(format!("{:032x}", 1000));
    tab["pinned"] = json!(false);
    tab["root"]["panelId"] = json!(format!("{:032x}", 1003));
    tab["root"]["primaryPresentation"]["presentationId"] = json!(format!("{:032x}", 1004));
    tab["root"]["childPresentations"] = json!([]);
    context["tabs"].as_array_mut().unwrap().push(tab);
    if !with_tab_metadata {
        context["groups"] = json!([]);
        context["tabs"][0]["pinned"] = json!(false);
        context["tabs"][1]["groupId"] = Value::Null;
    }
    let primary =
        document["terminalContexts"][0]["tabs"][0]["root"]["primaryPresentation"]["presentationId"]
            .clone();
    document["terminalDirectories"] = json!([
        {"presentationId":primary,"path":"C:/primary"},
        {"presentationId":format!("{:032x}",1001),"path":"C:/child"}
    ]);
    document["agentSessions"] = json!([{"presentationId":primary,"session":{"source":"herdr:grok","agent":"grok","reference":{"kind":"id","value":"resume"}}}]);

    let placement = WorkerPlacement {
        worker_id: WorkerId::from_bytes([10; 16]).unwrap(),
        generation: worker(2).generation,
    };
    let owner = orchbee_workbench_protocol::WorkerId::parse(&format!(
        "{:032x}",
        u128::from_be_bytes(placement.worker_id.as_bytes())
    ))
    .unwrap();
    let authority = WorkbenchCheckpoint::parse(&serde_json::to_vec(&document).unwrap())
        .unwrap()
        .reconstruct(SiteId::parse(&format!("{:032x}", 2000)).unwrap(), &owner)
        .unwrap();
    let targets = authority.restoration_targets(&owner).unwrap();
    (SiteCoordinator::new(Some(authority)), targets, placement)
}

#[test]
fn restart_rebinds_children_and_grouped_pinned_tabs_before_open_reuses_first_terminal() {
    let (mut coordinator, targets, worker) = restored(true);
    assert_eq!(targets.len(), 3);
    let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    let pending: Vec<_> = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            coordinator
                .prepare_checkpoint_terminal(
                    target,
                    id(100 + index as u128),
                    worker,
                    expiry(),
                    None,
                    None,
                )
                .unwrap()
        })
        .collect();
    let membership = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .standalone_presentation_membership()
        .unwrap();
    assert_eq!(membership.members().len(), 3);
    assert!(
        coordinator
            .workbench
            .as_ref()
            .unwrap()
            .tree_standalone_membership()
            .is_err(),
        "other structural-operation guards stay independent"
    );
    assert!(matches!(
        coordinator.prepare_open(
            &TerminalOpenRequest::new(id(60), geometry()),
            None,
            Some(worker),
            None
        ),
        PreparedOpen::Wait
    ));
    for (index, pending) in pending.into_iter().enumerate() {
        let authorization = launch_authorization(&pending);
        assert_eq!(authorization.worker_id(), worker.worker_id);
        let identity = HostTerminalIdentity::new(
            HostCreationId::from_bytes([3; 16]).unwrap(),
            HostEpoch::from_bytes([4; 16]).unwrap(),
            HostTerminalId::from_bytes([(index + 1) as u8; 16]).unwrap(),
        );
        let completion = Completion::Launch(
            TerminalLaunchResult::new(
                authorization,
                TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                    identity,
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
        coordinator.seed_checkpoint_agent_sessions().unwrap();
    }
    let opened = coordinator.prepare_open(
        &TerminalOpenRequest::new(id(61), geometry()),
        Some(id(200)),
        Some(worker),
        Some(expiry()),
    );
    assert!(
        matches!(opened, PreparedOpen::Ready(OpenOutcome::Opened { terminal_create_request_id }) if terminal_create_request_id == id(100))
    );
    let after = coordinator.workbench.as_ref().unwrap().snapshot();
    assert_eq!(after.navigation, before.navigation);
    assert_eq!(after.terminal_contexts[0].tabs.len(), 2);
    assert_eq!(
        after.terminal_contexts[0].groups,
        before.terminal_contexts[0].groups
    );
    assert!(after.terminal_contexts[0].tabs[0].pinned);

    // Closing the restored pinned Panel must settle both its primary and child,
    // then retarget Open onto the surviving grouped Tab's exact fresh binding.
    for index in 0..2 {
        let snapshot = coordinator.workbench.as_ref().unwrap().snapshot();
        let context = &snapshot.terminal_contexts[0];
        let tab = &context.tabs[0];
        let orchbee_workbench_protocol::PanelNode::Panel { panel_id, .. } = &tab.root else {
            panic!("restored Panel")
        };
        let close = orchbee_workbench_protocol::WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3000 + index)).unwrap(),
            site_id: snapshot.site_id.clone(),
            expected_revision: snapshot.revision,
            action: orchbee_workbench_protocol::WorkbenchAction::PanelClose {
                terminal_context_id: context.terminal_context_id.clone(),
                tab_id: tab.tab_id.clone(),
                panel_id: panel_id.clone(),
            },
        };
        let result = coordinator
            .execute_terminal_tab_close(
                close.clone(),
                Some(worker),
                true,
                std::time::Instant::now(),
            )
            .unwrap();
        assert!(matches!(
            result.outcome,
            orchbee_workbench_protocol::WorkbenchCommandOutcome::PanelCloseAccepted { .. }
        ));
        assert!(super::multileaf_membership::drive_close_to_publication(
            &mut coordinator,
            &close
        ));
        if index == 0 {
            assert!(
                matches!(coordinator.prepare_open(&TerminalOpenRequest::new(id(62), geometry()), None, Some(worker), None), PreparedOpen::Ready(OpenOutcome::Opened { terminal_create_request_id }) if terminal_create_request_id == id(102))
            );
        }
    }
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));
    let context = &coordinator
        .workbench
        .as_ref()
        .unwrap()
        .snapshot()
        .terminal_contexts[0];
    assert!(context.tabs.is_empty());
    assert_eq!(context.groups, before.terminal_contexts[0].groups);
}

#[test]
fn restart_refusal_does_not_commit_a_second_physical_reservation() {
    let (mut coordinator, targets, worker) = restored(true);
    let pending = coordinator
        .prepare_checkpoint_terminal(&targets[0], id(100), worker, expiry(), None, None)
        .unwrap();
    let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    assert!(
        coordinator
            .prepare_checkpoint_terminal(&targets[0], id(101), worker, expiry(), None, None)
            .is_err()
    );
    assert_eq!(coordinator.workbench.as_ref().unwrap().snapshot(), &before);
    let next = coordinator
        .prepare_checkpoint_terminal(&targets[1], id(102), worker, expiry(), None, None)
        .unwrap();
    assert_eq!(
        u128::from_be_bytes(launch_authorization(&next).terminal_id().as_bytes()),
        2
    );
    coordinator
        .complete(pending, Completion::Unknown, Some(worker))
        .unwrap();
    coordinator
        .complete(next, Completion::Unavailable, Some(worker))
        .unwrap();
}

#[test]
fn unavailable_restart_is_retained_without_replaying_or_faking_open_success() {
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
        coordinator
            .complete(pending, Completion::Unavailable, Some(worker))
            .unwrap();
    }
    let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    for candidate in [200, 201] {
        assert!(matches!(
            coordinator.prepare_open(
                &TerminalOpenRequest::new(id(60), geometry()),
                Some(id(candidate)),
                Some(worker),
                Some(expiry())
            ),
            PreparedOpen::Ready(OpenOutcome::Unavailable { .. })
        ));
    }
    assert_eq!(coordinator.workbench.as_ref().unwrap().snapshot(), &before);
}
