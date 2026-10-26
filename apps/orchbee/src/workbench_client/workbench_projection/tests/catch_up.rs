use super::*;
use orchbee_workbench_protocol::{
    Delta, DeltaChange, WorkbenchPublicationDocument, apply_workbench_publication,
};

fn baseline_split_snapshot(revision: u64) -> Snapshot {
    // S40{ left@34, S46{ middle@44, right@45 }@39 }(H): two Splits, three leaves.
    let mut snapshot = snapshot(2, revision);
    snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
                slot_id: slot_id(34),
            },
            PanelSlot {
                node: PanelNode::Split {
                    ratio: Default::default(),
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
    };
    snapshot
}

fn mixed_history_root() -> PanelNode {
    // Folded history over the baseline: close "middle" (S46 collapses into "right" under slot 39),
    // split "right" under a fresh S60, then move the fresh leaf to the root under a fresh S70 graft
    // (S60 collapses back to "right" in slot 39). Retained tuples: left, right, S40, slots 34/39.
    PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: panel(60, 61, 62, 63, "new"),
                slot_id: slot_id(71),
            },
            PanelSlot {
                node: PanelNode::Split {
                    ratio: Default::default(),
                    children: Box::new([
                        PanelSlot {
                            node: panel(30, 31, 32, 33, "left"),
                            slot_id: slot_id(34),
                        },
                        PanelSlot {
                            node: panel(50, 51, 52, 53, "right"),
                            slot_id: slot_id(39),
                        },
                    ]),
                    orientation: PanelOrientation::Horizontal,
                    split_id: split_id(40),
                },
                slot_id: slot_id(72),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(70),
    }
}

fn mixed_history_snapshot(revision: u64) -> Snapshot {
    let mut snapshot = baseline_split_snapshot(revision);
    snapshot.terminal_contexts[0].tabs[0].root = mixed_history_root();
    snapshot
}

#[test]
fn canonical_refresh_accepts_a_mixed_folded_split_close_and_move_history() {
    let current = project_preferred_workspace(&baseline_split_snapshot(10)).unwrap();
    let candidate = project_preferred_workspace(&mixed_history_snapshot(13)).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_rejects_a_folded_shaped_candidate_at_the_same_revision() {
    let current = project_preferred_workspace(&baseline_split_snapshot(10)).unwrap();
    let candidate = project_preferred_workspace(&mixed_history_snapshot(10)).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate),
        Err(ClientProjectionDivergence)
    );
}

#[test]
fn canonical_refresh_rejects_identity_reassignment_inside_a_folded_history() {
    let current = project_preferred_workspace(&baseline_split_snapshot(10)).unwrap();

    // A retained Panel identity whose complete member tuple changed: "right" kept its Panel and
    // Presentation identities but re-bound to another terminal.
    let mut mutated_member = mixed_history_snapshot(13);
    let PanelNode::Split { children, .. } = &mut mutated_member.terminal_contexts[0].tabs[0].root
    else {
        panic!("the mixed fixture must contain a split root");
    };
    let PanelNode::Split {
        children: inner, ..
    } = &mut children[1].node
    else {
        panic!("the mixed fixture must nest S40");
    };
    inner[1].node = panel(50, 51, 52, 99, "right");
    assert_eq!(
        validate_canonical_refresh(
            &current,
            project_preferred_workspace(&mutated_member).unwrap()
        ),
        Err(ClientProjectionDivergence)
    );

    // A removed leaf identity re-introduced with a different member tuple: "middle"'s Panel
    // identity returns bound to different Presentation and terminal identities, disguised as a
    // fresh split leaf.
    let mut resurrected = mixed_history_snapshot(13);
    let PanelNode::Split { children, .. } = &mut resurrected.terminal_contexts[0].tabs[0].root
    else {
        panic!("the mixed fixture must contain a split root");
    };
    children[0].node = panel(35, 99, 100, 101, "middle");
    assert_eq!(
        validate_canonical_refresh(&current, project_preferred_workspace(&resurrected).unwrap()),
        Err(ClientProjectionDivergence)
    );
}

