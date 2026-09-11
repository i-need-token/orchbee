use super::*;

#[test]
fn canonical_refresh_accepts_identical_revision_advances_and_ordered_extensions() {
    let empty = projected(0, 1);
    let one = projected(1, 2);
    assert_eq!(validate_canonical_refresh(&empty, one.clone()), Ok(one));

    let current = projected(2, 10);
    assert_eq!(
        validate_canonical_refresh(&current, current.clone()),
        Ok(current.clone())
    );

    let mut same_revision_rename = current.clone();
    same_revision_rename.tabs[0].tab_label = label("same revision change");
    assert_eq!(
        validate_canonical_refresh(&current, same_revision_rename),
        Err(NativeProjectionDivergence)
    );

    let mut revision_only = current.clone();
    revision_only.revision = WorkbenchRevision::new(11);
    assert_eq!(
        validate_canonical_refresh(&current, revision_only.clone()),
        Ok(revision_only)
    );

    let extension = projected(5, 12);
    assert_eq!(
        validate_canonical_refresh(&current, extension.clone()),
        Ok(extension)
    );

    let mut renamed_and_extended = projected(4, 13);
    renamed_and_extended.tabs[0].tab_label = label("renamed first");
    renamed_and_extended.tabs[1].tab_label = label("renamed second");
    assert_eq!(
        validate_canonical_refresh(&current, renamed_and_extended.clone()),
        Ok(renamed_and_extended)
    );
}

#[test]
fn canonical_refresh_accepts_one_exact_panel_split_extension() {
    let mut current_snapshot = snapshot(3, 10);
    current_snapshot.terminal_contexts[0].tabs[1].root = PanelNode::Split {
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
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    let mut candidate_snapshot = current_snapshot;
    candidate_snapshot.revision = WorkbenchRevision::new(11);
    let old_root = candidate_snapshot.terminal_contexts[0].tabs[1].root.clone();
    let PanelNode::Split {
        children,
        orientation,
        split_id: old_split_id,
    } = old_root
    else {
        panic!("the baseline fixture must contain a split root");
    };
    let [old_first, old_second] = *children;
    candidate_snapshot.terminal_contexts[0].tabs[1].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "new"),
                            slot_id: slot_id(54),
                        },
                        PanelSlot {
                            node: old_first.node,
                            slot_id: slot_id(55),
                        },
                    ]),
                    orientation: PanelOrientation::Vertical,
                    split_id: split_id(56),
                },
                slot_id: old_first.slot_id,
            },
            old_second,
        ]),
        orientation,
        split_id: old_split_id,
    };
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_rejects_split_extension_that_mutates_the_untouched_sibling_subtree() {
    let mut current_snapshot = snapshot(2, 10);
    current_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(30, 31, 32, 33, "left"),
                            slot_id: slot_id(44),
                        },
                        PanelSlot {
                            node: panel(35, 36, 37, 38, "middle"),
                            slot_id: slot_id(45),
                        },
                    ]),
                    orientation: PanelOrientation::Vertical,
                    split_id: split_id(46),
                },
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: panel(50, 51, 52, 53, "right"),
                slot_id: slot_id(39),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    // The extension lands inside the second slot while the untouched inner-left Split has its
    // identity rewritten; the Panel set still matches an exact split.
    let mut candidate_snapshot = current_snapshot;
    candidate_snapshot.revision = WorkbenchRevision::new(11);
    let PanelNode::Split { children, .. } =
        candidate_snapshot.terminal_contexts[0].tabs[0].root.clone()
    else {
        panic!("the baseline fixture must contain a split root");
    };
    let [first, second] = *children;
    let PanelSlot {
        node:
            PanelNode::Split {
                children: left_children,
                orientation: left_orientation,
                ..
            },
        slot_id: left_slot_id,
    } = first
    else {
        panic!("the untouched fixture side must contain a split");
    };
    candidate_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: left_children,
                    orientation: left_orientation,
                    split_id: split_id(99),
                },
                slot_id: left_slot_id,
            },
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: second.node,
                            slot_id: slot_id(54),
                        },
                        PanelSlot {
                            node: panel(60, 61, 62, 63, "new"),
                            slot_id: slot_id(55),
                        },
                    ]),
                    orientation: PanelOrientation::Vertical,
                    split_id: split_id(56),
                },
                slot_id: second.slot_id,
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    assert_eq!(
        validate_canonical_refresh(
            &current,
            project_native_standalone_tabs(&candidate_snapshot).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn canonical_refresh_checks_every_cache_length_through_the_protocol_bound() {
    for current_tab_count in 0..=MAXIMUM_TABS_PER_CONTEXT {
        let current = projected(current_tab_count, 1);
        let revision_only = projected(current_tab_count, 2);
        assert_eq!(
            validate_canonical_refresh(&current, revision_only.clone()),
            Ok(revision_only)
        );

        if current_tab_count < MAXIMUM_TABS_PER_CONTEXT {
            let extension = projected(current_tab_count + 1, 2);
            assert_eq!(
                validate_canonical_refresh(&current, extension.clone()),
                Ok(extension)
            );
        }
        if current_tab_count > 0 {
            assert_eq!(
                validate_canonical_refresh(&current, projected(current_tab_count - 1, 2)),
                Ok(projected(current_tab_count - 1, 2))
            );
        }
    }
}

#[test]
fn canonical_refresh_rejects_outer_revision_and_membership_divergence() {
    let current = projected(2, 10);

    let mut candidates = Vec::new();
    let mut site_mismatch = projected(3, 11);
    site_mismatch.site_id = site_id(80);
    candidates.push(site_mismatch);

    let mut context_mismatch = projected(3, 11);
    context_mismatch.location = NativeContextLocation::Standalone(context_id(81));
    candidates.push(context_mismatch);

    candidates.push(projected(3, 9));
    candidates.push(projected(3, 10));

    let mut same_revision_reorder = current.clone();
    same_revision_reorder.tabs.swap(0, 1);
    candidates.push(same_revision_reorder);

    for candidate in candidates {
        assert_eq!(
            validate_canonical_refresh(&current, candidate),
            Err(NativeProjectionDivergence)
        );
    }
}

#[test]
fn canonical_refresh_rejects_every_non_tab_label_change_to_existing_members() {
    type Mutation = fn(&mut NativeSimpleTab);
    let mutations: [(&str, Mutation); 6] = [
        ("Tab ID", |tab| tab.tab_id = tab_id(200)),
        ("Panel ID", |tab| tab.panel_id = panel_id(201)),
        ("Presentation ID", |tab| {
            tab.presentation_id = presentation_id(202);
        }),
        ("Presentation label", |tab| {
            tab.presentation_label = label("changed presentation");
        }),
        ("Worker ID", |tab| tab.binding.worker_id = worker_id(203)),
        ("Terminal ID", |tab| {
            tab.binding.terminal_id = terminal_id(204);
        }),
    ];
    let current = projected(2, 10);
    for (field, mutate) in mutations {
        let mut candidate = projected(3, 11);
        mutate(&mut candidate.tabs[0]);
        assert_eq!(
            validate_canonical_refresh(&current, candidate),
            Err(NativeProjectionDivergence),
            "changed {field} must reject the complete refresh"
        );
    }
}
