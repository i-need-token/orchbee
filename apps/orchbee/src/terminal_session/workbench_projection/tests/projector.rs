use super::*;
use crate::client_view::PanelDirection;

#[test]
fn projects_exact_zero_one_and_256_tab_boundaries_in_order() {
    for tab_count in [0, 1, MAXIMUM_TABS_PER_CONTEXT] {
        let projection = project_native_standalone_tabs(&snapshot(tab_count, 17)).unwrap();
        assert_eq!(projection.site_id(), &site_id(SITE_VALUE));
        assert_eq!(projection.revision(), WorkbenchRevision::new(17));
        assert_eq!(
            projection.terminal_context_id(),
            &context_id(STANDALONE_CONTEXT_VALUE)
        );
        assert_eq!(projection.tabs().len(), tab_count);
        for (index, projected) in projection.tabs().iter().enumerate() {
            let seed = tab_seed(index);
            let expected_label = (index + 1).to_string();
            assert_eq!(projected.tab_id(), &tab_id(seed));
            assert_eq!(projected.tab_label().as_str(), expected_label);
            assert_eq!(projected.panel_id(), &panel_id(seed + 1));
            assert_eq!(projected.presentation_id(), &presentation_id(seed + 2));
            assert_eq!(projected.presentation_label().as_str(), expected_label);
            assert_eq!(projected.binding().worker_id(), &worker_id(seed + 3));
            assert_eq!(projected.binding().terminal_id(), &terminal_id(seed + 4));
        }
    }

    assert_eq!(
        project_native_standalone_tabs(&snapshot(MAXIMUM_TABS_PER_CONTEXT + 1, 17)),
        Err(NativeProjectionError)
    );
}

#[test]
fn requires_one_matching_context_and_ignores_project_workspace_contexts() {
    let mut missing = snapshot(1, 1);
    missing.navigation.standalone_terminal_context_id = context_id(90);
    assert_eq!(
        project_native_standalone_tabs(&missing),
        Err(NativeProjectionError)
    );

    let mut duplicate = snapshot(1, 1);
    duplicate
        .terminal_contexts
        .push(duplicate.terminal_contexts[0].clone());
    assert_eq!(
        project_native_standalone_tabs(&duplicate),
        Err(NativeProjectionError)
    );

    let mut coexistence = snapshot(2, 8);
    let unrelated_context_id = context_id(91);
    coexistence.navigation.projects.push(Project {
        directories: Vec::new(),
        label: "project".into(),
        project_id: project_id(92),
        workspaces: vec![Workspace {
            primary_directory_id: None,
            label: "workspace".into(),
            terminal_context_id: unrelated_context_id.clone(),
            workspace_id: workspace_id(93),
        }],
    });
    let mut unrelated = terminal_context(91, 1);
    unrelated.groups.push(TabGroup {
        color: TabGroupColor::Blue,
        group_id: group_id(94),
        label: "ignored group".into(),
    });
    unrelated.tabs[0].pinned = true;
    coexistence.terminal_contexts.insert(0, unrelated);
    let projection = project_native_standalone_tabs(&coexistence).unwrap();
    assert_eq!(projection.tabs().len(), 2);
    assert_eq!(projection.terminal_context_id(), &context_id(2));
}

#[test]
fn rejects_malformed_metadata_and_unsupported_members_without_filtering() {
    let mut unsupported = Vec::new();

    let mut grouped = snapshot(3, 1);
    grouped.terminal_contexts[0].tabs[1].group_id = Some(group_id(21));
    unsupported.push(grouped);

    let mut pinned = snapshot(3, 1);
    pinned.terminal_contexts[0].tabs[1].pinned = true;
    unsupported.push(pinned);

    let mut child = snapshot(3, 1);
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut child.terminal_contexts[0].tabs[1].root
    else {
        unreachable!("fixture tab uses one Panel")
    };
    child_presentations.push(Presentation {
        custom_label: None,
        binding: None,
        label: "child".into(),
        presentation_id: presentation_id(43),
    });
    unsupported.push(child);

    let mut unbound = snapshot(3, 1);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut unbound.terminal_contexts[0].tabs[1].root
    else {
        unreachable!("fixture tab uses one Panel")
    };
    primary_presentation.binding = None;
    unsupported.push(unbound);

    for candidate in unsupported {
        assert_eq!(
            project_native_standalone_tabs(&candidate),
            Err(NativeProjectionError),
            "one unsupported member must reject the complete projection"
        );
    }
}

#[test]
fn projects_split_tree_and_directional_leaf_neighbors() {
    let mut split = snapshot(3, 1);
    split.terminal_contexts[0].tabs[1].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: panel(35, 36, 37, 38, "right"),
                slot_id: slot_id(39),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };

    let projection = project_native_standalone_tabs(&split).unwrap();
    let tab = &projection.tabs()[1];
    assert_eq!(tab.panel_count(), 2);
    assert_eq!(tab.panel(0).unwrap().panel_id(), &panel_id(30));
    assert_eq!(
        tab.panel(1).unwrap().presentation_id(),
        &presentation_id(36)
    );
    assert_eq!(tab.panel_focus_target(0, PanelDirection::Right), Some(1));
    assert_eq!(tab.panel_focus_target(0, PanelDirection::Left), None);
    assert_eq!(tab.panel_focus_target(1, PanelDirection::Left), Some(0));
    assert_eq!(tab.panel_focus_target(1, PanelDirection::Right), None);
}

