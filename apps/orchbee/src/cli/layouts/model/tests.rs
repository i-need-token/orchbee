use super::*;
use orchbee_workbench_protocol::{ClientSplitRatio, WorkbenchSplitId};

fn id(value: usize) -> String {
    format!("{value:032x}")
}
fn panel(next: &mut usize) -> PanelNode {
    *next += 1;
    serde_json::from_value(json!({"kind":"panel","panelId":id(*next),
        "childPresentations":[],"primaryPresentation":{"presentationId":id(*next),
        "label":"automatic","customLabel":"manual","binding":{"terminalId":id(*next),"workerId":id(999)}}})).unwrap()
}
fn branch(first: PanelNode, second: PanelNode, next: &mut usize) -> PanelNode {
    *next += 1;
    serde_json::from_value(
        json!({"kind":"split","splitId":id(*next),"orientation":"horizontal",
        "children":[{"slotId":id(*next*2),"node":first},{"slotId":id(*next*2+1),"node":second}]}),
    )
    .unwrap()
}
fn balanced(count: usize, next: &mut usize) -> PanelNode {
    if count == 1 {
        return panel(next);
    }
    let first = balanced(count / 2, next);
    let second = balanced(count - count / 2, next);
    branch(first, second, next)
}
fn observations(
    root: &PanelNode,
) -> (Vec<ClientSplitArea>, BTreeMap<TerminalPresentationId, Leaf>) {
    fn walk(node: &PanelNode, out: &mut Vec<ClientSplitArea>) {
        if let PanelNode::Split {
            split_id,
            children,
            orientation,
            ..
        } = node
        {
            out.push(ClientSplitArea {
                split_id: split_id.clone(),
                first_slot_id: children[0].slot_id.clone(),
                second_slot_id: children[1].slot_id.clone(),
                orientation: *orientation,
                ratio: ClientSplitRatio::new(0.3).unwrap(),
                area: None,
                divider: None,
            });
            for child in children.iter() {
                walk(&child.node, out);
            }
        }
    }
    let mut splits = Vec::new();
    walk(root, &mut splits);
    let facts = leaves(root)
        .unwrap()
        .iter()
        .filter(|(_, member)| member.binding.is_some())
        .map(|(_, member)| {
            (
                member.presentation_id.clone(),
                Leaf {
                    cwd: None,
                    launch: None,
                },
            )
        })
        .collect();
    (splits, facts)
}

#[test]
fn export_preserves_order_orientation_observed_ratio_manual_label_and_literal_argv() {
    let mut next = 0;
    let mut root = balanced(3, &mut next);
    let PanelNode::Split { children, .. } = &mut root else {
        panic!()
    };
    let PanelNode::Split { orientation, .. } = &mut children[1].node else {
        panic!()
    };
    *orientation = PanelOrientation::Vertical;
    let (splits, mut facts) = observations(&root);
    let first = leaves(&root).unwrap()[0].0.clone();
    facts.insert(
        leaves(&root).unwrap()[0].1.presentation_id.clone(),
        Leaf {
            cwd: Some("C:\\目录 空格".into()),
            launch: Some(
                LaunchSelection::new(
                    "C:\\Program Files\\shell.exe".into(),
                    vec!["".into(), "quoted \" text".into(), "中文".into()],
                )
                .unwrap(),
            ),
        },
    );
    let result = tree(&root, &splits, &facts).unwrap();
    assert_eq!(result["direction"], "right");
    assert_eq!(result["ratio"], 0.3);
    assert_eq!(result["second"]["direction"], "down");
    assert_eq!(
        result["first"],
        json!({"type":"panel","panelId":first,"rightClick":"orchbee","children":[],"primary":{"presentationId":first,"name":"automatic","start":true,"label":"manual",
        "cwd":"C:\\目录 空格","command":["C:\\Program Files\\shell.exe","","quoted \" text","中文"],"env":{}}})
    );
    assert!(result["second"]["first"]["primary"].get("cwd").is_none());
    assert!(
        result["second"]["first"]["primary"]
            .get("command")
            .is_none()
    );
    assert_eq!(result["second"]["first"]["primary"]["env"], json!({}));
}

#[test]
fn export_refuses_missing_extra_or_replaced_observations_instead_of_guessing() {
    let root = balanced(3, &mut 0);
    let (mut splits, mut facts) = observations(&root);
    let removed = splits.pop().unwrap();
    assert_eq!(tree(&root, &splits, &facts), Err("layout-changed"));
    splits.push(removed.clone());
    splits[0].first_slot_id = removed.first_slot_id.clone();
    assert_eq!(tree(&root, &splits, &facts), Err("layout-changed"));
    let (mut splits, _) = observations(&root);
    splits[1].split_id = WorkbenchSplitId::parse(&id(888)).unwrap();
    assert_eq!(tree(&root, &splits, &facts), Err("layout-changed"));
    splits[1] = splits[0].clone();
    assert_eq!(tree(&root, &splits, &facts), Err("layout-changed"));
    let (splits, _) = observations(&root);
    let removed = facts.pop_first().unwrap();
    assert_eq!(tree(&root, &splits, &facts), Err("layout-changed"));
    facts.insert(TerminalPresentationId::parse(&id(777)).unwrap(), removed.1);
    assert_eq!(tree(&root, &splits, &facts), Err("layout-changed"));
}

#[test]
fn portable_bounds_are_checked_before_terminal_reads_and_never_drop_child_presentations() {
    assert_eq!(leaves(&balanced(24, &mut 0)).unwrap().len(), 24);
    assert!(matches!(
        leaves(&balanced(25, &mut 0)),
        Err("layout-too-large")
    ));
    let mut next = 0;
    let mut root = panel(&mut next);
    for _ in 1..MAX_DEPTH {
        let other = panel(&mut next);
        root = branch(root, other, &mut next);
    }
    assert!(leaves(&root).is_ok());
    let other = panel(&mut next);
    root = branch(root, other, &mut next);
    assert!(matches!(leaves(&root), Err("layout-too-large")));
    let mut root = panel(&mut next);
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = &mut root
    else {
        panic!()
    };
    let mut child = primary_presentation.clone();
    child.presentation_id = TerminalPresentationId::parse(&id(9999)).unwrap();
    child.binding = None;
    child_presentations.push(child);
    assert_eq!(leaves(&root).unwrap().len(), 2);
    let (splits, facts) = observations(&root);
    let value = tree(&root, &splits, &facts).unwrap();
    assert_eq!(value["primary"]["start"], true);
    assert_eq!(value["children"][0]["start"], false);
    assert!(value["children"][0].get("command").is_none());
    let plan = crate::cli::layouts::template::parse(&serde_json::to_vec(&value).unwrap())
        .unwrap()
        .plan();
    assert!(plan.leaves[0].primary.start);
    assert!(!plan.leaves[0].children[0].start);
}
