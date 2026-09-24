use super::*;
use orchbee_workbench_protocol::{PanelLabelUpdate, WorkbenchResponse, WorkbenchRevision};
use serde_json::Value;

fn fixture(shape: usize) -> (WorkbenchAuthority, WorkbenchGrant, WorkbenchTabId) {
    let (mut authority, grant) = authority();
    let member = authority.tree_standalone_membership().unwrap().members()[0].clone();
    let tab = member.tab_id().clone();
    for step in 0..4 {
        let current = leaves(&authority.projection.snapshot().terminal_contexts[0].tabs[0].root);
        let panel = leaf_panel_id(&current[(shape + step) % current.len()]);
        let direction = [
            WorkbenchPanelSplitDirection::Left,
            WorkbenchPanelSplitDirection::Above,
            WorkbenchPanelSplitDirection::Right,
            WorkbenchPanelSplitDirection::Below,
        ][(shape + step) % 4];
        split(
            &mut authority,
            &grant,
            100 + step as u128,
            &tab,
            &panel,
            direction,
        );
    }
    let panels = leaves(&authority.projection.snapshot().terminal_contexts[0].tabs[0].root);
    for (index, panel) in panels.iter().enumerate() {
        let rename = command(
            &authority,
            200 + index as u128,
            WorkbenchAction::PanelRename {
                terminal_context_id: standalone_context(&authority),
                tab_id: tab.clone(),
                panel_id: leaf_panel_id(panel),
                label: PanelLabelUpdate::parse(Some(&format!("标题 {index}"))).unwrap(),
            },
        );
        assert!(
            matches!(authority.execute(Some(&grant), WorkbenchRequest::Command(rename)
            .try_into().unwrap()), WorkbenchResponse::Command(result)
            if matches!(result.outcome, WorkbenchCommandOutcome::Renamed { .. }))
        );
    }
    (authority, grant, tab)
}

fn request(
    authority: &WorkbenchAuthority,
    id: u128,
    tab: &WorkbenchTabId,
    source: &WorkbenchPanelId,
    target: &WorkbenchPanelId,
) -> WorkbenchCommand {
    command(
        authority,
        id,
        WorkbenchAction::PanelSwap {
            terminal_context_id: standalone_context(authority),
            tab_id: tab.clone(),
            panel_id: source.clone(),
            target_panel_id: target.clone(),
        },
    )
}

// The reference swaps JSON addresses by ordinal; it does not use production ID lookup or traversal.
fn leaf_addresses(node: &Value, prefix: &str, output: &mut Vec<String>) {
    if node["kind"] == "panel" {
        output.push(prefix.to_owned());
    } else {
        for (index, child) in node["children"].as_array().unwrap().iter().enumerate() {
            leaf_addresses(
                &child["node"],
                &format!("{prefix}/children/{index}/node"),
                output,
            );
        }
    }
}

#[test]
fn bounded_nested_permutations_preserve_complete_nodes_and_are_involutions() {
    for shape in 0..4 {
        for source in 0..5 {
            for target in 0..5 {
                let (mut authority, grant, tab) = fixture(shape);
                let before = authority.projection.snapshot().clone();
                let panels = leaves(&before.terminal_contexts[0].tabs[0].root);
                let source_id = leaf_panel_id(&panels[source]);
                let target_id = leaf_panel_id(&panels[target]);
                let cursor = authority.last_identity;
                let swap = request(&authority, 1000, &tab, &source_id, &target_id);
                assert!(
                    WorkbenchReadOrNavigationRequest::try_from(WorkbenchRequest::Command(
                        swap.clone()
                    ))
                    .is_err()
                );
                let accepted = submit(&mut authority, &grant, swap.clone());
                let changed = source != target;
                assert_eq!(
                    matches!(
                        accepted.outcome,
                        WorkbenchCommandOutcome::PanelSwapped { .. }
                    ),
                    changed
                );
                assert_eq!(authority.last_identity, cursor);
                let mut expected = serde_json::to_value(&before).unwrap();
                let root = &mut expected["terminalContexts"][0]["tabs"][0]["root"];
                let mut addresses = Vec::new();
                leaf_addresses(root, "", &mut addresses);
                let a = root.pointer(&addresses[source]).unwrap().clone();
                let b = root.pointer(&addresses[target]).unwrap().clone();
                *root.pointer_mut(&addresses[source]).unwrap() = b;
                *root.pointer_mut(&addresses[target]).unwrap() = a;
                expected["revision"] =
                    Value::String((before.revision.value() + u64::from(changed)).to_string());
                assert_eq!(
                    serde_json::to_value(authority.projection.snapshot()).unwrap(),
                    expected,
                    "shape={shape}, source={source}, target={target}"
                );
                assert_eq!(submit(&mut authority, &grant, swap.clone()), accepted);
                let inverse = request(&authority, 1001, &tab, &source_id, &target_id);
                let restored = submit(&mut authority, &grant, inverse);
                assert_eq!(
                    matches!(
                        restored.outcome,
                        WorkbenchCommandOutcome::PanelSwapped { .. }
                    ),
                    changed
                );
                let mut restored_snapshot = authority.projection.snapshot().clone();
                restored_snapshot.revision = before.revision;
                assert_eq!(restored_snapshot, before);
                assert_eq!(authority.last_identity, cursor);
                assert_eq!(submit(&mut authority, &grant, swap), accepted);
            }
        }
    }
}

