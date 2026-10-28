//! Seeded topology/viewport traces with deletion shrinking and JSON replay.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug)]
enum Action {
    Split { target: u8, vertical: bool },
    Collapse { target: u8 },
    Focus { target: u8 },
    Resize { width: u16, height: u16 },
    Ratio { target: u8, tenths: u8 },
}

fn encode(actions: &[Action]) -> String {
    let records: Vec<[u16; 3]> = actions
        .iter()
        .map(|action| match *action {
            Action::Split { target, vertical } => [0, u16::from(target), u16::from(vertical)],
            Action::Collapse { target } => [1, u16::from(target), 0],
            Action::Focus { target } => [2, u16::from(target), 0],
            Action::Resize { width, height } => [3, width, height],
            Action::Ratio { target, tenths } => [4, u16::from(target), u16::from(tenths)],
        })
        .collect();
    serde_json::to_string(&records).unwrap()
}

fn decode(encoded: &str) -> Vec<Action> {
    let records: Vec<[u16; 3]> = serde_json::from_str(encoded).unwrap();
    records
        .into_iter()
        .map(|[kind, first, second]| match kind {
            0 => Action::Split {
                target: u8::try_from(first).unwrap(),
                vertical: second != 0,
            },
            1 => Action::Collapse {
                target: u8::try_from(first).unwrap(),
            },
            2 => Action::Focus {
                target: u8::try_from(first).unwrap(),
            },
            3 => Action::Resize {
                width: first,
                height: second,
            },
            4 => Action::Ratio {
                target: u8::try_from(first).unwrap(),
                tenths: u8::try_from(second).unwrap(),
            },
            _ => panic!("unknown serialized action {kind}"),
        })
        .collect()
}

fn ids(tree: &ClientPanelTree, leaves: &mut Vec<usize>, splits: &mut Vec<String>) {
    match tree {
        ClientPanelTree::Panel { panel_index } => leaves.push(*panel_index),
        ClientPanelTree::Split {
            first,
            second,
            split_id,
            ..
        } => {
            splits.push(split_id.to_string());
            ids(first, leaves, splits);
            ids(second, leaves, splits);
        }
    }
}

fn insert(tree: &mut ClientPanelTree, target: usize, next: usize, vertical: bool) {
    match tree {
        ClientPanelTree::Panel { panel_index } if *panel_index == target => {
            *tree = split(
                panel(target),
                panel(next),
                if vertical {
                    PanelOrientation::Vertical
                } else {
                    PanelOrientation::Horizontal
                },
                100 + u128::try_from(next).unwrap() * 3,
            );
        }
        ClientPanelTree::Split { first, second, .. } => {
            insert(first, target, next, vertical);
            insert(second, target, next, vertical);
        }
        ClientPanelTree::Panel { .. } => {}
    }
}

fn collapse(tree: &mut ClientPanelTree, target: usize) -> bool {
    match tree {
        ClientPanelTree::Panel { panel_index } => *panel_index == target,
        ClientPanelTree::Split { first, second, .. } => {
            if collapse(first, target) {
                *tree = (**second).clone();
            } else if collapse(second, target) {
                *tree = (**first).clone();
            }
            false
        }
    }
}

