use super::*;
use crate::test_support::WorkbenchFixture;
use crate::workbench_projection::WorkbenchProjectionStore;
use orchbee_workbench_protocol::{TerminalPresentationId, WorkbenchPanelTerminalTarget};

fn fixture() -> (WorkbenchAuthority, WorkbenchGrant, WorkbenchCommand) {
    let mut authority =
        WorkbenchAuthority::workspace_fixture(SessionId::parse(&identity(700)).unwrap()).unwrap();
    authority.append_fixture_tab(binding(1)).unwrap();
    let snapshot = authority.snapshot();
    let context = &snapshot.terminal_contexts[0];
    let tab = &context.tabs[0];
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!()
    };
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(900)).unwrap(),
        session_id: snapshot.session_id.clone(),
        expected_revision: snapshot.revision,
        action: WorkbenchAction::PanelCreateTerminal {
            terminal_context_id: context.terminal_context_id.clone(),
            tab_id: tab.tab_id.clone(),
            panel_id: panel_id.clone(),
            target: WorkbenchPanelTerminalTarget::Child {
                label: WorkbenchLabel::parse("child").unwrap(),
            },
            geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    let grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone());
    (authority, grant, command)
}

fn prepare<'a>(
    authority: &'a mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> Box<PreparedWorkbenchPanelCreate<'a>> {
    let WorkbenchPanelCreateBegin::Admitted(admission) =
        authority.begin_terminal_panel_create(Some(grant), command)
    else {
        panic!("admission")
    };
    let WorkbenchPanelCreatePreparation::Prepared(prepared) = admission.finish(binding(2)) else {
        panic!("preparation")
    };
    prepared
}

#[test]
fn child_creation_appends_one_identity_with_effect_free_preparation_and_sealed_replay() {
    let (mut authority, grant, command) = fixture();
    let before = authority.snapshot().clone();
    let cursor = authority.last_identity;
    let ledger = authority.ledger.len();
    drop(prepare(&mut authority, &grant, command.clone()));
    assert_eq!(authority.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(authority.ledger.len(), ledger);
    let prepared = prepare(&mut authority, &grant, command.clone());
    let metadata = prepared.metadata().clone();
    assert_eq!(metadata.panel_id(), metadata.terminal_panel_id());
    assert_eq!(metadata.direction(), None);
    let pending = prepared.commit_into_pending();
    assert_eq!(authority.last_identity, cursor + 1);
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = &authority.snapshot().terminal_contexts[0].tabs[0].root
    else {
        panic!("must stay one Panel")
    };
    let PanelNode::Panel {
        primary_presentation: old_primary,
        ..
    } = &before.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    assert_eq!(primary_presentation, old_primary);
    assert_eq!(child_presentations.len(), 1);
    assert_eq!(
        child_presentations[0].presentation_id,
        *metadata.presentation_id()
    );
    assert_eq!(child_presentations[0].binding, Some(binding(2)));
    let WorkbenchPanelCreateBegin::Ready(replay) =
        authority.begin_terminal_panel_create(Some(&grant), command.clone())
    else {
        panic!()
    };
    assert!(
        matches!(replay.outcome, WorkbenchCommandOutcome::PanelTerminalPending { presentation_id, .. } if presentation_id == *metadata.presentation_id())
    );
    let (mut foreign, foreign_grant, foreign_command) = fixture();
    let foreign_token =
        prepare(&mut foreign, &foreign_grant, foreign_command).commit_into_pending();
    assert!(
        authority
            .prepare_terminal_panel_create_settlement(
                foreign_token,
                WorkbenchTerminalOutcome::OutcomeUnknown {}
            )
            .is_err()
    );
    let result = authority
        .prepare_terminal_panel_create_settlement(
            pending,
            WorkbenchTerminalOutcome::OutcomeUnknown {},
        )
        .unwrap()
        .commit();
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::PanelTerminalCreated {
            terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
            ..
        }
    ));
    let WorkbenchPanelCreateBegin::Ready(replay) =
        authority.begin_terminal_panel_create(Some(&grant), command)
    else {
        panic!()
    };
    assert_eq!(replay, result);
    assert_eq!(authority.last_identity, cursor + 1);
}

