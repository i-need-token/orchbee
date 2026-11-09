use super::*;
use crate::client_view::PanelDirection;

#[test]
fn workspace_navigation_retains_empty_owners_and_empty_workbench_has_no_identity() {
    let mut source = snapshot(2, 8);
    source.navigation.workspaces.push(Workspace {
        terminal_directory: None,
        automatic_label: false,
        project_id: None,
        primary_directory_id: None,
        label: "Empty Workspace".into(),
        terminal_context_id: context_id(91),
        workspace_id: workspace_id(93),
    });
    source.terminal_contexts.push(terminal_context(91, 0));
    let preferred = preferred_workspace_location(&source).unwrap();
    let first = project_preferred_workspace(&source).unwrap();
    assert_eq!(first.location(), &preferred);
    assert_eq!(
        first.canonical_context(),
        Some(&source.terminal_contexts[0])
    );
    assert_eq!(first.navigation().len(), 2);
    source.terminal_contexts[0].tabs.clear();
    let empty = project_preferred_workspace(&source).unwrap();
    assert!(empty.tabs().is_empty());
    assert_eq!(empty.navigation().len(), 2);
    assert_eq!(empty.location(), &preferred);
    let mut duplicate = source.clone();
    duplicate.navigation.workspaces[1].terminal_context_id = context_id(WORKSPACE_CONTEXT_VALUE);
    assert!(project_preferred_workspace(&duplicate).is_err());
    source.navigation.workspaces.clear();
    source.navigation.workspace_order.clear();
    source.terminal_contexts.clear();
    let empty = project_preferred_workspace(&source).unwrap();
    assert_eq!(empty.location(), &ClientContextLocation::NoWorkspace);
    assert!(empty.navigation().is_empty());
    assert!(empty.canonical_context().is_none());
    assert!(empty.terminal_context_id().is_none());
}

#[test]
fn workspace_navigation_keeps_exact_owner_and_capacity_checks() {
    let mut source = snapshot(0, 1);
    for index in 0..MAXIMUM_TERMINAL_CONTEXTS - 1 {
        let seed = 10_000 + u128::try_from(index).unwrap() * 2;
        source.navigation.workspaces.push(Workspace {
            terminal_directory: None,
            automatic_label: false,
            project_id: None,
            primary_directory_id: None,
            label: format!("Workspace {index}"),
            terminal_context_id: context_id(seed),
            workspace_id: workspace_id(seed + 1),
        });
        source.terminal_contexts.push(terminal_context(seed, 0));
    }
    let first = preferred_workspace_location(&source).unwrap();
    assert_eq!(
        project_native_context_tabs(&source, &first)
            .unwrap()
            .navigation()
            .len(),
        1024
    );
    let mut missing = source.clone();
    missing.terminal_contexts.remove(0);
    assert!(project_native_context_tabs(&missing, &first).is_err());
    let mut duplicate = source.clone();
    duplicate
        .terminal_contexts
        .push(source.terminal_contexts[0].clone());
    assert!(project_native_context_tabs(&duplicate, &first).is_err());
    source.navigation.workspaces.push(Workspace {
        terminal_directory: None,
        automatic_label: false,
        project_id: None,
        primary_directory_id: None,
        label: "Overflow".into(),
        terminal_context_id: context_id(90_000),
        workspace_id: workspace_id(90_001),
    });
    source.terminal_contexts.push(terminal_context(90_000, 0));
    assert!(project_native_context_tabs(&source, &first).is_err());
}

#[test]
fn projects_exact_zero_one_and_256_tab_boundaries_in_order() {
    for tab_count in [0, 1, MAXIMUM_TABS_PER_CONTEXT] {
        let projection = project_preferred_workspace(&snapshot(tab_count, 17)).unwrap();
        assert_eq!(projection.session_id(), &session_id(SESSION_VALUE));
        assert_eq!(projection.revision(), WorkbenchRevision::new(17));
        assert_eq!(
            projection.terminal_context_id().unwrap(),
            &context_id(WORKSPACE_CONTEXT_VALUE)
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
            assert_eq!(
                projected.binding().expect("bound fixture").worker_id(),
                &worker_id(seed + 3)
            );
            assert_eq!(
                projected.binding().expect("bound fixture").terminal_id(),
                &terminal_id(seed + 4)
            );
        }
    }

    assert_eq!(
        project_preferred_workspace(&snapshot(MAXIMUM_TABS_PER_CONTEXT + 1, 17)),
        Err(ClientProjectionError)
    );
}

