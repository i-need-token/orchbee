use super::*;
use orchbee_workbench_protocol::{
    PanelOrientation, PanelRightClickTarget, WorkbenchLayoutTemplate,
};

fn action(owner: &WorkbenchAuthority) -> WorkbenchAction {
    WorkbenchAction::TabCreateLayout {
        terminal_context_id: owner.snapshot().navigation.standalone_terminal_context_id.clone(),
        label: WorkbenchLabel::parse("Layout").unwrap(), automatic_label: true,
        layout: serde_json::from_value::<WorkbenchLayoutTemplate>(serde_json::json!({
            "kind":"split", "orientation":"vertical", "children":[
                {"kind":"panel","rightClick":"panel","primary":{"label":"First","customLabel":"Custom"},
                    "children":[{"label":"Child 1"},{"label":"Child 2","customLabel":"Pinned name"}]},
                {"kind":"panel","rightClick":"orchbee","primary":{"label":"Second"},"children":[]}
            ]
        })).unwrap(),
    }
}
fn request(owner: &WorkbenchAuthority, id: u128) -> WorkbenchCommand {
    command(id, owner.snapshot().revision.value(), action(owner))
}

#[test]
fn materialization_is_atomic_unbound_and_exactly_replayable() {
    let (mut owner, grant) = authority();
    let request = request(&owner, 1000);
    let before = owner.snapshot().clone();
    let cursor = owner.last_identity;
    let plan = owner
        .plan(
            &request,
            PureWorkbenchAction::from_action(&request.action).unwrap(),
        )
        .unwrap_or_else(|_| panic!("plan"));
    let prepared = owner
        .prepare_command_commit(request.clone(), plan)
        .unwrap_or_else(|_| panic!("prepare"));
    drop(prepared);
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.last_identity, cursor);
    assert!(owner.ledger.is_empty());
    let result = submit(&mut owner, &grant, request.clone());
    let WorkbenchCommandOutcome::TabLayoutCreated {
        revision,
        tab_id,
        terminal_context_id,
    } = &result.outcome
    else {
        panic!("create")
    };
    assert_eq!(revision.value(), before.revision.value() + 1);
    assert_eq!(owner.last_identity, cursor + 10);
    let context = owner
        .snapshot()
        .terminal_contexts
        .iter()
        .find(|context| &context.terminal_context_id == terminal_context_id)
        .unwrap();
    let tab = &context.tabs[0];
    assert_eq!(&tab.tab_id, tab_id);
    assert!(tab.automatic_label);
    assert!(!tab.pinned);
    assert!(tab.group_id.is_none());
    let PanelNode::Split {
        orientation,
        children,
        ..
    } = &tab.root
    else {
        panic!("split")
    };
    assert_eq!(*orientation, PanelOrientation::Vertical);
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        right_click,
        ..
    } = &children[0].node
    else {
        panic!("panel")
    };
    assert_eq!(*right_click, PanelRightClickTarget::Panel);
    assert!(primary_presentation.binding.is_none());
    assert_eq!(primary_presentation.label, "First");
    assert_eq!(primary_presentation.display_label(), "Custom");
    assert_eq!(
        child_presentations
            .iter()
            .map(|member| member.display_label())
            .collect::<Vec<_>>(),
        ["Child 1", "Pinned name"]
    );
    assert!(
        child_presentations
            .iter()
            .all(|member| member.binding.is_none())
    );
    assert!(owner.tree_standalone_membership().unwrap().is_empty());
    let after = owner.snapshot().clone();
    assert_eq!(submit(&mut owner, &grant, request.clone()), result);
    assert_eq!(owner.snapshot(), &after);
    let mut conflict = request;
    if let WorkbenchAction::TabCreateLayout { label, .. } = &mut conflict.action {
        *label = WorkbenchLabel::parse("Other").unwrap();
    }
    assert!(matches!(
        submit(&mut owner, &grant, conflict).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &after);
}

