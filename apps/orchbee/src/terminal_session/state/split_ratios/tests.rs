//! Ratio ownership boundaries and serializable mutation/replacement traces.
use super::*;
use orchbee_workbench_protocol::{ClientSplitChange, WorkbenchPanelSlotId};
use serde_json::json;

fn snapshot(count: usize, revision: u64) -> Snapshot {
    let tabs: Vec<_> = (0..count).map(|index| {
        let base = 100 + index * 10;
        let leaf = |number: usize| json!({"kind":"panel","panelId":format!("{number:032x}"),
            "primaryPresentation":{"binding":null,"label":"shell","presentationId":format!("{:032x}",number+1)},
            "childPresentations":[]});
        json!({"tabId":format!("{base:032x}"),"label":"tab","groupId":null,"pinned":false,
            "root":{"kind":"split","splitId":format!("{:032x}",base+1),"orientation":"horizontal",
                "children":[{"slotId":format!("{:032x}",base+2),"node":leaf(base+3)},
                            {"slotId":format!("{:032x}",base+5),"node":leaf(base+6)}]}})
    }).collect();
    let contexts: Vec<_> = tabs
        .chunks(256)
        .enumerate()
        .map(|(index, tabs)| {
            json!({
                "terminalContextId":format!("{:032x}",2+index),"groups":[],"tabs":tabs
            })
        })
        .collect();
    let workspaces:Vec<_>=(1..contexts.len()).map(|index|json!({
        "workspaceId":format!("{:032x}",80_000+index),"label":"workspace","primaryDirectoryId":null,
        "terminalContextId":format!("{:032x}",2+index)
    })).collect();
    let projects = if workspaces.is_empty() {
        vec![]
    } else {
        vec![json!({
            "projectId":format!("{:032x}",90_000),"label":"project","directories":[],"workspaces":workspaces
        })]
    };
    serde_json::from_value(json!({"protocolVersion":1,"revision":revision.to_string(),
        "siteId":format!("{:032x}",1),
        "navigation":{"projects":projects,"standaloneTerminalContextId":format!("{:032x}",2)},
        "terminalContexts":contexts}))
    .unwrap()
}
fn change(snapshot: &Snapshot, index: usize, before: f64, after: f64) -> ClientLayoutAdjustment {
    let context = &snapshot.terminal_contexts[index / 256];
    let tab = &context.tabs[index % 256];
    let PanelNode::Split {
        split_id, children, ..
    } = &tab.root
    else {
        panic!("fixture split")
    };
    ClientLayoutAdjustment {
        terminal_context_id: context.terminal_context_id.clone(),
        tab_id: tab.tab_id.clone(),
        workbench_revision: snapshot.revision,
        selected: false,
        change: ClientLayoutChange::Split {
            split: ClientSplitChange {
                split_id: split_id.clone(),
                first_slot_id: children[0].slot_id.clone(),
                second_slot_id: children[1].slot_id.clone(),
                previous_ratio: ClientSplitRatio::new(before).unwrap(),
                ratio: ClientSplitRatio::new(after).unwrap(),
            },
        },
    }
}
fn observed(ratios: &SplitRatios, adjustment: &ClientLayoutAdjustment) -> f64 {
    let ClientLayoutChange::Split { split } = &adjustment.change else {
        panic!("fixture split")
    };
    ratios
        .get(
            &adjustment.terminal_context_id,
            &adjustment.tab_id,
            &split.split_id,
            &split.first_slot_id,
            &split.second_slot_id,
        )
        .value()
}

#[test]
fn changed_commit_retains_inactive_targets_and_noop_never_prunes() {
    let mut snapshot = snapshot(2, 1);
    let mut ratios = SplitRatios::default();
    let first = change(&snapshot, 0, 0.5, 0.7);
    let second = change(&snapshot, 1, 0.5, 0.3);
    ratios.commit(&snapshot, &first).unwrap();
    ratios.commit(&snapshot, &second).unwrap();
    assert_eq!(observed(&ratios, &first), 0.7);
    assert_eq!(observed(&ratios, &second), 0.3);
    snapshot.terminal_contexts[0].tabs.remove(1);
    snapshot.revision = WorkbenchRevision::new(2);
    let before = ratios.clone();
    ratios
        .commit(&snapshot, &change(&snapshot, 0, 0.7, 0.7))
        .unwrap();
    assert_eq!(ratios, before, "no-op is not garbage collection");
    ratios
        .commit(&snapshot, &change(&snapshot, 0, 0.7, 0.5))
        .unwrap();
    assert!(
        ratios.entries.is_empty(),
        "changed commit prunes removed membership and default overrides"
    );
}

#[test]
fn semantic_events_ratio_sequence_exhaustion_preserves_owner_and_noop_still_succeeds() {
    let snapshot = snapshot(1, 1);
    let mut ratios = SplitRatios {
        sequence: u64::MAX,
        ..SplitRatios::default()
    };
    let before = ratios.clone();
    assert_eq!(
        ratios.commit(&snapshot, &change(&snapshot, 0, 0.5, 0.7)),
        Err(Reason::Busy)
    );
    assert_eq!(ratios, before);
    ratios
        .commit(&snapshot, &change(&snapshot, 0, 0.5, 0.5))
        .unwrap();
    assert_eq!(ratios, before);
}

