use super::*;
use crate::test_support::WorkbenchFixture;
use crate::workbench_projection::WorkbenchProjectionStore;
use orchbee_workbench_protocol::{
    PanelOrientation, PanelSlot, Presentation, SiteId, TerminalId, TerminalPresentationId,
    WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchRevision, WorkbenchSplitId, WorkerId,
};

fn authority() -> WorkbenchAuthority {
    WorkbenchAuthority::workspace_fixture(SiteId::parse(&identity(91)).unwrap()).unwrap()
}
fn binding() -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&identity(81)).unwrap(),
        worker_id: WorkerId::parse(&identity(71)).unwrap(),
    }
}

#[test]
fn prepared_membership_is_effect_free_until_commit_and_query_survives_retirement() {
    let mut authority = authority();
    let before = authority.projection.snapshot().clone();
    let prepared = authority.prepare_fixture_tab(binding()).unwrap();
    assert_eq!(prepared.metadata().revision(), WorkbenchRevision::new(1));
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, 4);

    authority.append_fixture_tab(binding()).unwrap();
    authority.retire_terminal_membership();
    let membership = authority.fixture_membership().unwrap();
    assert_eq!(membership.members()[0].binding(), &binding());
}

#[test]
fn workspace_query_allows_multiple_canonical_tabs_but_rejects_noncanonical_shape() {
    let mut authority = authority();
    authority.append_fixture_tab(binding()).unwrap();
    let mut candidate = authority.projection.snapshot().clone();
    let context_id = candidate.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = candidate
        .terminal_contexts
        .iter_mut()
        .find(|context| context.terminal_context_id == context_id)
        .unwrap();
    let second_binding = PresentationBinding {
        terminal_id: TerminalId::parse(&identity(82)).unwrap(),
        worker_id: WorkerId::parse(&identity(72)).unwrap(),
    };
    append_simple_tab(
        context,
        &parse_tab_ids(10).unwrap(),
        "second",
        false,
        second_binding,
    );
    candidate.revision = WorkbenchRevision::new(2);
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    let membership = authority.fixture_membership().unwrap();
    assert_eq!(membership.members()[0].binding(), &binding());

    let mut malformed = authority.projection.snapshot().clone();
    malformed.terminal_contexts[0].tabs[0].pinned = true;
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(malformed)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    assert_eq!(
        authority.fixture_membership(),
        Err(WorkbenchTerminalInvariantError)
    );
}

#[test]
fn strict_membership_exposes_every_exact_tuple_in_canonical_order_through_the_projection_bound() {
    for count in [
        0,
        1,
        2,
        orchbee_workbench_protocol::MAXIMUM_TABS_PER_CONTEXT,
    ] {
        let mut authority = authority();
        let mut snapshot = authority.projection.snapshot().clone();
        let context = &mut snapshot.terminal_contexts[0];
        for index in 0..count {
            append_simple_tab(
                context,
                &parse_tab_ids(7 + 3 * index as u128).unwrap(),
                "member",
                false,
                PresentationBinding {
                    terminal_id: TerminalId::parse(&identity(1000 + index as u128)).unwrap(),
                    worker_id: WorkerId::parse(&identity(71)).unwrap(),
                },
            );
        }
        context.tabs.reverse();
        let encoded =
            serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap();
        authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
        let membership = authority.fixture_membership().unwrap();
        assert_eq!(
            membership.terminal_context_id(),
            &snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone()
        );
        assert_eq!(membership.is_empty(), count == 0);
        assert_eq!(membership.members().len(), count);
        for (member, tab) in membership
            .members()
            .iter()
            .zip(&snapshot.terminal_contexts[0].tabs)
        {
            let PanelNode::Panel {
                panel_id,
                primary_presentation,
                ..
            } = &tab.root
            else {
                panic!("simple member expected")
            };
            assert_eq!(member.tab_id(), &tab.tab_id);
            assert_eq!(member.panel_id(), panel_id);
            assert_eq!(
                member.presentation_id(),
                &primary_presentation.presentation_id
            );
            assert_eq!(
                Some(member.binding()),
                primary_presentation.binding.as_ref()
            );
        }
    }
}