#[test]
fn requires_one_matching_context_and_ignores_project_workspace_contexts() {
    let mut missing = snapshot(1, 1);
    missing.navigation.workspaces[0].terminal_context_id = context_id(90);
    assert_eq!(
        project_preferred_workspace(&missing),
        Err(ClientProjectionError)
    );

    let mut duplicate = snapshot(1, 1);
    duplicate
        .terminal_contexts
        .push(duplicate.terminal_contexts[0].clone());
    assert_eq!(
        project_preferred_workspace(&duplicate),
        Err(ClientProjectionError)
    );

    let mut coexistence = snapshot(2, 8);
    let unrelated_context_id = context_id(91);
    coexistence.navigation.workspaces.extend(vec![Workspace {
        terminal_directory: None,
        automatic_label: false,
        project_id: Some((project_id(92)).clone()),
        primary_directory_id: None,
        label: "workspace".into(),
        terminal_context_id: unrelated_context_id.clone(),
        workspace_id: workspace_id(93),
    }]);
    coexistence.navigation.projects.push(Project {
        project_id: project_id(92),
        label: "project".into(),
    });
    let mut unrelated = terminal_context(91, 1);
    unrelated.groups.push(TabGroup {
        color: TabGroupColor::Blue,
        group_id: group_id(94),
        label: "ignored group".into(),
    });
    unrelated.tabs[0].pinned = true;
    coexistence.terminal_contexts.insert(0, unrelated);
    let projection = project_preferred_workspace(&coexistence).unwrap();
    assert_eq!(projection.tabs().len(), 2);
    assert_eq!(projection.terminal_context_id().unwrap(), &context_id(2));
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
        launch_failure: None,
        agent_not_resumed: None,
        launch_program: None,
    });
    assert!(project_preferred_workspace(&child).is_ok());

    let mut unbound = snapshot(3, 1);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut unbound.terminal_contexts[0].tabs[1].root
    else {
        unreachable!("fixture tab uses one Panel")
    };
    primary_presentation.binding = None;
    assert!(project_preferred_workspace(&unbound).is_ok());

    for candidate in unsupported {
        assert_eq!(
            project_preferred_workspace(&candidate),
            Err(ClientProjectionError),
            "one unsupported member must reject the complete projection"
        );
    }
}

#[test]
fn projects_split_tree_and_directional_leaf_neighbors() {
    let mut split = snapshot(3, 1);
    split.terminal_contexts[0].tabs[1].root = PanelNode::Split {
        ratio: Default::default(),
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

    let projection = project_preferred_workspace(&split).unwrap();
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
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    ratio: Default::default(),
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
                    ratio: Default::default(),
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

    let projection = project_preferred_workspace(&split).unwrap();
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
    let projection = project_preferred_workspace(&labeled).unwrap();
    for tab in projection.tabs() {
        assert_eq!(tab.tab_label().as_str(), exact_label);
        assert_eq!(tab.presentation_label().as_str(), exact_label);
    }

    let same = id(50);
    let mut equal_text = snapshot(1, 1);
    equal_text.session_id = SessionId::parse(&same).unwrap();
    equal_text.navigation.workspaces[0].terminal_context_id =
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
    let projection = project_preferred_workspace(&equal_text).unwrap();
    let tab = &projection.tabs()[0];
    assert_eq!(projection.session_id().as_str(), same);
    assert_eq!(projection.terminal_context_id().unwrap().as_str(), same);
    assert_eq!(tab.tab_id().as_str(), same);
    assert_eq!(tab.panel_id().as_str(), same);
    assert_eq!(tab.presentation_id().as_str(), same);
    assert_eq!(
        tab.binding().expect("bound fixture").worker_id().as_str(),
        same
    );
    assert_eq!(
        tab.binding().expect("bound fixture").terminal_id().as_str(),
        same
    );
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
            project_preferred_workspace(&candidate),
            Err(ClientProjectionError)
        );
    }
}
