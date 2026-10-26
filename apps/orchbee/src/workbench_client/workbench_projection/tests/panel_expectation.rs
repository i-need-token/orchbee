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
        ratio: Default::default(),
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
    project_preferred_workspace(&snapshot_with_root(tab_count, revision, root)).unwrap()
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
    let swapped = project_preferred_workspace(&swapped).unwrap();
    assert_eq!(
        validate_split_panel(&baseline, &expected, swapped),
        Err(NativeProjectionDivergence)
    );
    // A sibling Tab diverged alongside the graft.
    let mut divergent = snapshot_with_root(2, 11, split_candidate());
    divergent.terminal_contexts[0].tabs[1].root = panel(1_011, 1_099, 1_013, 1_014, "2");
    let divergent = project_preferred_workspace(&divergent).unwrap();
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
    let swapped = project_preferred_workspace(&swapped).unwrap();
    assert_eq!(
        validate_moved_panel(&baseline, &expected, swapped),
        Err(NativeProjectionDivergence)
    );
    let mut divergent = snapshot_with_root(2, 11, move_candidate());
    divergent.terminal_contexts[0].tabs[1].root = panel(1_011, 1_099, 1_013, 1_014, "2");
    let divergent = project_preferred_workspace(&divergent).unwrap();
    assert_eq!(
        validate_moved_panel(&baseline, &expected, divergent),
        Err(NativeProjectionDivergence)
    );
}

// ---- nested (depth ≥ 2) grafts ----------------------------------------------
//
// The flat fixtures only exercise leaves directly under the root Split. These
// cases put the source and target leaves at depth two, where grafts must
// preserve every intermediate Split/Slot identity and removals collapse whole
// ancestor chains.

fn leaf_c() -> PanelNode {
    panel(4_011, 4_012, 4_013, 4_014, "aux")
}

