use super::*;
use crate::workbench_projection::WorkbenchProjectionStore;
use orchbee_workbench_protocol::{
    PanelOrientation, PanelSlot, Presentation, SiteId, TerminalId, TerminalPresentationId,
    WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchRevision, WorkbenchSplitId, WorkerId,
};

fn authority() -> WorkbenchAuthority {
    WorkbenchAuthority::new(SiteId::parse(&identity(91)).unwrap()).unwrap()
}
fn binding() -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&identity(81)).unwrap(),
        worker_id: WorkerId::parse(&identity(71)).unwrap(),
    }
}

#[test]
fn first_membership_commits_exact_shape_and_three_identities_without_a_command_ledger_entry() {
    let mut authority = authority();
    authority.commit_standalone_intent(binding()).unwrap();
    let snapshot = authority.projection.snapshot();
    assert_eq!(snapshot.revision.value(), 1);
    assert_eq!(snapshot.terminal_contexts.len(), 1);
    let context = &snapshot.terminal_contexts[0];
    assert_eq!(
        context.terminal_context_id,
        snapshot.navigation.standalone_terminal_context_id
    );
    assert!(context.groups.is_empty());
    assert_eq!(context.tabs.len(), 1);
    let tab = &context.tabs[0];
    assert_eq!(tab.tab_id.as_str(), identity(2));
    assert_eq!(tab.label, "1");
    assert_eq!(tab.group_id, None);
    assert!(!tab.pinned);
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        child_presentations,
    } = &tab.root
    else {
        panic!("expected one primary Panel");
    };
    assert_eq!(panel_id.as_str(), identity(3));
    assert_eq!(primary_presentation.presentation_id.as_str(), identity(4));
    assert_eq!(primary_presentation.label, "1");
    assert_eq!(primary_presentation.binding, Some(binding()));
    assert!(child_presentations.is_empty());
    assert_eq!(authority.last_identity, 4);
    assert!(authority.ledger.is_empty());
    assert_eq!(authority.ledger_bytes, 0);
    let before = snapshot.clone();
    assert_eq!(
        authority.commit_standalone_intent(binding()),
        Err(StandaloneIntentError::Occupied)
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, 4);
}

#[test]
fn membership_preflight_preserves_projection_and_cursor_at_revision_and_identity_boundaries() {
    for (cursor, revision, reason) in [
        (
            u128::MAX - 2,
            0,
            WorkbenchRejectionReason::IdentityExhausted,
        ),
        (1, u64::MAX, WorkbenchRejectionReason::RevisionExhausted),
    ] {
        let mut authority = authority();
        authority.last_identity = cursor;
        let mut snapshot = authority.projection.snapshot().clone();
        snapshot.revision = WorkbenchRevision::new(revision);
        authority.projection = WorkbenchProjectionStore::load(
            &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
        )
        .unwrap();
        assert_eq!(
            authority.commit_standalone_intent(binding()),
            Err(StandaloneIntentError::Rejected(reason))
        );
        assert_eq!(authority.projection.snapshot(), &snapshot);
        assert_eq!(authority.last_identity, cursor);
        assert!(!authority.retired);
        assert!(authority.ledger.is_empty());
    }
    let mut authority = authority();
    authority.last_identity = u128::MAX - 3;
    authority.commit_standalone_intent(binding()).unwrap();
    assert_eq!(authority.last_identity, u128::MAX);
}

#[test]
fn unavailable_workbench_never_commits_membership_or_consumes_an_identity() {
    let mut authority = authority();
    authority.retired = true;
    let before = authority.projection.snapshot().clone();
    assert_eq!(
        authority.commit_standalone_intent(binding()),
        Err(StandaloneIntentError::Unavailable)
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, 1);
}

#[test]
fn a_full_valid_projection_refuses_membership_before_allocating_or_retiring() {
    use orchbee_workbench_protocol::{
        MAXIMUM_ENCODED_PROJECTION_BYTES, Project, WorkbenchProjectId, WorkbenchTerminalContextId,
        WorkbenchWorkspaceId, Workspace,
    };
    let mut authority = authority();
    let mut snapshot = authority.projection.snapshot().clone();
    for index in 0_u128..256 {
        let context_id = WorkbenchTerminalContextId::parse(&identity(1000 + index)).unwrap();
        snapshot.navigation.projects.push(Project {
            directories: Vec::new(),
            project_id: WorkbenchProjectId::parse(&identity(2000 + index)).unwrap(),
            label: "1".to_owned(),
            workspaces: vec![Workspace {
                primary_directory_id: None,
                workspace_id: WorkbenchWorkspaceId::parse(&identity(3000 + index)).unwrap(),
                terminal_context_id: context_id.clone(),
                label: "1".to_owned(),
            }],
        });
        snapshot
            .terminal_contexts
            .push(super::super::empty_context(context_id));
    }
    let base = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
        .unwrap()
        .len();
    let mut remaining = MAXIMUM_ENCODED_PROJECTION_BYTES - base;
    for project in &mut snapshot.navigation.projects {
        for label in [&mut project.label, &mut project.workspaces[0].label] {
            let increment = remaining.min(2047);
            let bytes = increment + 1;
            *label = "\u{1f41d}".repeat(bytes / 4);
            label.push_str(match bytes % 4 {
                0 => "",
                1 => "a",
                2 => "\u{00e9}",
                _ => "\u{4e2d}",
            });
            remaining -= increment;
        }
    }
    assert_eq!(remaining, 0);
    let encoded =
        serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap();
    assert_eq!(encoded.len(), MAXIMUM_ENCODED_PROJECTION_BYTES);
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    authority.last_identity = 4000;
    assert_eq!(
        authority.commit_standalone_intent(binding()),
        Err(StandaloneIntentError::Rejected(
            WorkbenchRejectionReason::ResourceLimit
        ))
    );
    assert_eq!(authority.projection.snapshot(), &snapshot);
    assert_eq!(authority.last_identity, 4000);
    assert!(!authority.retired);
    assert_eq!(authority.ledger_bytes, 0);
}

