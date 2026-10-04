//! Generated nested graphs and serialized command traces with deletion shrinking.
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Debug)]
struct Step {
    kind: u8,
    target: u8,
    weight: u8,
}

fn tree(depth: u8, counter: &mut u128) -> Value {
    let mut mint = || {
        *counter += 1;
        format!("{:032x}", *counter)
    };
    if depth == 0 {
        return json!({"kind":"panel", "panelId":mint(),
            "primaryPresentation":{"presentationId":mint(),"label":"leaf","binding":null},
            "childPresentations":[]});
    }
    let id = mint();
    let first = mint();
    let second = mint();
    json!({"kind":"split","splitId":id,"orientation":if depth.is_multiple_of(2) {"vertical"} else {"horizontal"},
        "ratio":if depth.is_multiple_of(2) {0.3} else {0.5},
        "children":[{"slotId":first,"node":tree(depth-1,counter)},
            {"slotId":second,"node":tree(depth-1,counter)}]})
}
fn paths(node: &PanelNode, prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    if let PanelNode::Split { children, .. } = node {
        out.push(prefix.clone());
        for (index, child) in children.iter().enumerate() {
            prefix.push(index);
            paths(&child.node, prefix, out);
            prefix.pop();
        }
    }
}
fn node_at<'a>(mut node: &'a mut PanelNode, path: &[usize]) -> &'a mut PanelNode {
    for index in path {
        let PanelNode::Split { children, .. } = node else {
            panic!("generated path")
        };
        node = &mut children[*index].node;
    }
    node
}
fn run(depth: u8, steps: &[Step]) -> Result<(), String> {
    let (mut owner, grant) = fixture(0);
    let mut expected = owner.snapshot().clone();
    expected.terminal_contexts[1].tabs[0].root =
        serde_json::from_value(tree(depth, &mut 50_000)).unwrap();
    let encoded = serde_json::to_vec(
        &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(expected.clone()),
    )
    .unwrap();
    owner.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    let mut targets = Vec::new();
    paths(
        &expected.terminal_contexts[1].tabs[0].root,
        &mut Vec::new(),
        &mut targets,
    );
    let mut receipts: Vec<(WorkbenchCommand, WorkbenchCommandResult)> = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        if step.kind >= 6 && receipts.is_empty() {
            continue;
        }
        let context_id = expected.terminal_contexts[1].terminal_context_id.clone();
        let tab_id = expected.terminal_contexts[1].tabs[0].tab_id.clone();
        let path = &targets[usize::from(step.target) % targets.len()];
        let root = node_at(&mut expected.terminal_contexts[1].tabs[0].root, path);
        let PanelNode::Split {
            split_id,
            children,
            ratio,
            ..
        } = root
        else {
            panic!()
        };
        let previous = *ratio;
        let requested = if step.kind == 1 {
            previous
        } else {
            WorkbenchSplitRatio::new([0.1, 0.3, 0.5, 0.7, 0.9][usize::from(step.weight) % 5])
                .unwrap()
        };
        let mut action = WorkbenchAction::SplitRatioSet {
            terminal_context_id: context_id,
            tab_id,
            split: ClientSplitIdentity {
                split_id: split_id.clone(),
                first_slot_id: children[0].slot_id.clone(),
                second_slot_id: children[1].slot_id.clone(),
            },
            previous_ratio: previous,
            ratio: requested,
        };
        let WorkbenchAction::SplitRatioSet {
            split,
            previous_ratio,
            ..
        } = &mut action
        else {
            panic!()
        };
        if step.kind == 3 {
            std::mem::swap(&mut split.first_slot_id, &mut split.second_slot_id);
        }
        if step.kind == 4 {
            *previous_ratio = if previous.value() == 0.1 {
                WorkbenchSplitRatio::HALF
            } else {
                WorkbenchSplitRatio::new(0.1).unwrap()
            };
        }
        let revision = expected.revision.value();
        let mut request = command(
            index as u128 + 1,
            if step.kind == 2 {
                revision + 1
            } else {
                revision
            },
            action,
        );
        if step.kind >= 6 && !receipts.is_empty() {
            let (old, result) = &receipts[usize::from(step.target) % receipts.len()];
            if step.kind == 6 {
                let wire = serde_json::to_vec(&WorkbenchRequest::Command(old.clone())).unwrap();
                let WorkbenchRequest::Command(replay) =
                    orchbee_workbench_protocol::parse_workbench_request(&wire).unwrap()
                else {
                    panic!()
                };
                if submit(&mut owner, &grant, replay) != *result {
                    return Err(format!("replay at {index}"));
                }
            } else {
                request.request_id = old.request_id.clone();
                request.expected_revision = WorkbenchRevision::new(u64::MAX);
                if !matches!(
                    submit(&mut owner, &grant, request).outcome,
                    WorkbenchCommandOutcome::Rejected {
                        reason: WorkbenchRejectionReason::RequestIdentityConflict,
                        ..
                    }
                ) {
                    return Err(format!("conflict at {index}"));
                }
            }
        } else {
            let result = if step.kind == 5 {
                let WorkbenchResponse::Command(result) = owner.execute(
                    None,
                    WorkbenchRequest::Command(request.clone())
                        .try_into()
                        .unwrap(),
                ) else {
                    panic!()
                };
                result
            } else {
                submit(&mut owner, &grant, request.clone())
            };
            if step.kind >= 2 {
                if !matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::Rejected { .. } | WorkbenchCommandOutcome::Denied {}
                ) {
                    return Err(format!("guard admitted at {index}"));
                }
            } else if requested == previous {
                if result.outcome
                    != (WorkbenchCommandOutcome::Unchanged {
                        revision: expected.revision,
                    })
                {
                    return Err(format!("no-op at {index}"));
                }
            } else {
                expected.revision = WorkbenchRevision::new(revision + 1);
                let PanelNode::Split { ratio, .. } =
                    node_at(&mut expected.terminal_contexts[1].tabs[0].root, path)
                else {
                    panic!()
                };
                *ratio = requested;
                if result.outcome
                    != (WorkbenchCommandOutcome::SplitRatioConfigured {
                        revision: expected.revision,
                    })
                {
                    return Err(format!("commit at {index}"));
                }
            }
            if step.kind != 5 {
                receipts.push((request, result));
            }
        }
        if owner.snapshot() != &expected || owner.last_identity != u128::MAX {
            return Err(format!("non-target field or counter changed at {index}"));
        }
        orchbee_workbench_protocol::parse_snapshot(
            &serde_json::to_vec(
                &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(
                    owner.snapshot().clone(),
                ),
            )
            .unwrap(),
        )
        .unwrap();
    }
    Ok(())
}
#[test]
fn nested_ratio_traces_preserve_other_fields_and_replay_exactly() {
    for seed in 1_u64..=48 {
        let mut random = seed;
        let depth = 1 + (seed % 3) as u8;
        let steps: Vec<_> = (0..64)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                Step {
                    kind: (random % 8) as u8,
                    target: random.to_le_bytes()[2],
                    weight: random.to_le_bytes()[3],
                }
            })
            .collect();
        if let Err(error) = run(depth, &steps) {
            let mut reduced = steps;
            let mut index = 0;
            while index < reduced.len() {
                let mut candidate = reduced.clone();
                candidate.remove(index);
                if run(depth, &candidate).is_err() {
                    reduced = candidate;
                    index = 0;
                } else {
                    index += 1;
                }
            }
            panic!("seed={seed}; depth={depth}; failure={error}; replay={reduced:?}");
        }
    }
}
