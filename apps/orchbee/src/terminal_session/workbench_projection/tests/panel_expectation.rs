use super::*;

fn leaf_a() -> PanelNode {
    panel(1_001, 1_002, 1_003, 1_004, "1")
}

fn leaf_b() -> PanelNode {
    panel(2_011, 2_012, 2_013, 2_014, "peer")
}

fn leaf_new() -> PanelNode {
    panel(3_003, 3_004, 3_005, 3_006, "1")
}

fn graft(
    first: PanelNode,
    first_slot: u128,
    second: PanelNode,
    second_slot: u128,
    orientation: PanelOrientation,
    split: u128,
) -> PanelNode {
    PanelNode::Split {
        split_id: split_id(split),
        orientation,
        children: Box::new([
            PanelSlot {
                node: first,
                slot_id: slot_id(first_slot),
            },
            PanelSlot {
                node: second,
                slot_id: slot_id(second_slot),
            },
        ]),
    }
}

fn baseline_root() -> PanelNode {
    graft(
        leaf_a(),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    )
}

fn snapshot_with_root(tab_count: usize, revision: u64, root: PanelNode) -> Snapshot {
    let mut snapshot = snapshot(tab_count, revision);
    snapshot.terminal_contexts[0].tabs[0].root = root;
    snapshot
}

fn candidate_with_root(tab_count: usize, revision: u64, root: PanelNode) -> NativeContextTabs {
    project_native_standalone_tabs(&snapshot_with_root(tab_count, revision, root)).unwrap()
}

fn baseline(tab_count: usize, revision: u64) -> NativeContextTabs {
    candidate_with_root(tab_count, revision, baseline_root())
}

/// The exact graft a `Right` `panel.split` of the fixture leaf produces.
fn split_graft() -> PanelNode {
    graft(
        leaf_a(),
        3_001,
        leaf_new(),
        3_002,
        PanelOrientation::Horizontal,
        3_000,
    )
}

fn split_candidate() -> PanelNode {
    graft(
        split_graft(),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    )
}

fn split_expectation() -> SplitPanelExpectation {
    SplitPanelExpectation {
        committed_revision: WorkbenchRevision::new(11),
        terminal_context_id: context_id(2),
        tab_id: tab_id(1_000),
        panel_id: panel_id(1_001),
        new_panel_id: panel_id(3_003),
        split_id: split_id(3_000),
        first_slot_id: slot_id(3_001),
        second_slot_id: slot_id(3_002),
        presentation_id: presentation_id(3_004),
        direction: WorkbenchPanelSplitDirection::Right,
        requested_label: label("1"),
    }
}

/// The exact graft a `Right` `panel.move` of the fixture leaf produces: the moved leaf lands
/// after the target leaf under a fresh Split.
fn move_candidate() -> PanelNode {
    graft(
        leaf_b(),
        3_001,
        leaf_a(),
        3_002,
        PanelOrientation::Horizontal,
        3_000,
    )
}

fn move_expectation() -> MovedPanelExpectation {
    MovedPanelExpectation::new(
        WorkbenchRevision::new(11),
        context_id(2),
        tab_id(1_000),
        panel_id(1_001),
        panel_id(2_011),
        WorkbenchPanelSplitDirection::Right,
    )
}

fn rejected_split(root: PanelNode) {
    let baseline = baseline(1, 10);
    let candidate = candidate_with_root(1, 11, root);
    assert_eq!(
        validate_split_panel(&baseline, &split_expectation(), candidate),
        Err(NativeProjectionDivergence)
    );
}