#[test]
fn strict_membership_rejects_a_non_head_child_presentation_or_unbound_member() {
    for child in [false, true] {
        let mut authority = authority();
        authority.append_fixture_tab(binding()).unwrap();
        let mut candidate = authority.projection.snapshot().clone();
        append_simple_tab(
            &mut candidate.terminal_contexts[0],
            &parse_tab_ids(10).unwrap(),
            "second",
            false,
            PresentationBinding {
                terminal_id: TerminalId::parse(&identity(82)).unwrap(),
                worker_id: WorkerId::parse(&identity(71)).unwrap(),
            },
        );
        let second = &mut candidate.terminal_contexts[0].tabs[1];
        let PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } = &mut second.root
        else {
            panic!("simple member expected")
        };
        if child {
            child_presentations.push(Presentation {
                custom_label: None,
                presentation_id: TerminalPresentationId::parse(&identity(8)).unwrap(),
                label: "child".to_owned(),
                binding: None,
                launch_failure: None,
            });
        } else {
            primary_presentation.binding = None;
        }
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).unwrap();
        authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
        assert_eq!(
            authority.fixture_membership(),
            Err(WorkbenchTerminalInvariantError)
        );
    }
}

#[test]
fn duplicate_bootstrap_binding_is_rejected_before_strict_membership_is_constructed() {
    let mut authority = authority();
    authority.append_fixture_tab(binding()).unwrap();
    let before = authority.fixture_membership().unwrap();
    let mut duplicate = authority.projection.snapshot().clone();
    append_simple_tab(
        &mut duplicate.terminal_contexts[0],
        &parse_tab_ids(10).unwrap(),
        "duplicate",
        false,
        binding(),
    );
    let document = WorkbenchProtocolDocument::Snapshot(duplicate);
    let encoded = serialize_document(&document).unwrap();
    assert!(WorkbenchProjectionStore::load(&encoded).is_err());
    assert_eq!(authority.fixture_membership().unwrap(), before);
}

#[test]
fn panel_presentation_visitor_is_depth_first_primary_then_children() {
    let node = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: vec![Presentation {
                        custom_label: None,
                        binding: None,
                        label: "left-child".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(104)).unwrap(),
                        launch_failure: None,
                    }],
                    panel_id: WorkbenchPanelId::parse(&identity(102)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(binding()),
                        label: "left-primary".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(103)).unwrap(),
                        launch_failure: None,
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&identity(106)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: None,
                        label: "right-primary".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(107)).unwrap(),
                        launch_failure: None,
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&identity(105)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&identity(100)).unwrap(),
    };
    let mut seen = Vec::new();
    visit_panel_presentations(&node, &mut |panel_id, presentation| {
        seen.push((
            panel_id.as_str().to_owned(),
            presentation.presentation_id.as_str().to_owned(),
            presentation.binding.clone(),
        ));
    });

    assert_eq!(
        seen,
        vec![
            (identity(102), identity(103), Some(binding())),
            (identity(102), identity(104), None),
            (identity(106), identity(107), None),
        ]
    );
}

#[test]
fn tree_membership_collects_split_primary_leaves_for_the_workspace() {
    let mut authority = authority();
    authority.append_fixture_tab(binding()).unwrap();
    let mut snapshot = authority.projection.snapshot().clone();
    let tab = &mut snapshot.terminal_contexts[0].tabs[0];
    tab.root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&identity(102)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(binding()),
                        label: "left".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(103)).unwrap(),
                        launch_failure: None,
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&identity(105)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            terminal_id: TerminalId::parse(&identity(82)).unwrap(),
                            worker_id: WorkerId::parse(&identity(72)).unwrap(),
                        }),
                        label: "right".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(106)).unwrap(),
                        launch_failure: None,
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&identity(104)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&identity(100)).unwrap(),
    };
    snapshot.revision = WorkbenchRevision::new(2);
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();

    let (_, members) = tree_membership_context(
        authority.projection.snapshot(),
        &authority.projection.snapshot().navigation.workspaces[0]
            .terminal_context_id
            .clone(),
    )
    .unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].tab_id(), members[1].tab_id());
    assert_eq!(members[0].presentation_id().as_str(), identity(103));
    assert_eq!(members[1].presentation_id().as_str(), identity(106));
    assert!(
        terminal_membership(
            authority.projection.snapshot(),
            &authority.projection.snapshot().navigation.workspaces[0]
                .terminal_context_id
                .clone()
        )
        .is_ok()
    );
}