#[test]
fn navigates_nested_split_by_geometric_adjacency() {
    let mut split = snapshot(3, 1);
    split.terminal_contexts[0].tabs[1].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(30, 31, 32, 33, "top-left"),
                            slot_id: slot_id(34),
                        },
                        PanelSlot {
                            node: panel(35, 36, 37, 38, "top-right"),
                            slot_id: slot_id(39),
                        },
                    ]),
                    orientation: PanelOrientation::Horizontal,
                    split_id: split_id(40),
                },
                slot_id: slot_id(41),
            },
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(42, 43, 44, 45, "bottom-left"),
                            slot_id: slot_id(46),
                        },
                        PanelSlot {
                            node: panel(47, 48, 49, 50, "bottom-right"),
                            slot_id: slot_id(51),
                        },
                    ]),
                    orientation: PanelOrientation::Horizontal,
                    split_id: split_id(52),
                },
                slot_id: slot_id(53),
            },
        ]),
        orientation: PanelOrientation::Vertical,
        split_id: split_id(54),
    };

    let projection = project_native_standalone_tabs(&split).unwrap();
    let tab = &projection.tabs()[1];
    assert_eq!(tab.panel_count(), 4);
    assert_eq!(tab.panel_focus_target(0, PanelDirection::Right), Some(1));
    assert_eq!(tab.panel_focus_target(1, PanelDirection::Left), Some(0));
    assert_eq!(tab.panel_focus_target(0, PanelDirection::Down), Some(2));
    assert_eq!(tab.panel_focus_target(1, PanelDirection::Down), Some(3));
    assert_eq!(tab.panel_focus_target(2, PanelDirection::Up), Some(0));
    assert_eq!(tab.panel_focus_target(3, PanelDirection::Up), Some(1));
}

#[test]
fn preserves_exact_duplicate_whitespace_labels_and_separate_identity_domains() {
    let exact_label = "  duplicate\tlabel  ";
    let mut labeled = snapshot(2, 1);
    for tab in &mut labeled.terminal_contexts[0].tabs {
        tab.label = exact_label.into();
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut tab.root
        else {
            unreachable!("fixture tab uses one Panel")
        };
        primary_presentation.label = exact_label.into();
    }
    let projection = project_native_standalone_tabs(&labeled).unwrap();
    for tab in projection.tabs() {
        assert_eq!(tab.tab_label().as_str(), exact_label);
        assert_eq!(tab.presentation_label().as_str(), exact_label);
    }

    let same = id(50);
    let mut equal_text = snapshot(1, 1);
    equal_text.site_id = SiteId::parse(&same).unwrap();
    equal_text.navigation.standalone_terminal_context_id =
        WorkbenchTerminalContextId::parse(&same).unwrap();
    equal_text.terminal_contexts[0].terminal_context_id =
        WorkbenchTerminalContextId::parse(&same).unwrap();
    let tab = &mut equal_text.terminal_contexts[0].tabs[0];
    tab.tab_id = WorkbenchTabId::parse(&same).unwrap();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &mut tab.root
    else {
        unreachable!("fixture tab uses one Panel")
    };
    *panel_id = WorkbenchPanelId::parse(&same).unwrap();
    primary_presentation.presentation_id = TerminalPresentationId::parse(&same).unwrap();
    let binding = primary_presentation.binding.as_mut().unwrap();
    binding.worker_id = WorkerId::parse(&same).unwrap();
    binding.terminal_id = TerminalId::parse(&same).unwrap();
    let projection = project_native_standalone_tabs(&equal_text).unwrap();
    let tab = &projection.tabs()[0];
    assert_eq!(projection.site_id().as_str(), same);
    assert_eq!(projection.terminal_context_id().as_str(), same);
    assert_eq!(tab.tab_id().as_str(), same);
    assert_eq!(tab.panel_id().as_str(), same);
    assert_eq!(tab.presentation_id().as_str(), same);
    assert_eq!(tab.binding().worker_id().as_str(), same);
    assert_eq!(tab.binding().terminal_id().as_str(), same);
}

#[test]
fn rejects_labels_that_cannot_be_represented_as_workbench_labels() {
    let too_long = "x".repeat(MAXIMUM_LABEL_LENGTH + 1);
    let mut invalid = Vec::new();

    for label in [String::new(), too_long.clone()] {
        let mut tab_label = snapshot(1, 1);
        tab_label.terminal_contexts[0].tabs[0].label = label;
        invalid.push(tab_label);
    }
    for label in [String::new(), too_long] {
        let mut presentation_label = snapshot(1, 1);
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut presentation_label.terminal_contexts[0].tabs[0].root
        else {
            unreachable!("fixture tab uses one Panel")
        };
        primary_presentation.label = label;
        invalid.push(presentation_label);
    }

    for candidate in invalid {
        assert_eq!(
            project_native_standalone_tabs(&candidate),
            Err(NativeProjectionError)
        );
    }
}