#[test]
fn slot_replacement_and_stale_snapshot_never_inherit_or_overwrite_weights() {
    let mut snapshot = snapshot(1, 2);
    let mut ratios = SplitRatios::default();
    let original = change(&snapshot, 0, 0.5, 0.7);
    ratios.commit(&snapshot, &original).unwrap();
    let before = ratios.clone();
    assert_eq!(ratios.commit(&snapshot, &original), Err(Reason::StaleView));
    let mut stale = snapshot.clone();
    stale.revision = WorkbenchRevision::new(1);
    assert_eq!(
        ratios.commit(&stale, &change(&stale, 0, 0.7, 0.6)),
        Err(Reason::StaleView)
    );
    assert_eq!(ratios, before);
    let PanelNode::Split { children, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    children.swap(0, 1);
    let replacement = change(&snapshot, 0, 0.5, 0.3);
    assert_eq!(observed(&ratios, &replacement), 0.5);
    assert_eq!(
        ratios.commit(&snapshot, &original),
        Err(Reason::TargetUnavailable)
    );
    ratios.commit(&snapshot, &replacement).unwrap();
    assert_eq!(ratios.entries.len(), 1);
    assert_eq!(observed(&ratios, &replacement), 0.3);
}

#[test]
fn capacity_refusal_is_atomic_and_existing_entries_remain_editable() {
    // Defensive owner-capacity boundary. This typed topology obeys per-context limits,
    // but intentionally exceeds the smaller encoded Snapshot budget; no wire admission is claimed.
    let snapshot = snapshot(MAXIMUM_OVERRIDES + 1, 1);
    let mut ratios = SplitRatios::default();
    for index in 0..MAXIMUM_OVERRIDES {
        let adjustment = change(&snapshot, index, 0.5, 0.7);
        let ClientLayoutChange::Split { split } = adjustment.change else {
            panic!()
        };
        ratios.entries.insert(
            (
                adjustment.terminal_context_id,
                adjustment.tab_id,
                split.split_id,
            ),
            Entry {
                first: split.first_slot_id,
                second: split.second_slot_id,
                ratio: split.ratio,
            },
        );
    }
    let before = ratios.clone();
    assert_eq!(
        ratios.commit(&snapshot, &change(&snapshot, MAXIMUM_OVERRIDES, 0.5, 0.7)),
        Err(Reason::Busy)
    );
    assert_eq!(ratios, before);
    ratios
        .commit(&snapshot, &change(&snapshot, 0, 0.7, 0.6))
        .unwrap();
    assert_eq!(observed(&ratios, &change(&snapshot, 0, 0.6, 0.6)), 0.6);
}

fn replay(actions: &[[u8; 2]]) -> Result<(), String> {
    let mut snapshot = snapshot(1, 1);
    let mut ratios = SplitRatios::default();
    let mut expected = 0.5;
    for &[kind, value] in actions {
        match kind {
            0 => {
                let next = f64::from(1 + value % 9) / 10.0;
                let adjustment = change(&snapshot, 0, expected, next);
                ratios
                    .commit(&snapshot, &adjustment)
                    .map_err(|reason| format!("{reason:?}"))?;
                expected = next;
            }
            1 => {
                snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
                let PanelNode::Split { children, .. } =
                    &mut snapshot.terminal_contexts[0].tabs[0].root
                else {
                    panic!()
                };
                children[0].slot_id = WorkbenchPanelSlotId::parse(&format!(
                    "{:032x}",
                    50_000 + snapshot.revision.value()
                ))
                .unwrap();
                expected = 0.5;
            }
            2 => {
                let before = ratios.clone();
                ratios
                    .commit(&snapshot, &change(&snapshot, 0, expected, expected))
                    .map_err(|reason| format!("{reason:?}"))?;
                if ratios != before {
                    return Err("no-op mutated state".into());
                }
            }
            _ => return Err("unknown trace action".into()),
        }
        if observed(&ratios, &change(&snapshot, 0, expected, expected)) != expected {
            return Err("ratio crossed identity or disagreed with last commit".into());
        }
        if ratios.entries.len() > 1 {
            return Err("replaced identities leaked entries".into());
        }
    }
    Ok(())
}
#[test]
fn generated_ratio_and_slot_replacement_traces_have_reduced_json_replay() {
    for seed in [17_u64, 41, 701, 991] {
        let mut random = seed;
        let mut actions = Vec::new();
        for _ in 0..256 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            actions.push([
                u8::try_from(random % 3).unwrap(),
                u8::try_from((random >> 8) % 256).unwrap(),
            ]);
        }
        let encoded = serde_json::to_string(&actions).unwrap();
        let mut decoded: Vec<[u8; 2]> = serde_json::from_str(&encoded).unwrap();
        if let Err(reason) = replay(&decoded) {
            let mut index = 0;
            while index < decoded.len() {
                let mut candidate = decoded.clone();
                candidate.remove(index);
                if replay(&candidate).is_err() {
                    decoded = candidate;
                    index = 0;
                } else {
                    index += 1;
                }
            }
            panic!(
                "seed={seed} {reason} replay={}",
                serde_json::to_string(&decoded).unwrap()
            );
        }
    }
}
