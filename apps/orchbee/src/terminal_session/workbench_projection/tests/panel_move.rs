use super::*;

/// Split(A, B) — a flat horizontal split baseline.
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

/// Split(A, Split(B, C)) — left leaf beside a nested vertical split.
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
fn canonical_refresh_accepts_a_leaf_relocated_beside_its_sibling() {
    let current = project_native_standalone_tabs(&split_snapshot(2, 10)).unwrap();

    // Move A right of B: the root Split is replaced by a fresh Split(B, A).
    let mut candidate_snapshot = split_snapshot(2, 11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(35, 36, 37, 38, "right"),
                slot_id: slot_id(91),
            },
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(92),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(90),
    };
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_accepts_a_leaf_relocated_across_orientations() {
    let current = project_native_standalone_tabs(&split_snapshot(2, 10)).unwrap();

    // Move A below B: fresh vertical Split(B, A).
    let mut candidate_snapshot = split_snapshot(2, 11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(35, 36, 37, 38, "right"),
                slot_id: slot_id(91),
            },
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(92),
            },
        ]),
        orientation: PanelOrientation::Vertical,
        split_id: split_id(90),
    };
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_accepts_relocation_into_a_nested_target() {
    let mut current_snapshot = snapshot(2, 10);
    current_snapshot.terminal_contexts[0].tabs[0].root = nested_split_root();
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    // Move C left of A: the inner Split collapses to B, and a fresh Split(C, A) occupies A's slot.
    let mut candidate_snapshot = current_snapshot;
    candidate_snapshot.revision = WorkbenchRevision::new(11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "right"),
                            slot_id: slot_id(91),
                        },
                        PanelSlot {
                            node: panel(30, 31, 32, 33, "left"),
                            slot_id: slot_id(92),
                        },
                    ]),
                    orientation: PanelOrientation::Horizontal,
                    split_id: split_id(90),
                },
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: panel(35, 36, 37, 38, "middle"),
                slot_id: slot_id(39),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_accepts_relocation_that_promotes_the_inner_split_to_root() {
    let mut current_snapshot = snapshot(2, 10);
    current_snapshot.terminal_contexts[0].tabs[0].root = nested_split_root();
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    // Move A below C: A's parent is the root Split, so the inner Split(B, Split(C, A)) survives
    // as the new root — the former inner Split keeps its identity.
    let mut candidate_snapshot = current_snapshot;
    candidate_snapshot.revision = WorkbenchRevision::new(11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(35, 36, 37, 38, "middle"),
                slot_id: slot_id(44),
            },
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "right"),
                            slot_id: slot_id(91),
                        },
                        PanelSlot {
                            node: panel(30, 31, 32, 33, "left"),
                            slot_id: slot_id(92),
                        },
                    ]),
                    orientation: PanelOrientation::Vertical,
                    split_id: split_id(90),
                },
                slot_id: slot_id(45),
            },
        ]),
        orientation: PanelOrientation::Vertical,
        split_id: split_id(46),
    };
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_rejects_a_move_graft_reusing_known_identities() {
    let current = project_native_standalone_tabs(&split_snapshot(2, 10)).unwrap();

    // Fresh children order but the Split identity is retained: not a legitimate graft.
    let mut reused_split = split_snapshot(2, 11);
    reused_split.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(35, 36, 37, 38, "right"),
                slot_id: slot_id(91),
            },
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(92),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    assert_eq!(
        validate_canonical_refresh(
            &current,
            project_native_standalone_tabs(&reused_split).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );

    // A fresh Split identity that recycles a baseline Slot identity.
    let mut reused_slot = split_snapshot(2, 11);
    reused_slot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(35, 36, 37, 38, "right"),
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(92),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(90),
    };
    assert_eq!(
        validate_canonical_refresh(
            &current,
            project_native_standalone_tabs(&reused_slot).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn canonical_refresh_rejects_relocation_that_mutates_the_moved_or_untouched_subtrees() {
    let mut current_snapshot = snapshot(2, 10);
    current_snapshot.terminal_contexts[0].tabs[0].root = nested_split_root();
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    // The moved leaf arrived with a different binding.
    let mut mutated_leaf = current_snapshot.clone();
    mutated_leaf.revision = WorkbenchRevision::new(11);
    mutated_leaf.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(50, 51, 52, 99, "right"),
                            slot_id: slot_id(91),
                        },
                        PanelSlot {
                            node: panel(30, 31, 32, 33, "left"),
                            slot_id: slot_id(92),
                        },
                    ]),
                    orientation: PanelOrientation::Horizontal,
                    split_id: split_id(90),
                },
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: panel(35, 36, 37, 38, "middle"),
                slot_id: slot_id(39),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    assert_eq!(
        validate_canonical_refresh(
            &current,
            project_native_standalone_tabs(&mutated_leaf).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );

    // The surviving sibling kept its Panel but lost its Slot identity.
    let mut mutated_slot = current_snapshot;
    mutated_slot.revision = WorkbenchRevision::new(11);
    mutated_slot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "right"),
                            slot_id: slot_id(91),
                        },
                        PanelSlot {
                            node: panel(30, 31, 32, 33, "left"),
                            slot_id: slot_id(92),
                        },
                    ]),
                    orientation: PanelOrientation::Horizontal,
                    split_id: split_id(90),
                },
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: panel(35, 36, 37, 38, "middle"),
                slot_id: slot_id(98),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    assert_eq!(
        validate_canonical_refresh(
            &current,
            project_native_standalone_tabs(&mutated_slot).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn canonical_refresh_accepts_a_leaf_swapped_within_its_own_parent() {
    let mut current_snapshot = snapshot(2, 10);
    current_snapshot.terminal_contexts[0].tabs[0].root = nested_split_root();
    let current = project_native_standalone_tabs(&current_snapshot).unwrap();

    // Move C left of B inside the inner Split: the inner Split is replaced by a fresh Split(C, B)
    // while the outer structure stays untouched.
    let mut candidate_snapshot = current_snapshot;
    candidate_snapshot.revision = WorkbenchRevision::new(11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "right"),
                            slot_id: slot_id(93),
                        },
                        PanelSlot {
                            node: panel(35, 36, 37, 38, "middle"),
                            slot_id: slot_id(94),
                        },
                    ]),
                    orientation: PanelOrientation::Vertical,
                    split_id: split_id(95),
                },
                slot_id: slot_id(39),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(40),
    };
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_rejects_restructures_that_are_not_single_leaf_relocation() {
    let current = project_native_standalone_tabs(&split_snapshot(2, 11)).unwrap();

    // A leaf vanished entirely: the Panel set changed, so this is a close, not a move.
    let mut dropped = split_snapshot(2, 12);
    dropped.terminal_contexts[0].tabs[0].root = PanelNode::Split {
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
    assert!(project_native_standalone_tabs(&dropped).is_err());

    // A stranger leaf replaced the moved leaf: the Panel set differs.
    let mut stranger = split_snapshot(2, 12);
    stranger.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(60, 61, 62, 63, "stranger"),
                slot_id: slot_id(91),
            },
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(92),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(90),
    };
    assert_eq!(
        validate_canonical_refresh(&current, project_native_standalone_tabs(&stranger).unwrap()),
        Err(NativeProjectionDivergence)
    );

    // A move composed with an outer reorder is not a single-leaf relocation: pruning either
    // graft hypothesis leaves a tree that differs from the reduced baseline.
    let mut nested_current = snapshot(2, 10);
    nested_current.terminal_contexts[0].tabs[0].root = nested_split_root();
    let nested = project_native_standalone_tabs(&nested_current).unwrap();
    let mut compounded = nested_current;
    compounded.revision = WorkbenchRevision::new(11);
    compounded.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "right"),
                            slot_id: slot_id(93),
                        },
                        PanelSlot {
                            node: panel(35, 36, 37, 38, "middle"),
                            slot_id: slot_id(94),
                        },
                    ]),
                    orientation: PanelOrientation::Vertical,
                    split_id: split_id(95),
                },
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
    assert_eq!(
        validate_canonical_refresh(
            &nested,
            project_native_standalone_tabs(&compounded).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );
}