#[test]
fn swap_refusals_preserve_projection_and_allocates_no_identity_even_at_exhaustion() {
    let (mut authority, grant, tab) = fixture(0);
    let before = authority.projection.snapshot().clone();
    let panels = leaves(&before.terminal_contexts[0].tabs[0].root);
    let source = leaf_panel_id(&panels[0]);
    let target = leaf_panel_id(&panels[4]);
    let swap = request(&authority, 1100, &tab, &source, &target);
    assert!(
        matches!(authority.begin_terminal_panel_placement(None, swap.clone()),
        WorkbenchPanelPlacementBegin::Ready(result) if result.outcome == WorkbenchCommandOutcome::Denied {})
    );
    for (id, reason) in [
        (1101, WorkbenchRejectionReason::RevisionMismatch),
        (1102, WorkbenchRejectionReason::PanelUnavailable),
    ] {
        let mut invalid = request(&authority, id, &tab, &source, &target);
        if id == 1101 {
            invalid.expected_revision = WorkbenchRevision::new(0);
        } else if let WorkbenchAction::PanelSwap {
            target_panel_id, ..
        } = &mut invalid.action
        {
            *target_panel_id = WorkbenchPanelId::parse(&identity(999999)).unwrap();
        }
        assert!(matches!(submit(&mut authority, &grant, invalid).outcome,
            WorkbenchCommandOutcome::Rejected { reason: actual, .. } if actual == reason));
        assert_eq!(authority.projection.snapshot(), &before);
    }
    authority.ledger_bytes = crate::workbench_authority::MAXIMUM_LEDGER_BYTES;
    let retained = authority.ledger.len();
    assert_eq!(
        submit(&mut authority, &grant, swap.clone()).outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(authority.ledger.len(), retained);
    assert_eq!(authority.projection.snapshot(), &before);
    authority.ledger_bytes = 0;
    authority.last_identity = u128::MAX;
    let accepted = submit(&mut authority, &grant, swap.clone());
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::PanelSwapped { .. }
    ));
    assert_eq!(authority.last_identity, u128::MAX);
    let mut conflict = swap;
    if let WorkbenchAction::PanelSwap {
        panel_id,
        target_panel_id,
        ..
    } = &mut conflict.action
    {
        std::mem::swap(panel_id, target_panel_id);
    }
    assert!(matches!(
        submit(&mut authority, &grant, conflict).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
}

#[test]
fn swap_revision_exhaustion_still_allows_exact_noop_without_allocating() {
    let (mut authority, grant, tab) = fixture(1);
    let mut before = authority.projection.snapshot().clone();
    before.revision = WorkbenchRevision::new(u64::MAX);
    authority
        .projection
        .advance_projection(
            &serialize_document(&WorkbenchProtocolDocument::Snapshot(before.clone())).unwrap(),
        )
        .unwrap();
    let panels = leaves(&before.terminal_contexts[0].tabs[0].root);
    let a = leaf_panel_id(&panels[0]);
    let b = leaf_panel_id(&panels[1]);
    let cursor = authority.last_identity;
    let noop = request(&authority, 1200, &tab, &a, &a);
    assert!(matches!(submit(&mut authority, &grant, noop).outcome,
        WorkbenchCommandOutcome::Unchanged { revision } if revision == before.revision));
    let swap = request(&authority, 1201, &tab, &a, &b);
    assert!(matches!(
        submit(&mut authority, &grant, swap).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionExhausted,
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
}
