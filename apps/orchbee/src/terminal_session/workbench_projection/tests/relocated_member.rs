//! `panel.relocate` consumer coverage: a retained member tuple may change its
//! owning Tab — the one authorized cross-Tab delta — while every
//! identity-bearing field and every surviving structural identity stay exact.

use super::*;

/// Baseline: Tab 1 is Split(left, right); Tab 2 is a single leaf. The moved
/// member is `right` — panel `35`, presentation `36`, binding `37/38`.
fn two_tab_split_snapshot(revision: u64) -> Snapshot {
    let mut snapshot = snapshot(2, revision);
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

/// The relocated destination root: the destination's previous leaf first and
/// the moved leaf second under a fresh vertical Split — the `below` product.
fn grafted_destination(direction_moved_first: bool) -> PanelNode {
    let (first, second) = if direction_moved_first {
        (panel(35, 36, 37, 38, "right"), panel(1011, 1012, 1013, 1014, "2"))
    } else {
        (panel(1011, 1012, 1013, 1014, "2"), panel(35, 36, 37, 38, "right"))
    };
    PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: first,
                slot_id: slot_id(91),
            },
            PanelSlot {
                node: second,
                slot_id: slot_id(92),
            },
        ]),
        orientation: PanelOrientation::Vertical,
        split_id: split_id(90),
    }
}

#[test]
fn canonical_refresh_accepts_a_member_relocated_into_a_sibling_tab() {
    let current = project_native_standalone_tabs(&two_tab_split_snapshot(10)).unwrap();

    // Move `right` below Tab 2's leaf: Tab 1 collapses onto `left`; Tab 2's
    // root becomes a fresh Split(previous, moved). Every moved identity is
    // retained verbatim — only the owning Tab changed.
    let mut candidate_snapshot = two_tab_split_snapshot(11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root =
        panel(30, 31, 32, 33, "left");
    candidate_snapshot.terminal_contexts[0].tabs[1].root = grafted_destination(false);
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_accepts_a_member_relocated_into_a_fresh_tab() {
    let current = project_native_standalone_tabs(&two_tab_split_snapshot(10)).unwrap();

    // Move `right` into a brand-new Tab: the appended Tab's root is exactly the
    // retained leaf and its Tab identity is fresh — no baseline comparison.
    let mut candidate_snapshot = two_tab_split_snapshot(11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root =
        panel(30, 31, 32, 33, "left");
    candidate_snapshot.terminal_contexts[0].tabs.push(Tab {
        group_id: None,
        label: "3".into(),
        pinned: false,
        root: panel(35, 36, 37, 38, "right"),
        tab_id: tab_id(7000),
    });
    let candidate = project_native_standalone_tabs(&candidate_snapshot).unwrap();
    assert_eq!(
        validate_canonical_refresh(&current, candidate.clone()),
        Ok(candidate)
    );
}

#[test]
fn canonical_refresh_rejects_a_relocated_member_whose_identity_mutated() {
    let current = project_native_standalone_tabs(&two_tab_split_snapshot(10)).unwrap();

    // The moved leaf landed under the destination Tab but its binding changed:
    // the Panel identity overlaps a baseline member while the tuple differs.
    for (worker, terminal, presentation, leaf_label) in [
        (37, 99, 36, "right"),
        (99, 38, 36, "right"),
        (37, 38, 99, "right"),
        (37, 38, 36, "renamed"),
    ] {
        let mut candidate_snapshot = two_tab_split_snapshot(11);
        candidate_snapshot.terminal_contexts[0].tabs[0].root =
            panel(30, 31, 32, 33, "left");
        candidate_snapshot.terminal_contexts[0].tabs[1].root = PanelNode::Split {
            children: Box::new([
                PanelSlot {
                    node: panel(1011, 1012, 1013, 1014, "2"),
                    slot_id: slot_id(91),
                },
                PanelSlot {
                    node: panel(35, presentation, worker, terminal, leaf_label),
                    slot_id: slot_id(92),
                },
            ]),
            orientation: PanelOrientation::Vertical,
            split_id: split_id(90),
        };
        assert_eq!(
            validate_canonical_refresh(
                &current,
                project_native_standalone_tabs(&candidate_snapshot).unwrap()
            ),
            Err(NativeProjectionDivergence),
            "mutated member fields must diverge"
        );
    }
}

#[test]
fn canonical_refresh_rejects_a_surviving_tab_that_keeps_no_member() {
    let current = project_native_standalone_tabs(&two_tab_split_snapshot(10)).unwrap();

    // Tab 1's identity survives but both original members left: `right` moved
    // into Tab 2 and `left` was replaced by a fresh leaf. The authority never
    // mints this — `last-panel-in-tab` keeps one member behind — so the
    // consumer treats a surviving-but-empty Tab identity as divergence.
    let mut candidate_snapshot = two_tab_split_snapshot(11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root =
        panel(60, 61, 62, 63, "new");
    candidate_snapshot.terminal_contexts[0].tabs[1].root = grafted_destination(false);
    assert_eq!(
        validate_canonical_refresh(
            &current,
            project_native_standalone_tabs(&candidate_snapshot).unwrap()
        ),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn canonical_refresh_rejects_a_retained_split_identity_reparented_across_tabs() {
    let current = project_native_standalone_tabs(&two_tab_split_snapshot(10)).unwrap();

    // The moved leaf stayed exact but the SOURCE Split identity reappeared as
    // the destination's graft: structural tuples stay bound to their owning
    // Tab, so a Split may never follow its leaf across the boundary.
    let mut candidate_snapshot = two_tab_split_snapshot(11);
    candidate_snapshot.terminal_contexts[0].tabs[0].root =
        panel(30, 31, 32, 33, "left");
    candidate_snapshot.terminal_contexts[0].tabs[1].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(1011, 1012, 1013, 1014, "2"),
                slot_id: slot_id(91),
            },
            PanelSlot {
                node: panel(35, 36, 37, 38, "right"),
                slot_id: slot_id(92),
            },
        ]),
        orientation: PanelOrientation::Vertical,
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
