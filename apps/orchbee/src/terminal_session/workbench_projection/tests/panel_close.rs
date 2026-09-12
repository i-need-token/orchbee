use super::*;

fn split_snapshot(tab_count: usize, revision: u64) -> Snapshot {
    let mut snapshot = snapshot(tab_count, revision);
    snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
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
    snapshot
}

fn nested_split_root() -> PanelNode {
    PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(35, 36, 37, 38, "middle"),
                            slot_id: slot_id(44),
                        },
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "right"),
                            slot_id: slot_id(45),
                        },
                    ]),
                    orientation: PanelOrientation::Vertical,
                    split_id: split_id(46),
                },
                slot_id: slot_id(39),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    }
}

#[test]
fn canonical_refresh_accepts_close_contraction_to_the_surviving_leaf() {
    let current = project_native_standalone_tabs(&split_snapshot(2, 10)).unwrap();

    for kept_first in [true, false] {
        let mut candidate_snapshot = split_snapshot(2, 11);
        let PanelNode::Split { children, .. } =
            candidate_snapshot.terminal_contexts[0].tabs[0].root.clone()
        else {
            panic!("the baseline fixture must contain a split root");
        };
        let [first, second] = *children;
        candidate_snapshot.terminal_contexts[0].tabs[0].root =
            if kept_first { first } else { second }.node;
        let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
        assert_eq!(
            validate_canonical_refresh(&current, candidate.clone()),
            Ok(candidate)
        );
    }
}

#[test]
fn canonical_refresh_accepts_close_contraction_inside_a_surviving_split() {
    let mut current_snapshot = snapshot(2, 10);
    current_snapshot.terminal_contexts[0].tabs[0].root = nested_split_root();
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    let mut candidate_snapshot = current_snapshot.clone();
    candidate_snapshot.revision = WorkbenchRevision::new(11);
    let PanelNode::Split { children, .. } =
        candidate_snapshot.terminal_contexts[0].tabs[0].root.clone()
    else {
        panic!("the baseline fixture must contain a split root");
    };
    let [first, second] = *children;
    let PanelNode::Split {
        children: inner, ..
    } = second.node
    else {
        panic!("the nested fixture must contain an inner split");
    };
    let [middle, right] = *inner;
    candidate_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            first,
            PanelSlot {
                node: right.node,
                slot_id: second.slot_id,
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    let _ = middle;
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_accepts_close_contraction_that_promotes_a_subtree_to_root() {
    let mut current_snapshot = snapshot(2, 10);
    current_snapshot.terminal_contexts[0].tabs[0].root = nested_split_root();
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    // Closing the left leaf replaces the root Split with the sibling subtree.
    let mut candidate_snapshot = current_snapshot;
    candidate_snapshot.revision = WorkbenchRevision::new(11);
    let PanelNode::Split { children, .. } =
        candidate_snapshot.terminal_contexts[0].tabs[0].root.clone()
    else {
        panic!("the baseline fixture must contain a split root");
    };
    let [_first, second] = *children;
    candidate_snapshot.terminal_contexts[0].tabs[0].root = second.node;
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_rejects_contraction_that_mutates_the_untouched_sibling_subtree() {
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
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "right"),
                            slot_id: slot_id(48),
                        },
                        PanelSlot {
                            node: panel(60, 61, 62, 63, "far-right"),
                            slot_id: slot_id(49),
                        },
                    ]),
                    orientation: PanelOrientation::Vertical,
                    split_id: split_id(47),
                },
                slot_id: slot_id(39),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    // Closing "left" collapses the inner-left Split; the untouched inner-right Split must keep
    // every identity even though the Panel set still matches an exact close.
    let mut candidate_snapshot = current_snapshot;
    candidate_snapshot.revision = WorkbenchRevision::new(11);
    let PanelNode::Split { children, .. } =
        candidate_snapshot.terminal_contexts[0].tabs[0].root.clone()
    else {
        panic!("the baseline fixture must contain a split root");
    };
    let [first, second] = *children;
    let PanelNode::Split {
        children: inner, ..
    } = first.node
    else {
        panic!("the nested fixture must contain an inner split");
    };
    let [_left, middle] = *inner;
    let PanelSlot {
        node:
            PanelNode::Split {
                children: right_children,
                orientation: right_orientation,
                ..
            },
        slot_id: right_slot_id,
    } = second
    else {
        panic!("the untouched fixture side must contain a split");
    };
    candidate_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: middle.node,
                slot_id: first.slot_id,
            },
            PanelSlot {
                node: PanelNode::Split {
                    children: right_children,
                    orientation: right_orientation,
                    split_id: split_id(99),
                },
                slot_id: right_slot_id,
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
fn canonical_refresh_accepts_a_close_history_that_collapsed_two_leaves() {
    // Two committed closes folded into one refresh: each collapsed its parent Split into the
    // sibling subtree, so the candidate keeps only the retained leaf.
    let mut double_close = snapshot(2, 10);
    double_close.terminal_contexts[0].tabs[0].root = nested_split_root();
    let current_nested = project_native_standalone_tabs(&double_close).unwrap();
    let mut collapsed = double_close;
    collapsed.revision = WorkbenchRevision::new(11);
    collapsed.terminal_contexts[0].tabs[0].root = panel(30, 31, 32, 33, "left");
    let candidate = project_native_standalone_tabs(&collapsed).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current_nested, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_rejects_contraction_candidates_that_reassign_retained_identity() {
    let current = project_native_standalone_tabs(&split_snapshot(2, 10)).unwrap();

    // A replacement leaf that was never a member of the baseline: the surviving Tab identity
    // retains no member, so this is reassignment disguised as retention, not a folded close.
    let mut stranger = split_snapshot(2, 11);
    stranger.terminal_contexts[0].tabs[0].root = panel(60, 61, 62, 63, "stranger");
    assert_eq!(
        validate_canonical_refresh(&current, project_native_standalone_tabs(&stranger).unwrap()),
        Err(NativeProjectionDivergence)
    );

    // The surviving leaf mutated: same shape, different binding.
    let mut relabeled = split_snapshot(2, 11);
    relabeled.terminal_contexts[0].tabs[0].root = panel(30, 31, 32, 63, "left");
    assert_eq!(
        validate_canonical_refresh(
            &current,
            project_native_standalone_tabs(&relabeled).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );

    // The Split kept its shape while a leaf vanished from it.
    let mut swapped = split_snapshot(2, 11);
    swapped.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(39),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    assert!(project_native_standalone_tabs(&swapped).is_err());

    // A retained Split whose ordered Slot identities changed: the surviving children were swapped
    // under identities that were never free to move.
    let mut reordered_snapshot = snapshot(2, 10);
    reordered_snapshot.terminal_contexts[0].tabs[0].root = nested_split_root();
    let mut reordered = reordered_snapshot.clone();
    reordered.revision = WorkbenchRevision::new(11);
    let PanelNode::Split { children, .. } = reordered.terminal_contexts[0].tabs[0].root.clone()
    else {
        panic!("the baseline fixture must contain a split root");
    };
    let [first, second] = *children;
    let PanelNode::Split {
        children: inner, ..
    } = second.node
    else {
        panic!("the nested fixture must contain an inner split");
    };
    let [_middle, right] = *inner;
    reordered.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: right.node,
                slot_id: second.slot_id,
            },
            first,
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    assert_eq!(
        validate_canonical_refresh(
            &project_native_standalone_tabs(&reordered_snapshot).unwrap(),
            project_native_standalone_tabs(&reordered).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );
}