#[test]
fn canonical_refresh_rejects_structure_mutation_inside_a_folded_history() {
    let current = project_preferred_workspace(&baseline_split_snapshot(10)).unwrap();

    // The retained S40 kept its ordered Slot identities but changed orientation.
    let mut flipped = mixed_history_snapshot(13);
    let PanelNode::Split { children, .. } = &mut flipped.terminal_contexts[0].tabs[0].root else {
        panic!("the mixed fixture must contain a split root");
    };
    let PanelNode::Split { orientation, .. } = &mut children[1].node else {
        panic!("the mixed fixture must nest S40");
    };
    *orientation = PanelOrientation::Vertical;
    assert_eq!(
        validate_canonical_refresh(&current, project_preferred_workspace(&flipped).unwrap()),
        Err(ClientProjectionDivergence)
    );

    // The retained Slot 39 was re-parented under the fresh S70 instead of staying under S40.
    let mut reparented = mixed_history_snapshot(13);
    reparented.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: panel(50, 51, 52, 53, "right"),
                slot_id: slot_id(39),
            },
            PanelSlot {
                node: PanelNode::Split {
                    ratio: Default::default(),
                    children: Box::new([
                        PanelSlot {
                            node: panel(30, 31, 32, 33, "left"),
                            slot_id: slot_id(34),
                        },
                        PanelSlot {
                            node: panel(60, 61, 62, 63, "new"),
                            slot_id: slot_id(71),
                        },
                    ]),
                    orientation: PanelOrientation::Horizontal,
                    split_id: split_id(40),
                },
                slot_id: slot_id(72),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: split_id(70),
    };
    assert_eq!(
        validate_canonical_refresh(&current, project_preferred_workspace(&reparented).unwrap()),
        Err(ClientProjectionDivergence)
    );
}

#[test]
fn publication_fold_then_canonical_refresh_accepts_delta_batches_and_snapshot_fallback() {
    // The client folds `publication.read` batches into one final snapshot before validating; a
    // journal gap yields a Snapshot publication that replaces the running projection outright.
    let baseline_snapshot = baseline_split_snapshot(10);
    let mut mid_snapshot = baseline_split_snapshot(11);
    mid_snapshot.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: panel(30, 31, 32, 33, "left"),
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
    let final_snapshot = mixed_history_snapshot(13);
    let delta = |base: u64, revision: u64, terminal_context: &TerminalContext| {
        WorkbenchPublicationDocument::Delta(Delta {
            base_revision: WorkbenchRevision::new(base),
            changes: vec![DeltaChange::TerminalContextUpserted {
                terminal_context: terminal_context.clone(),
            }],
            protocol_version: ProtocolVersion,
            revision: WorkbenchRevision::new(revision),
            session_id: session_id(SESSION_VALUE),
        })
    };

    // A folded Delta batch lands on the same final snapshot.
    let folded = apply_workbench_publication(
        &baseline_snapshot,
        &[
            delta(10, 11, &mid_snapshot.terminal_contexts[0]),
            delta(11, 13, &final_snapshot.terminal_contexts[0]),
        ],
    )
    .unwrap();
    assert_eq!(folded, final_snapshot);
    let current = project_preferred_workspace(&baseline_snapshot).unwrap();
    let candidate = project_preferred_workspace(&folded).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );

    // The journal-gap Snapshot fallback validates through the same baseline->final comparison.
    let folded = apply_workbench_publication(
        &baseline_snapshot,
        &[WorkbenchPublicationDocument::Snapshot(final_snapshot)],
    )
    .unwrap();
    let candidate = project_preferred_workspace(&folded).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn publication_fold_then_canonical_refresh_still_rejects_identity_reassignment() {
    // A folded batch whose final snapshot reassigns a retained member tuple must not pass
    // validation just because it arrived through the canonical fallback shape.
    let baseline_snapshot = baseline_split_snapshot(10);
    let mut tampered = mixed_history_snapshot(13);
    let PanelNode::Split { children, .. } = &mut tampered.terminal_contexts[0].tabs[0].root else {
        panic!("the mixed fixture must contain a split root");
    };
    children[0].node = panel(35, 99, 100, 101, "middle");
    let folded = apply_workbench_publication(
        &baseline_snapshot,
        &[WorkbenchPublicationDocument::Snapshot(tampered)],
    )
    .unwrap();
    let current = project_preferred_workspace(&baseline_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, project_preferred_workspace(&folded).unwrap()),
        Err(ClientProjectionDivergence)
    );
}