#[test]
fn materialization_checks_grants_revisions_identity_and_ledger_before_effects() {
    let (mut owner, grant) = authority();
    let request = request(&owner, 1000);
    let before = owner.snapshot().clone();
    let response = owner.execute(
        None,
        WorkbenchRequest::Command(request.clone())
            .try_into()
            .unwrap(),
    );
    assert!(matches!(
        response,
        WorkbenchResponse::Command(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Denied {},
            ..
        })
    ));
    assert!(owner.ledger.is_empty());
    let mut stale = request.clone();
    stale.expected_revision = WorkbenchRevision::new(999);
    let rejected = submit(&mut owner, &grant, stale.clone());
    assert!(matches!(
        rejected.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
    assert_eq!(submit(&mut owner, &grant, stale), rejected);
    let mut fresh = request.clone();
    fresh.request_id = WorkbenchRequestId::parse(&identity(1001)).unwrap();
    owner.last_identity = u128::MAX - 9;
    let rejected = submit(&mut owner, &grant, fresh);
    assert!(matches!(
        rejected.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::IdentityExhausted,
            ..
        }
    ));
    assert_eq!(owner.last_identity, u128::MAX - 9);
    owner.last_identity = 1;
    owner.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    let mut fresh = request;
    fresh.request_id = WorkbenchRequestId::parse(&identity(1002)).unwrap();
    assert!(matches!(
        submit(&mut owner, &grant, fresh).outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::RequestLedgerFull
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.last_identity, 1);
    assert!(!owner.retired);
}

#[test]
fn unbound_workspace_materialization_needs_no_directory_and_respects_context_close() {
    let (mut owner, nav) = authority();
    let project = submit(&mut owner, &nav, command(1000, 0, create("Project")));
    let project_id = project_id(&project);
    let revision = owner.snapshot().revision.value();
    let workspace = submit(
        &mut owner,
        &nav,
        command(
            1001,
            revision,
            WorkbenchAction::WorkspaceCreate {
                project_id: project_id.clone(),
                label: WorkbenchLabel::parse("Workspace").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id,
        terminal_context_id,
        ..
    } = workspace.outcome
    else {
        panic!("workspace")
    };
    let mut create = request(&owner, 1002);
    if let WorkbenchAction::TabCreateLayout {
        terminal_context_id: context,
        ..
    } = &mut create.action
    {
        *context = terminal_context_id.clone();
    }
    assert!(matches!(
        submit(&mut owner, &nav, create).outcome,
        WorkbenchCommandOutcome::TabLayoutCreated { .. }
    ));
    let grant =
        WorkbenchGrant::manage_navigation(owner.site_id().clone()).with_terminal_lifecycle();
    let close = command(
        1003,
        owner.snapshot().revision.value(),
        WorkbenchAction::WorkspaceClose {
            project_id,
            workspace_id,
        },
    );
    let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
        owner.begin_workspace_close(Some(&grant), close)
    else {
        panic!("close")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("primary")
    };
    assert!(primary.members().is_empty());
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish_members(vec![], std::time::Instant::now())
    else {
        panic!("prepared")
    };
    let accepted = prepared.commit();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
    ));
    let mut create = request(&owner, 1004);
    if let WorkbenchAction::TabCreateLayout {
        terminal_context_id: context,
        ..
    } = &mut create.action
    {
        *context = terminal_context_id;
    }
    let before = owner.snapshot().clone();
    let cursor = owner.last_identity;
    assert!(matches!(
        submit(&mut owner, &nav, create).outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.last_identity, cursor);
}

#[test]
fn missing_context_and_total_panel_capacity_reject_without_allocation() {
    let (mut owner, grant) = authority();
    let mut create = request(&owner, 1000);
    if let WorkbenchAction::TabCreateLayout {
        terminal_context_id,
        ..
    } = &mut create.action
    {
        *terminal_context_id = WorkbenchTerminalContextId::parse(&identity(999)).unwrap();
    }
    let before = owner.snapshot().clone();
    let cursor = owner.last_identity;
    assert!(matches!(
        submit(&mut owner, &grant, create).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::TerminalContextUnavailable,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.last_identity, cursor);
    let WorkbenchAction::TabCreateLayout { layout, label, .. } = action(&owner) else {
        panic!()
    };
    let mut context = before.terminal_contexts[0].clone();
    let mut cursor = 1;
    super::super::layout_create::materialize(&mut context, &layout, &label, false, &mut cursor)
        .unwrap_or_else(|_| panic!());
    // Exercise total-node admission independently of the canonical identity validator.
    let mut deep = context.tabs[0].root.clone();
    for _ in 0..11 {
        deep = PanelNode::Split {
            ratio: Default::default(),
            split_id: orchbee_workbench_protocol::WorkbenchSplitId::parse(&identity(9000)).unwrap(),
            orientation: PanelOrientation::Horizontal,
            children: Box::new([
                orchbee_workbench_protocol::PanelSlot {
                    slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&identity(
                        9001,
                    ))
                    .unwrap(),
                    node: deep.clone(),
                },
                orchbee_workbench_protocol::PanelSlot {
                    slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&identity(
                        9002,
                    ))
                    .unwrap(),
                    node: deep,
                },
            ]),
        };
    }
    context.tabs[0].root = deep;
    let old = context.clone();
    let cursor_before = cursor;
    assert!(matches!(
        super::super::layout_create::materialize(&mut context, &layout, &label, false, &mut cursor),
        Err(PlanFailure::Rejected(
            WorkbenchRejectionReason::ResourceLimit
        ))
    ));
    assert_eq!(context, old);
    assert_eq!(cursor, cursor_before);
}