#[test]
fn binding_existing_primary_or_child_preserves_identity_and_labels_without_allocating() {
    for child in [false, true] {
        let (mut authority, grant, mut command) = fixture();
        let mut snapshot = authority.snapshot().clone();
        let PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } = &mut snapshot.terminal_contexts[0].tabs[0].root
        else {
            panic!()
        };
        let target = if child {
            let mut member = primary_presentation.clone();
            member.presentation_id = TerminalPresentationId::parse(&identity(800)).unwrap();
            child_presentations.push(member);
            &mut child_presentations[0]
        } else {
            primary_presentation
        };
        target.binding = None;
        target.label = "automatic".into();
        target.custom_label =
            Some(orchbee_workbench_protocol::PanelCustomLabel::parse("override").unwrap());
        let target_id = target.presentation_id.clone();
        let mut expected = snapshot.clone();
        authority.projection = WorkbenchProjectionStore::load(
            &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
        )
        .unwrap();
        authority.last_identity = u128::MAX;
        let WorkbenchAction::PanelCreateTerminal { target, .. } = &mut command.action else {
            panic!()
        };
        *target = WorkbenchPanelTerminalTarget::Unbound {
            presentation_id: target_id.clone(),
        };
        let pending = prepare(&mut authority, &grant, command).commit_into_pending();
        let PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } = &mut expected.terminal_contexts[0].tabs[0].root
        else {
            panic!()
        };
        let target = if child {
            &mut child_presentations[0]
        } else {
            primary_presentation
        };
        target.binding = Some(binding(2));
        expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
        assert_eq!(authority.snapshot(), &expected);
        assert_eq!(pending.metadata().presentation_id(), &target_id);
        assert_eq!(authority.last_identity, u128::MAX);
    }
}

#[test]
fn member_creation_rejects_capacity_missing_bound_and_unauthorized_targets_before_effect() {
    for case in [
        "capacity",
        "missing",
        "bound",
        "navigation-only",
        "creation-only",
        "stale",
    ] {
        let (mut authority, mut grant, mut command) = fixture();
        let mut snapshot = authority.snapshot().clone();
        let PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } = &mut snapshot.terminal_contexts[0].tabs[0].root
        else {
            panic!()
        };
        let WorkbenchAction::PanelCreateTerminal { target, .. } = &mut command.action else {
            panic!()
        };
        match case {
            "capacity" => {
                for index in 0..16 {
                    let mut member = primary_presentation.clone();
                    member.presentation_id =
                        TerminalPresentationId::parse(&identity(1000 + index)).unwrap();
                    member.binding = None;
                    child_presentations.push(member);
                }
            }
            "missing" => {
                *target = WorkbenchPanelTerminalTarget::Unbound {
                    presentation_id: TerminalPresentationId::parse(&identity(850)).unwrap(),
                }
            }
            "bound" => {
                *target = WorkbenchPanelTerminalTarget::Unbound {
                    presentation_id: primary_presentation.presentation_id.clone(),
                }
            }
            "navigation-only" => grant.create_terminal = false,
            "creation-only" => grant.mutate_navigation = false,
            "stale" => command.expected_revision = WorkbenchRevision::new(0),
            _ => unreachable!(),
        }
        authority.projection = WorkbenchProjectionStore::load(
            &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
        )
        .unwrap();
        let cursor = authority.last_identity;
        let WorkbenchPanelCreateBegin::Ready(result) =
            authority.begin_terminal_panel_create(Some(&grant), command.clone())
        else {
            panic!("{case}")
        };
        assert!(
            matches!(
                result.outcome,
                WorkbenchCommandOutcome::Denied {} | WorkbenchCommandOutcome::Rejected { .. }
            ),
            "{case}: {:?}",
            result.outcome
        );
        assert_eq!(authority.snapshot(), &snapshot);
        assert_eq!(authority.last_identity, cursor);
        assert!(!authority.retired);
        let WorkbenchPanelCreateBegin::Ready(replay) =
            authority.begin_terminal_panel_create(Some(&grant), command)
        else {
            panic!()
        };
        assert_eq!(replay, result);
    }
}

#[test]
fn publication_only_close_fences_child_creation_and_existing_member_binding() {
    use crate::{
        WorkbenchTabCloseBegin, WorkbenchTabClosePreparation, WorkbenchTabCloseResolution,
    };
    let (mut authority, grant, mut command) = fixture();
    let grant = grant.with_terminal_lifecycle();
    let mut snapshot = authority.snapshot().clone();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut snapshot.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    primary_presentation.binding = None;
    let presentation_id = primary_presentation.presentation_id.clone();
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
    )
    .unwrap();
    let WorkbenchAction::PanelCreateTerminal {
        terminal_context_id,
        tab_id,
        panel_id,
        ..
    } = &command.action
    else {
        panic!()
    };
    let mut close = command.clone();
    close.request_id = WorkbenchRequestId::parse(&identity(950)).unwrap();
    close.action = WorkbenchAction::PanelClose {
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        panel_id: panel_id.clone(),
    };
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), close)
    else {
        panic!()
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!()
    };
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish_resolved_members(vec![], std::time::Instant::now())
    else {
        panic!()
    };
    let _ = prepared.commit();
    let cursor = authority.last_identity;
    for bind in [false, true] {
        if bind {
            let WorkbenchAction::PanelCreateTerminal { target, .. } = &mut command.action else {
                panic!()
            };
            *target = WorkbenchPanelTerminalTarget::Unbound {
                presentation_id: presentation_id.clone(),
            };
        }
        let WorkbenchPanelCreateBegin::Ready(result) =
            authority.begin_terminal_panel_create(Some(&grant), command.clone())
        else {
            panic!()
        };
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            }
        ));
        assert!(!authority.ledger.contains_key(&command.request_id));
        assert_eq!(authority.snapshot(), &snapshot);
        assert_eq!(authority.last_identity, cursor);
    }
}