#[test]
fn tree_membership_preserves_unbound_layouts_and_enumerates_only_bound_members() {
    // Unbound primary and child Presentations still refuse complete membership.
    fn split_snapshot(authority: &WorkbenchAuthority) -> Snapshot {
        let mut snapshot = authority.projection.snapshot().clone();
        let tab = &mut snapshot.terminal_contexts[0].tabs[0];
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = tab.root.clone()
        else {
            panic!("simple member expected")
        };
        tab.root = PanelNode::Split {
            ratio: Default::default(),
            children: Box::new([
                PanelSlot {
                    node: PanelNode::Panel {
                        right_click: Default::default(),
                        child_presentations: Vec::new(),
                        panel_id,
                        primary_presentation,
                    },
                    slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
                },
                PanelSlot {
                    node: PanelNode::Panel {
                        right_click: Default::default(),
                        child_presentations: Vec::new(),
                        panel_id: WorkbenchPanelId::parse(&identity(105)).unwrap(),
                        primary_presentation: Presentation {
                            custom_label: None,
                            binding: Some(PresentationBinding {
                                terminal_id: TerminalId::parse(&identity(82)).unwrap(),
                                worker_id: WorkerId::parse(&identity(72)).unwrap(),
                            }),
                            label: "right".to_owned(),
                            presentation_id: TerminalPresentationId::parse(&identity(106)).unwrap(),
                            launch_failure: None,
                        },
                    },
                    slot_id: WorkbenchPanelSlotId::parse(&identity(104)).unwrap(),
                },
            ]),
            orientation: PanelOrientation::Horizontal,
            split_id: WorkbenchSplitId::parse(&identity(100)).unwrap(),
        };
        snapshot
    }
    for case in ["unbound leaf", "child presentations"] {
        let mut authority = authority();
        authority.append_fixture_tab(binding()).unwrap();
        let mut snapshot = split_snapshot(&authority);
        let tab = &mut snapshot.terminal_contexts[0].tabs[0];
        match case {
            "pinned tab" => tab.pinned = true,
            _ => {
                let PanelNode::Split { children, .. } = &mut tab.root else {
                    panic!("split root expected")
                };
                let PanelNode::Panel {
                    primary_presentation,
                    child_presentations,
                    ..
                } = &mut children[0].node
                else {
                    panic!("panel leaf expected")
                };
                match case {
                    "unbound leaf" => primary_presentation.binding = None,
                    _ => child_presentations.push(Presentation {
                        custom_label: None,
                        presentation_id: TerminalPresentationId::parse(&identity(130)).unwrap(),
                        label: "child".to_owned(),
                        binding: None,
                        launch_failure: None,
                    }),
                }
            }
        }
        snapshot.revision = WorkbenchRevision::new(2);
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
        authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
        let before = authority.snapshot().clone();
        let membership = authority.fixture_tree_membership().unwrap();
        assert_eq!(
            membership.members().len(),
            if case == "unbound leaf" { 1 } else { 2 }
        );
        assert_eq!(
            authority
                .terminal_tree_membership(membership.terminal_context_id())
                .unwrap(),
            membership
        );
        assert_eq!(authority.snapshot(), &before);
        assert!(!authority.retired);
        assert!(
            authority.fixture_membership().is_err(),
            "complete membership requires every binding"
        );
    }
}

#[test]
fn unbound_primary_never_borrows_a_bound_child_and_bootstrap_preserves_occupied_layout() {
    let mut owner = authority();
    owner.append_fixture_tab(binding()).unwrap();
    let mut snapshot = owner.snapshot().clone();
    let tab_id = snapshot.terminal_contexts[0].tabs[0].tab_id.clone();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        child_presentations,
        ..
    } = &mut snapshot.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    let panel_id = panel_id.clone();
    let mut child = primary_presentation.clone();
    child.presentation_id = TerminalPresentationId::parse(&identity(501)).unwrap();
    primary_presentation.binding = None;
    child_presentations.push(child.clone());
    owner.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
    )
    .unwrap();
    let membership = owner.fixture_presentation_membership().unwrap();
    assert_eq!(membership.members().len(), 1);
    assert_eq!(
        membership.members()[0].presentation_id(),
        &child.presentation_id
    );
    assert_eq!(membership.member_for_panel(&tab_id, &panel_id), None);

    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut snapshot.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    child_presentations[0].binding = None;
    owner.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
    )
    .unwrap();
    assert!(owner.fixture_presentation_membership().unwrap().is_empty());

    assert_eq!(owner.snapshot(), &snapshot);
    assert!(!owner.retired);
}