fn rejected_move(root: PanelNode) {
    let baseline = baseline(1, 10);
    let candidate = candidate_with_root(1, 11, root);
    assert_eq!(
        validate_moved_panel(&baseline, &move_expectation(), candidate),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn split_expectation_accepts_the_exact_committed_graft() {
    let baseline = baseline(1, 10);
    let candidate = candidate_with_root(1, 11, split_candidate());
    assert!(validate_split_panel(&baseline, &split_expectation(), candidate).is_ok());
}

#[test]
fn split_expectation_rejects_a_graft_under_the_wrong_leaf() {
    // The committed Split hangs under the peer instead of the split leaf; the
    // ungrafted shape alone cannot distinguish this from the real graft.
    rejected_split(graft(
        leaf_a(),
        2_001,
        graft(
            leaf_b(),
            3_001,
            leaf_new(),
            3_002,
            PanelOrientation::Horizontal,
            3_000,
        ),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
}

#[test]
fn split_expectation_rejects_direction_and_orientation_mismatches() {
    // Added leaf first: the source child is not the split leaf for a Right split.
    rejected_split(graft(
        graft(
            leaf_new(),
            3_001,
            leaf_a(),
            3_002,
            PanelOrientation::Horizontal,
            3_000,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // Vertical graft for a horizontal direction.
    rejected_split(graft(
        graft(
            leaf_a(),
            3_001,
            leaf_new(),
            3_002,
            PanelOrientation::Vertical,
            3_000,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // Expectation direction Below but the graft is horizontal.
    let baseline = baseline(1, 10);
    let mut expected = split_expectation();
    expected.direction = WorkbenchPanelSplitDirection::Below;
    let candidate = candidate_with_root(1, 11, split_candidate());
    assert_eq!(
        validate_split_panel(&baseline, &expected, candidate),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn split_expectation_rejects_wrong_slot_and_split_identities() {
    // Wrong slot identities.
    rejected_split(graft(
        graft(
            leaf_a(),
            3_011,
            leaf_new(),
            3_012,
            PanelOrientation::Horizontal,
            3_000,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // No Split carries the committed split identity.
    rejected_split(graft(
        graft(
            leaf_a(),
            3_001,
            leaf_new(),
            3_002,
            PanelOrientation::Horizontal,
            3_999,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // The committed split identity already exists in the baseline.
    let baseline = baseline(1, 10);
    let mut expected = split_expectation();
    expected.split_id = split_id(2_000);
    let candidate = candidate_with_root(1, 11, split_candidate());
    assert_eq!(
        validate_split_panel(&baseline, &expected, candidate),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn split_expectation_rejects_a_divergent_added_leaf() {
    for divergent in [
        // Wrong new panel identity.
        panel(3_999, 3_004, 3_005, 3_006, "1"),
        // Wrong presentation identity.
        panel(3_003, 3_999, 3_005, 3_006, "1"),
        // Wrong presentation label.
        panel(3_003, 3_004, 3_005, 3_006, "other"),
    ] {
        rejected_split(graft(
            graft(
                leaf_a(),
                3_001,
                divergent,
                3_002,
                PanelOrientation::Horizontal,
                3_000,
            ),
            2_001,
            leaf_b(),
            2_002,
            PanelOrientation::Horizontal,
            2_000,
        ));
    }
}

#[test]
fn split_expectation_rejects_a_divergent_source_member() {
    // The source leaf keeps its Panel identity but carries a mutated member tuple.
    rejected_split(graft(
        graft(
            panel(1_001, 1_099, 1_003, 1_004, "1"),
            3_001,
            leaf_new(),
            3_002,
            PanelOrientation::Horizontal,
            3_000,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
}

#[test]
fn split_expectation_rejects_non_increment_and_stale_revisions() {
    let baseline = baseline(1, 10);
    let mut expected = split_expectation();
    // The committed revision must be exactly baseline+1.
    expected.committed_revision = WorkbenchRevision::new(12);
    let candidate = candidate_with_root(1, 12, split_candidate());
    assert_eq!(
        validate_split_panel(&baseline, &expected, candidate),
        Err(NativeProjectionDivergence)
    );
    // A candidate older than the commit is rejected.
    let expected = split_expectation();
    let stale = candidate_with_root(1, 10, split_candidate());
    assert_eq!(
        validate_split_panel(&baseline, &expected, stale),
        Err(NativeProjectionDivergence)
    );
    // A candidate that never applied the graft is rejected at the commit revision.
    let unchanged = candidate_with_root(1, 11, baseline_root());
    assert_eq!(
        validate_split_panel(&baseline, &expected, unchanged),
        Err(NativeProjectionDivergence)
    );
    // A superseding candidate is accepted without the exact graft.
    let superseding = candidate_with_root(1, 12, baseline_root());
    assert!(validate_split_panel(&baseline, &expected, superseding).is_ok());
}

#[test]
fn split_expectation_rejects_a_tab_at_a_different_index_and_divergent_siblings() {
    let baseline = baseline(2, 10);
    let expected = split_expectation();
    // The committed Tab landed at a different index.
    let mut swapped = snapshot_with_root(2, 11, split_candidate());
    swapped.terminal_contexts[0].tabs.swap(0, 1);
    let swapped = project_native_standalone_tabs(&swapped).unwrap();
    assert_eq!(
        validate_split_panel(&baseline, &expected, swapped),
        Err(NativeProjectionDivergence)
    );
    // A sibling Tab diverged alongside the graft.
    let mut divergent = snapshot_with_root(2, 11, split_candidate());
    divergent.terminal_contexts[0].tabs[1].root = panel(1_011, 1_099, 1_013, 1_014, "2");
    let divergent = project_native_standalone_tabs(&divergent).unwrap();
    assert_eq!(
        validate_split_panel(&baseline, &expected, divergent),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn move_expectation_accepts_the_exact_committed_graft() {
    let baseline = baseline(1, 10);
    let candidate = candidate_with_root(1, 11, move_candidate());
    assert!(validate_moved_panel(&baseline, &move_expectation(), candidate).is_ok());
}

#[test]
fn move_expectation_rejects_direction_and_order_mismatches() {
    // Moved leaf first for a Right move.
    rejected_move(graft(
        leaf_a(),
        3_001,
        leaf_b(),
        3_002,
        PanelOrientation::Horizontal,
        3_000,
    ));
    // Vertical graft for a horizontal direction.
    rejected_move(graft(
        leaf_b(),
        3_001,
        leaf_a(),
        3_002,
        PanelOrientation::Vertical,
        3_000,
    ));
}

#[test]
fn move_expectation_rejects_reused_split_identities() {
    // The graft must be a fresh Split, not a baseline identity.
    rejected_move(graft(
        leaf_b(),
        3_001,
        leaf_a(),
        3_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
}

#[test]
fn move_expectation_rejects_wrong_or_residual_leaves() {
    // The moved child is a foreign leaf rather than the moved Panel.
    rejected_move(graft(
        leaf_b(),
        3_001,
        leaf_new(),
        3_002,
        PanelOrientation::Horizontal,
        3_000,
    ));
    // The graft sits under a foreign parent instead of replacing the moved leaf.
    rejected_move(graft(
        leaf_new(),
        2_001,
        graft(
            leaf_b(),
            3_001,
            leaf_a(),
            3_002,
            PanelOrientation::Horizontal,
            3_000,
        ),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // No graft exists at the commit revision.
    rejected_move(baseline_root());
}

#[test]
fn move_expectation_rejects_divergent_member_tuples() {
    // The moved leaf carries a mutated member tuple.
    rejected_move(graft(
        leaf_b(),
        3_001,
        panel(1_001, 1_099, 1_003, 1_004, "1"),
        3_002,
        PanelOrientation::Horizontal,
        3_000,
    ));
    // The target leaf carries a mutated member tuple.
    rejected_move(graft(
        panel(2_011, 2_099, 2_013, 2_014, "peer"),
        3_001,
        leaf_a(),
        3_002,
        PanelOrientation::Horizontal,
        3_000,
    ));
}

#[test]
fn move_expectation_rejects_non_increment_and_stale_revisions() {
    let baseline = baseline(1, 10);
    let mut expected = move_expectation();
    expected.committed_revision = WorkbenchRevision::new(12);
    let candidate = candidate_with_root(1, 12, move_candidate());
    assert_eq!(
        validate_moved_panel(&baseline, &expected, candidate),
        Err(NativeProjectionDivergence)
    );
    let expected = move_expectation();
    let stale = candidate_with_root(1, 10, move_candidate());
    assert_eq!(
        validate_moved_panel(&baseline, &expected, stale),
        Err(NativeProjectionDivergence)
    );
    let superseding = candidate_with_root(1, 12, baseline_root());
    assert!(validate_moved_panel(&baseline, &expected, superseding).is_ok());
}

#[test]
fn move_expectation_rejects_a_tab_at_a_different_index_and_divergent_siblings() {
    let baseline = baseline(2, 10);
    let expected = move_expectation();
    let mut swapped = snapshot_with_root(2, 11, move_candidate());
    swapped.terminal_contexts[0].tabs.swap(0, 1);
    let swapped = project_native_standalone_tabs(&swapped).unwrap();
    assert_eq!(
        validate_moved_panel(&baseline, &expected, swapped),
        Err(NativeProjectionDivergence)
    );
    let mut divergent = snapshot_with_root(2, 11, move_candidate());
    divergent.terminal_contexts[0].tabs[1].root = panel(1_011, 1_099, 1_013, 1_014, "2");
    let divergent = project_native_standalone_tabs(&divergent).unwrap();
    assert_eq!(
        validate_moved_panel(&baseline, &expected, divergent),
        Err(NativeProjectionDivergence)
    );
}
