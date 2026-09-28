use super::*;
mod members;
use crate::WorkbenchReadOrNavigationRequest;
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, PresentationBinding, ProtocolVersion, SiteId, TerminalId,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchLabel,
    WorkbenchPanelSplitDirection, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision, WorkbenchTerminalContextId, WorkbenchTerminalGeometry, WorkerId,
};

fn identity(value: u128) -> String {
    format!("{value:032x}")
}

fn binding(value: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&identity(value)).unwrap(),
        worker_id: WorkerId::parse(&identity(value + 1000)).unwrap(),
    }
}

fn split_command(
    authority: &WorkbenchAuthority,
    tab_id: orchbee_workbench_protocol::WorkbenchTabId,
    panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
) -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(900)).unwrap(),
        site_id: authority.site_id().clone(),
        expected_revision: WorkbenchRevision::new(1),
        action: WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&identity(1)).unwrap(),
            tab_id,
            panel_id,
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("second").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    }
}

#[test]
fn panel_split_commits_one_new_leaf_and_retains_the_source_terminal() {
    let site = SiteId::parse(&identity(700)).unwrap();
    let mut authority = WorkbenchAuthority::new(site).unwrap();
    authority.commit_standalone_intent(binding(1)).unwrap();
    let baseline = authority.tree_standalone_membership().unwrap();
    let source = baseline.members().first().unwrap().clone();

    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone());
    let mut request = split_command(
        &authority,
        source.tab_id().clone(),
        source.panel_id().clone(),
    );
    let WorkbenchAction::PanelSplit { right_click, .. } = &mut request.action else {
        panic!("split")
    };
    *right_click = orchbee_workbench_protocol::PanelRightClickTarget::Panel;
    let WorkbenchPanelCreateBegin::Admitted(admission) =
        authority.begin_terminal_panel_create(Some(&grant), request)
    else {
        panic!("expected panel split admission");
    };
    let WorkbenchPanelCreatePreparation::Prepared(prepared) = admission.finish(binding(2)) else {
        panic!("expected prepared panel split");
    };
    let metadata = prepared.metadata().clone();
    assert_eq!(metadata.panel_id(), source.panel_id());
    assert_eq!(
        metadata.direction(),
        Some(WorkbenchPanelSplitDirection::Right)
    );
    let pending = prepared.commit_into_pending();
    let pending_result = authority
        .ledger
        .get(metadata.request_id())
        .map(RetainedRequest::replay_result)
        .cloned()
        .expect("pending request");
    assert!(matches!(
        pending_result.outcome,
        WorkbenchCommandOutcome::PanelSplitPending { .. }
    ));

    let membership = authority.tree_standalone_membership().unwrap();
    assert_eq!(membership.members().len(), 2);
    assert_eq!(
        membership
            .members()
            .iter()
            .filter(|member| member.binding() == &binding(1))
            .count(),
        1
    );
    assert_eq!(
        membership
            .members()
            .iter()
            .filter(|member| member.binding() == &binding(2))
            .count(),
        1
    );

    let snapshot = authority.execute(
        Some(&WorkbenchGrant::read_only(authority.site_id().clone())),
        WorkbenchReadOrNavigationRequest::try_from(WorkbenchRequest::SnapshotRead(
            orchbee_workbench_protocol::SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&identity(901)).unwrap(),
            },
        ))
        .unwrap(),
    );
    let WorkbenchResponse::Snapshot(result) = snapshot else {
        panic!("expected snapshot");
    };
    let orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available { snapshot } =
        result.outcome
    else {
        panic!("expected available snapshot");
    };
    let context = snapshot.terminal_contexts.first().unwrap();
    let tab = context.tabs.first().unwrap();
    let PanelNode::Split {
        children,
        orientation,
        ..
    } = &tab.root
    else {
        panic!("expected split root");
    };
    assert_eq!(*orientation, PanelOrientation::Horizontal);
    assert_eq!(children.len(), 2);
    for (index, slot) in children.iter().enumerate() {
        let PanelNode::Panel { right_click, .. } = &slot.node else {
            panic!("leaf")
        };
        assert_eq!(
            *right_click,
            if index == 0 {
                orchbee_workbench_protocol::PanelRightClickTarget::Orchbee
            } else {
                orchbee_workbench_protocol::PanelRightClickTarget::Panel
            }
        );
    }

    let prepared = authority
        .prepare_terminal_panel_create_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            },
        )
        .unwrap();
    let result = prepared.commit();
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &snapshot);
}