/// root Split [slot 2_001 → inner vertical Split 2_100 [2_101 → leaf_a |
/// 2_102 → leaf_c] | slot 2_002 → leaf_b]: leaf_a sits at depth two.
fn nested_root() -> PanelNode {
    graft(
        graft(
            leaf_a(),
            2_101,
            leaf_c(),
            2_102,
            PanelOrientation::Vertical,
            2_100,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    )
}

fn nested_baseline(tab_count: usize, revision: u64) -> NativeContextTabs {
    candidate_with_root(tab_count, revision, nested_root())
}

/// The committed Split hangs inside the inner Split's first slot.
fn nested_split_candidate() -> PanelNode {
    graft(
        graft(
            split_graft(),
            2_101,
            leaf_c(),
            2_102,
            PanelOrientation::Vertical,
            2_100,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    )
}

/// Moving leaf_a right of leaf_b removes it from the inner Split, which
/// collapses so leaf_c inherits the root's first slot.
fn nested_move_out_candidate() -> PanelNode {
    graft(
        leaf_c(),
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
    )
}

/// Moving leaf_b below leaf_c drops the root Split entirely: the inner Split
/// becomes the root and carries the graft in its second slot.
fn nested_move_in_candidate() -> PanelNode {
    graft(
        leaf_a(),
        2_101,
        graft(
            leaf_c(),
            3_101,
            leaf_b(),
            3_102,
            PanelOrientation::Vertical,
            3_100,
        ),
        2_102,
        PanelOrientation::Vertical,
        2_100,
    )
}

fn nested_move_in_expectation() -> MovedPanelExpectation {
    MovedPanelExpectation::new(
        WorkbenchRevision::new(11),
        context_id(2),
        tab_id(1_000),
        panel_id(2_011),
        panel_id(4_011),
        WorkbenchPanelSplitDirection::Below,
    )
}

/// One close of the split's source leaf: the committed graft collapses and
/// the added leaf retains the source's slot inside the inner Split. Three
/// leaves survive — a split adds a leaf, so removing the source alone cannot
/// return the tree to two leaves.
fn nested_split_source_closed_root() -> PanelNode {
    graft(
        graft(
            leaf_new(),
            2_101,
            leaf_c(),
            2_102,
            PanelOrientation::Vertical,
            2_100,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    )
}

/// Only leaf_c and leaf_b remain under the root Split. This shape is
/// reachable in one close after the move-out commit (the moved leaf's close
/// collapses the graft and leaf_b keeps its slot), but only after a second
/// close following a split commit (the added leaf must go too before the
/// inner Split collapses).
fn nested_closed_source_root() -> PanelNode {
    graft(
        leaf_c(),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    )
}

fn rejected_nested_split(root: PanelNode) {
    let baseline = nested_baseline(1, 10);
    let candidate = candidate_with_root(1, 11, root);
    assert_eq!(
        validate_split_panel(&baseline, &split_expectation(), candidate),
        Err(NativeProjectionDivergence)
    );
}

fn rejected_nested_move(root: PanelNode) {
    let baseline = nested_baseline(1, 10);
    let candidate = candidate_with_root(1, 11, root);
    assert_eq!(
        validate_moved_panel(&baseline, &move_expectation(), candidate),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn split_expectation_accepts_a_graft_nested_inside_an_inner_split() {
    let baseline = nested_baseline(1, 10);
    let candidate = candidate_with_root(1, 11, nested_split_candidate());
    assert!(validate_split_panel(&baseline, &split_expectation(), candidate).is_ok());
}

#[test]
fn split_expectation_accepts_a_nested_graft_for_a_below_direction() {
    // A Below split of the depth-two leaf produces a vertical graft.
    let baseline = nested_baseline(1, 10);
    let mut expected = split_expectation();
    expected.direction = WorkbenchPanelSplitDirection::Below;
    let candidate = candidate_with_root(
        1,
        11,
        graft(
            graft(
                graft(
                    leaf_a(),
                    3_001,
                    leaf_new(),
                    3_002,
                    PanelOrientation::Vertical,
                    3_000,
                ),
                2_101,
                leaf_c(),
                2_102,
                PanelOrientation::Vertical,
                2_100,
            ),
            2_001,
            leaf_b(),
            2_002,
            PanelOrientation::Horizontal,
            2_000,
        ),
    );
    assert!(validate_split_panel(&baseline, &expected, candidate).is_ok());
}

#[test]
fn split_expectation_rejects_nested_grafts_under_the_wrong_member() {
    // The graft landed under leaf_c's slot instead of leaf_a's: identities stay
    // unique, but reducing the graft cannot reproduce the baseline inner Split.
    rejected_nested_split(graft(
        graft(
            leaf_c(),
            2_101,
            split_graft(),
            2_102,
            PanelOrientation::Vertical,
            2_100,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
}

#[test]
fn split_expectation_rejects_nested_wrong_members_positions_and_identities() {
    // The source member keeps its Panel identity but mutates its tuple.
    rejected_nested_split(graft(
        graft(
            graft(
                panel(1_001, 1_099, 1_003, 1_004, "1"),
                3_001,
                leaf_new(),
                3_002,
                PanelOrientation::Horizontal,
                3_000,
            ),
            2_101,
            leaf_c(),
            2_102,
            PanelOrientation::Vertical,
            2_100,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // The nested graft carries the wrong orientation for a Right split.
    rejected_nested_split(graft(
        graft(
            graft(
                leaf_a(),
                3_001,
                leaf_new(),
                3_002,
                PanelOrientation::Vertical,
                3_000,
            ),
            2_101,
            leaf_c(),
            2_102,
            PanelOrientation::Vertical,
            2_100,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // The nested graft carries foreign slot identities.
    rejected_nested_split(graft(
        graft(
            graft(
                leaf_a(),
                3_011,
                leaf_new(),
                3_012,
                PanelOrientation::Horizontal,
                3_000,
            ),
            2_101,
            leaf_c(),
            2_102,
            PanelOrientation::Vertical,
            2_100,
        ),
        2_001,
        leaf_b(),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // The expectation names the inner baseline Split, so a candidate that
    // merely reproduces the baseline shape is not a fresh graft.
    let baseline = nested_baseline(1, 10);
    let mut expected = split_expectation();
    expected.split_id = split_id(2_100);
    expected.first_slot_id = slot_id(2_101);
    expected.second_slot_id = slot_id(2_102);
    expected.new_panel_id = panel_id(4_011);
    expected.presentation_id = presentation_id(4_012);
    expected.requested_label = label("aux");
    expected.direction = WorkbenchPanelSplitDirection::Below;
    let candidate = candidate_with_root(1, 11, nested_root());
    assert_eq!(
        validate_split_panel(&baseline, &expected, candidate),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn move_expectation_accepts_a_nested_source_collapsing_its_ancestor() {
    let baseline = nested_baseline(1, 10);
    let candidate = candidate_with_root(1, 11, nested_move_out_candidate());
    assert!(validate_moved_panel(&baseline, &move_expectation(), candidate).is_ok());
}

#[test]
fn move_expectation_accepts_a_move_into_a_deeper_target() {
    let baseline = nested_baseline(1, 10);
    let candidate = candidate_with_root(1, 11, nested_move_in_candidate());
    assert!(validate_moved_panel(&baseline, &nested_move_in_expectation(), candidate).is_ok());
}

#[test]
fn move_expectation_rejects_nested_wrong_members_positions_and_identities() {
    // The moved child keeps its Panel identity but mutates its member tuple.
    rejected_nested_move(graft(
        leaf_c(),
        2_001,
        graft(
            leaf_b(),
            3_001,
            panel(1_001, 1_099, 1_003, 1_004, "1"),
            3_002,
            PanelOrientation::Horizontal,
            3_000,
        ),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // Moved child first for a Right move: the position does not match the
    // committed direction.
    rejected_nested_move(graft(
        leaf_c(),
        2_001,
        graft(
            leaf_a(),
            3_001,
            leaf_b(),
            3_002,
            PanelOrientation::Horizontal,
            3_000,
        ),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
    // The graft reuses the collapsed inner Split's identity instead of being a
    // fresh Split.
    rejected_nested_move(graft(
        leaf_c(),
        2_001,
        graft(
            leaf_b(),
            3_001,
            leaf_a(),
            3_002,
            PanelOrientation::Horizontal,
            2_100,
        ),
        2_002,
        PanelOrientation::Horizontal,
        2_000,
    ));
}

#[test]
fn split_expectation_distinguishes_exact_and_superseding_revisions() {
    let baseline = nested_baseline(1, 10);
    let expected = split_expectation();
    // At the commit revision the exact graft is mandatory: even the shape one
    // close of the source leaf would produce is rejected while the revision
    // still equals the commit.
    let at_commit = candidate_with_root(1, 11, nested_split_source_closed_root());
    assert_eq!(
        validate_split_panel(&baseline, &expected, at_commit),
        Err(NativeProjectionDivergence)
    );
    // r12: one peer close removes the source leaf; the committed graft
    // collapses and the added leaf retains slot 2_101. That exact three-leaf
    // shape is a legal superseding read.
    let superseding = candidate_with_root(1, 12, nested_split_source_closed_root());
    assert!(validate_split_panel(&baseline, &expected, superseding).is_ok());
    // r13: a second peer close removes the added leaf too; the inner Split
    // collapses and leaf_c inherits the root's first slot — still superseding.
    let further = candidate_with_root(1, 13, nested_closed_source_root());
    assert!(validate_split_panel(&baseline, &expected, further).is_ok());
    // The added leaf closed instead at r12: the candidate rolls back to the
    // baseline shape, which is still superseding.
    let rolled_back = candidate_with_root(1, 12, nested_root());
    assert!(validate_split_panel(&baseline, &expected, rolled_back).is_ok());
}

#[test]
fn move_expectation_distinguishes_exact_and_superseding_revisions() {
    let baseline = nested_baseline(1, 10);
    let expected = move_expectation();
    // A move adds no leaf, so the post-close two-leaf shape below already
    // diverges from the committed tree at the commit revision and is rejected.
    let at_commit = candidate_with_root(1, 11, nested_closed_source_root());
    assert_eq!(
        validate_moved_panel(&baseline, &expected, at_commit),
        Err(NativeProjectionDivergence)
    );
    // r12: one peer close of the moved leaf collapses the committed graft and
    // leaf_b keeps slot 2_002 — reachable in a single close because a move
    // adds no leaf.
    let superseding = candidate_with_root(1, 12, nested_closed_source_root());
    assert!(validate_moved_panel(&baseline, &expected, superseding).is_ok());
}

#[test]
fn superseding_candidates_still_reject_mutated_members_and_structure() {
    let baseline = nested_baseline(1, 10);
    let split = split_expectation();
    let moved = move_expectation();
    // leaf_c survives the superseding edits but its presentation identity
    // changed: member-tuple retention still applies at later revisions.
    let mutated_member = candidate_with_root(
        1,
        12,
        graft(
            panel(4_011, 4_099, 4_013, 4_014, "aux"),
            2_001,
            leaf_b(),
            2_002,
            PanelOrientation::Horizontal,
            2_000,
        ),
    );
    assert_eq!(
        validate_split_panel(&baseline, &split, mutated_member.clone()),
        Err(NativeProjectionDivergence)
    );
    assert_eq!(
        validate_moved_panel(&baseline, &moved, mutated_member),
        Err(NativeProjectionDivergence)
    );
    // The inner Split survives but its orientation flipped: structure-tuple
    // retention rejects it even though every member tuple is intact.
    let flipped_structure = candidate_with_root(
        1,
        12,
        graft(
            graft(
                leaf_a(),
                2_101,
                leaf_c(),
                2_102,
                PanelOrientation::Horizontal,
                2_100,
            ),
            2_001,
            leaf_b(),
            2_002,
            PanelOrientation::Horizontal,
            2_000,
        ),
    );
    assert_eq!(
        validate_split_panel(&baseline, &split, flipped_structure.clone()),
        Err(NativeProjectionDivergence)
    );
    assert_eq!(
        validate_moved_panel(&baseline, &moved, flipped_structure),
        Err(NativeProjectionDivergence)
    );
}