#[test]
fn prepared_membership_is_effect_free_until_commit_and_query_survives_retirement() {
    let mut authority = authority();
    let before = authority.projection.snapshot().clone();
    let prepared = authority.prepare_standalone_intent(binding()).unwrap();
    assert_eq!(prepared.metadata().revision(), WorkbenchRevision::new(1));
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, 1);

    authority.commit_standalone_intent(binding()).unwrap();
    authority.retire_standalone_composition();
    let membership = authority.standalone_membership().unwrap();
    assert_eq!(membership.members()[0].binding(), &binding());
}

#[test]
fn standalone_query_allows_multiple_canonical_tabs_but_rejects_noncanonical_shape() {
    let mut authority = authority();
    authority.commit_standalone_intent(binding()).unwrap();
    let mut candidate = authority.projection.snapshot().clone();
    let context_id = candidate.navigation.standalone_terminal_context_id.clone();
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
        &parse_tab_ids(7).unwrap(),
        "second",
        second_binding,
    );
    candidate.revision = WorkbenchRevision::new(2);
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    let membership = authority.standalone_membership().unwrap();
    assert_eq!(membership.members()[0].binding(), &binding());

    let mut malformed = authority.projection.snapshot().clone();
    malformed.terminal_contexts[0].tabs[0].pinned = true;
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(malformed)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    assert_eq!(
        authority.standalone_membership(),
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
                &parse_tab_ids(4 + 3 * index as u128).unwrap(),
                "member",
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
        let membership = authority.standalone_membership().unwrap();
        assert_eq!(
            membership.terminal_context_id(),
            &snapshot.navigation.standalone_terminal_context_id
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
        authority.commit_standalone_intent(binding()).unwrap();
        let mut candidate = authority.projection.snapshot().clone();
        append_simple_tab(
            &mut candidate.terminal_contexts[0],
            &parse_tab_ids(7).unwrap(),
            "second",
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
            });
        } else {
            primary_presentation.binding = None;
        }
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).unwrap();
        authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
        assert_eq!(
            authority.standalone_membership(),
            Err(WorkbenchTerminalInvariantError)
        );
    }
}

#[test]
fn duplicate_bootstrap_binding_is_rejected_before_strict_membership_is_constructed() {
    let mut authority = authority();
    authority.commit_standalone_intent(binding()).unwrap();
    let before = authority.standalone_membership().unwrap();
    let mut duplicate = authority.projection.snapshot().clone();
    append_simple_tab(
        &mut duplicate.terminal_contexts[0],
        &parse_tab_ids(7).unwrap(),
        "duplicate",
        binding(),
    );
    let document = WorkbenchProtocolDocument::Snapshot(duplicate);
    let encoded = serialize_document(&document).unwrap();
    assert!(WorkbenchProjectionStore::load(&encoded).is_err());
    assert_eq!(authority.standalone_membership().unwrap(), before);
}

#[test]
fn panel_presentation_visitor_is_depth_first_primary_then_children() {
    let node = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: vec![Presentation {
                        custom_label: None,
                        binding: None,
                        label: "left-child".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(104)).unwrap(),
                    }],
                    panel_id: WorkbenchPanelId::parse(&identity(102)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(binding()),
                        label: "left-primary".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(103)).unwrap(),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&identity(106)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: None,
                        label: "right-primary".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(107)).unwrap(),
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
fn tree_membership_collects_split_primary_leaves_but_legacy_context_rejects_them() {
    let mut authority = authority();
    authority.commit_standalone_intent(binding()).unwrap();
    let mut snapshot = authority.projection.snapshot().clone();
    let tab = &mut snapshot.terminal_contexts[0].tabs[0];
    tab.root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&identity(102)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(binding()),
                        label: "left".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(103)).unwrap(),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
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
        &authority
            .projection
            .snapshot()
            .navigation
            .standalone_terminal_context_id,
    )
    .unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].tab_id(), members[1].tab_id());
    assert_eq!(members[0].presentation_id().as_str(), identity(103));
    assert_eq!(members[1].presentation_id().as_str(), identity(106));
    assert!(
        terminal_membership(
            authority.projection.snapshot(),
            &authority
                .projection
                .snapshot()
                .navigation
                .standalone_terminal_context_id
        )
        .is_err()
    );
}

#[test]
fn tree_membership_still_rejects_genuine_tree_corruption() {
    // The widened tree view keeps every other tree restriction: an unbound leaf,
    // a leaf carrying child presentations, or a pinned Tab inside a split must
    // still fail the invariant instead of being admitted.
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
            children: Box::new([
                PanelSlot {
                    node: PanelNode::Panel {
                        child_presentations: Vec::new(),
                        panel_id,
                        primary_presentation,
                    },
                    slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
                },
                PanelSlot {
                    node: PanelNode::Panel {
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
    for case in ["unbound leaf", "child presentations", "pinned tab"] {
        let mut authority = authority();
        authority.commit_standalone_intent(binding()).unwrap();
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
                    }),
                }
            }
        }
        snapshot.revision = WorkbenchRevision::new(2);
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
        authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
        assert_eq!(
            authority.tree_standalone_membership(),
            Err(WorkbenchTerminalInvariantError),
            "{case}"
        );
        assert_eq!(
            authority.terminal_tree_membership(
                &authority
                    .projection
                    .snapshot()
                    .navigation
                    .standalone_terminal_context_id
            ),
            Err(WorkbenchTerminalInvariantError),
            "{case}"
        );
    }
}