// Bits classify split directions, collapse, focus, resize, nested topology,
// collapsed geometry and fully visible geometry. Replay returns the first invariant failure.
fn replay(actions: &[Action]) -> Result<u16, String> {
    let mut tree = panel(0);
    let mut next = 1;
    let mut focus = Some(0);
    let mut area = Rect::new(7, 9, 120, 40);
    let mut coverage = 0_u16;
    let mut ratios = std::collections::BTreeMap::new();
    for action in actions {
        let mut leaves = Vec::new();
        ids(&tree, &mut leaves, &mut Vec::new());
        match *action {
            Action::Split { target, vertical } if leaves.len() < 24 => {
                insert(
                    &mut tree,
                    leaves[usize::from(target) % leaves.len()],
                    next,
                    vertical,
                );
                next += 1;
                coverage |= if vertical { 2 } else { 1 };
            }
            Action::Collapse { target } if leaves.len() > 1 => {
                collapse(&mut tree, leaves[usize::from(target) % leaves.len()]);
                coverage |= 4;
            }
            Action::Focus { target } => {
                focus = Some(leaves[usize::from(target) % leaves.len()]);
                coverage |= 8;
            }
            Action::Resize { width, height } => {
                area = Rect::new(7, 9, width, height);
                coverage |= 16;
            }
            Action::Ratio { target, tenths } => {
                let mut splits = Vec::new();
                ids(&tree, &mut Vec::new(), &mut splits);
                if !splits.is_empty() {
                    ratios.insert(
                        splits[usize::from(target) % splits.len()].clone(),
                        ClientSplitRatio::new(f64::from(tenths) / 10.0).unwrap(),
                    );
                    coverage |= 256;
                }
            }
            _ => {}
        }
        leaves.clear();
        let mut split_ids = Vec::new();
        ids(&tree, &mut leaves, &mut split_ids);
        if focus.is_none_or(|id| !leaves.contains(&id)) {
            focus = leaves.first().copied();
        }
        let projected = project_panel_tree(area, &tree, focus, &|split, _, _| {
            ratios
                .get(split.as_str())
                .copied()
                .unwrap_or(ClientSplitRatio::HALF)
        });
        if split_ids.len() > 1 {
            coverage |= 32;
        }
        if projected.leaves.len() < leaves.len() {
            coverage |= 64;
        } else {
            coverage |= 128;
        }
        if projected
            .splits
            .iter()
            .map(|entry| entry.split_id.to_string())
            .collect::<Vec<_>>()
            != split_ids
        {
            return Err("canonical split preorder/identity changed".into());
        }
        if projected.leaves.is_empty()
            || !projected
                .leaves
                .iter()
                .any(|entry| Some(entry.panel_index) == focus)
        {
            return Err("active leaf disappeared at a valid nonempty viewport".into());
        }
        let mut visible = BTreeSet::new();
        for (index, leaf) in projected.leaves.iter().enumerate() {
            if !leaves.contains(&leaf.panel_index)
                || !visible.insert(leaf.panel_index)
                || leaf.leaf.is_empty()
                || !area.contains((leaf.leaf.x, leaf.leaf.y).into())
                || leaf.leaf.right() > area.right()
                || leaf.leaf.bottom() > area.bottom()
                || leaf.content.x != leaf.leaf.x
                || leaf.content.width + u16::from(leaf.leaf.width > 1) != leaf.leaf.width
                || leaf.content.y != leaf.leaf.y
                || leaf.content.bottom() != leaf.leaf.bottom()
            {
                return Err("leaf membership, bounds or content geometry".into());
            }
            if projected
                .leaves
                .iter()
                .skip(index + 1)
                .any(|other| leaf.leaf.intersects(other.leaf))
                || projected
                    .dividers
                    .iter()
                    .any(|divider| leaf.leaf.intersects(*divider))
            {
                return Err("leaf overlap with another leaf or divider".into());
            }
        }
        if projected
            .splits
            .iter()
            .filter_map(|split| split.divider)
            .collect::<Vec<_>>()
            != projected.dividers
        {
            return Err("reported split dividers differ from renderer".into());
        }
        for split in &projected.splits {
            if let Some(divider) = split.divider {
                let Some(parent) = split.area else {
                    return Err("divider without area".into());
                };
                if divider.right() > parent.right()
                    || divider.bottom() > parent.bottom()
                    || divider.x < parent.x
                    || divider.y < parent.y
                    || match split.orientation {
                        PanelOrientation::Horizontal => {
                            divider.width != 1 || divider.height != parent.height
                        }
                        PanelOrientation::Vertical => {
                            divider.height != 1 || divider.width != parent.width
                        }
                    }
                {
                    return Err("split divider bounds/orientation".into());
                }
            }
        }
        let repeated = project_panel_tree(area, &tree, focus, &|split, _, _| {
            ratios
                .get(split.as_str())
                .copied()
                .unwrap_or(ClientSplitRatio::HALF)
        });
        if repeated.leaves != projected.leaves || repeated.dividers != projected.dividers {
            return Err("read projection changed on replay".into());
        }
    }
    Ok(coverage)
}

fn shrink(mut actions: Vec<Action>, fails: impl Fn(&[Action]) -> bool) -> Vec<Action> {
    let mut index = 0;
    while index < actions.len() {
        let mut candidate = actions.clone();
        candidate.remove(index);
        if fails(&candidate) {
            actions = candidate;
            index = 0;
        } else {
            index += 1;
        }
    }
    actions
}

#[test]
fn generated_hierarchy_changes_preserve_geometry_and_replayable_observation() {
    let mut coverage = 0;
    for seed in [
        0x41c64e6d_u64,
        0x9e3779b9,
        0xd1b54a32,
        0x94d049bb,
        0x8538ebad,
        0xda942042,
        0xa4093822,
        0x13198a2e,
    ] {
        let mut random = seed;
        let mut actions = vec![
            Action::Split {
                target: 0,
                vertical: false,
            },
            Action::Split {
                target: 1,
                vertical: true,
            },
            Action::Resize {
                width: 8,
                height: 6,
            },
            Action::Resize {
                width: 120,
                height: 40,
            },
        ];
        for index in 0..192 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let target = u8::try_from(random & 255).unwrap();
            actions.push(match random % 7 {
                0 | 1 => Action::Split {
                    target,
                    vertical: random & 256 != 0,
                },
                2 => Action::Collapse { target },
                3 => Action::Focus { target },
                4 => Action::Ratio {
                    target,
                    tenths: 1 + u8::try_from((random >> 12) % 9).unwrap(),
                },
                _ => Action::Resize {
                    width: [8, 9, 19, 20, 59, 60, 99, 100, 121][index % 9],
                    height: [2, 3, 6, 7, 11, 12, 23, 24, 41][(index / 9) % 9],
                },
            });
        }
        let encoded = encode(&actions);
        let decoded = decode(&encoded);
        match replay(&decoded) {
            Ok(reached) => coverage |= reached,
            Err(reason) => {
                let reduced = shrink(decoded, |trace| replay(trace).is_err());
                panic!("seed={seed:x}; {reason}; replay={}", encode(&reduced));
            }
        }
    }
    assert_eq!(
        coverage, 511,
        "every declared action/topology/geometry class must be reached"
    );
}

#[test]
fn deletion_shrinker_preserves_failure_and_emits_minimal_replay() {
    let trace = vec![
        Action::Focus { target: 0 },
        Action::Split {
            target: 0,
            vertical: false,
        },
        Action::Resize {
            width: 8,
            height: 6,
        },
    ];
    let reduced = shrink(trace, |actions| {
        actions
            .iter()
            .any(|action| matches!(action, Action::Resize { width: 8, .. }))
    });
    assert_eq!(reduced.len(), 1);
    let encoded = encode(&reduced);
    let replay = decode(&encoded);
    assert!(matches!(
        replay.as_slice(),
        [Action::Resize {
            width: 8,
            height: 6
        }]
    ));
}
