//! Resolver verdicts over real validated snapshots: bounded ambiguity with
//! the true total, traversal-order candidates, per-resource ambiguity, and
//! filter-target hit/miss/ambiguity for every filter arm.

use super::*;
use orchbee_workbench_protocol::parse_snapshot;

mod topology;

fn hex(n: u8) -> String {
    format!("{n:032x}")
}

const WORKER: &str = "6f7263686265652d6c6f63616c2d7631";

/// Deliberately unsorted document order: candidate ordering must follow
/// canonical traversal order, never id sort.
const DUP_TABS: [u8; 9] = [0x29, 0x25, 0x27, 0x22, 0x28, 0x24, 0x26, 0x23, 0x21];

fn presentation(id: u8, terminal: Option<u8>) -> Value {
    json!({
        "presentationId": hex(id),
        "label": "p",
        "binding": terminal.map(|terminal| json!({
            "terminalId": hex(terminal),
            "workerId": WORKER,
        })),
    })
}

fn panel(id: u8, primary: u8, children: &[u8], terminal: Option<u8>) -> Value {
    json!({
        "kind": "panel",
        "panelId": hex(id),
        "primaryPresentation": presentation(primary, terminal),
        "childPresentations": children
            .iter()
            .map(|&child| presentation(child, None))
            .collect::<Vec<_>>(),
    })
}

fn tab(id: u8, label: &str, root: Value) -> Value {
    json!({
        "tabId": hex(id),
        "label": label,
        "pinned": false,
        "groupId": null,
        "root": root,
    })
}

/// Two same-label projects (`alpha`), each owning one same-label
/// workspace (`ws`) on its own context; nine same-label `dup` tabs in
/// scrambled id order under the standalone context; one `main` tab under
/// each workspace context (a split root under the first).
fn snapshot() -> Snapshot {
    let document = json!({
        "kind": "workbench.snapshot",
        "protocolVersion": 1,
        "revision": "9",
        "siteId": hex(0xaa),
        "navigation": {"projects":[{"projectId":hex(0x10), "label":"alpha"}, {"projectId":hex(0x11), "label":"alpha"}], "standaloneTerminalContextId":hex(0x01), "directories":[{
                        "directoryId": hex(0x30),
                        "workerId": WORKER,
                        "path": "/a",
                    }, {
                        "directoryId": hex(0x31),
                        "workerId": WORKER,
                        "path": "/b",
                    }], "workspaces":[{"workspaceId":hex(0x40), "label":"ws", "primaryDirectoryId":hex(0x30), "terminalContextId":hex(0x50), "projectId":hex(0x10)}, {"workspaceId":hex(0x41), "label":"ws", "primaryDirectoryId":hex(0x31), "terminalContextId":hex(0x51), "projectId":hex(0x11)}]},
        "terminalContexts": [
            {
                "terminalContextId": hex(0x01),
                "groups": [],
                "tabs": DUP_TABS
                    .iter()
                    .map(|&n| tab(n, "dup", panel(0x80 + n, 0xa0 + n, &[], None)))
                    .collect::<Vec<_>>(),
            },
            {
                "terminalContextId": hex(0x50),
                "groups": [],
                "tabs": [tab(0x52, "main", json!({
                    "kind": "split",
                    "splitId": hex(0x53),
                    "orientation": "horizontal",
                    "children": [
                        {"slotId": hex(0x54), "node": panel(0x55, 0x56, &[], None)},
                        {"slotId": hex(0x57), "node": panel(0x58, 0x59, &[0x5b], Some(0x5a))},
                    ],
                }))],
            },
            {
                "terminalContextId": hex(0x51),
                "groups": [],
                "tabs": [tab(0x62, "main", panel(0x63, 0x64, &[], None))],
            },
        ],
    });
    parse_snapshot(&serde_json::to_vec(&document).expect("fixture encodes"))
        .expect("fixture snapshot validates")
}

fn failure(command: &QueryCommand) -> Failure {
    project(command, &snapshot()).expect_err("resolver failure expected")
}

#[test]
fn split_ratio_projections_validate_without_widening_legacy_documents() {
    let mut state = snapshot();
    let PanelNode::Split { ratio, .. } = &mut state.terminal_contexts[1].tabs[0].root else {
        panic!("split fixture")
    };
    *ratio = orchbee_workbench_protocol::WorkbenchSplitRatio::new(0.9).unwrap();
    for (command, definition) in [
        (
            QueryCommand::ApiSnapshot,
            include_str!("../schemas/orchbee.api.snapshot.json"),
        ),
        (
            QueryCommand::TabGet(Target::Id(hex(0x52))),
            include_str!("../schemas/orchbee.tab.get.json"),
        ),
    ] {
        let source = json!({"kind":"workbench-snapshot","siteId":hex(0xaa),"revision":"9"});
        let mut document = json!({"schema":schema_id(&command),"version":schema_version(&command),
            "source":source,"result":project(&command, &state).unwrap().result()});
        let definition: Value = serde_json::from_str(definition).unwrap();
        let validator = jsonschema::validator_for(&definition).unwrap();
        validator.validate(&document).unwrap();
        document["version"] = 6.into();
        assert!(
            !validator.is_valid(&document),
            "legacy schema must reject ratio"
        );
        let mut legacy = document.clone();
        let legacy_root = if matches!(command, QueryCommand::ApiSnapshot) {
            &mut legacy["result"]["snapshot"]["terminalContexts"][1]["tabs"][0]["root"]
        } else {
            &mut legacy["result"]["tab"]["root"]
        };
        legacy_root.as_object_mut().unwrap().remove("ratio");
        validator.validate(&legacy).unwrap();
        document["version"] = 7.into();
        let root = if matches!(command, QueryCommand::ApiSnapshot) {
            &mut document["result"]["snapshot"]["terminalContexts"][1]["tabs"][0]["root"]
        } else {
            &mut document["result"]["tab"]["root"]
        };
        root["ratio"] = 0.91.into();
        assert!(!validator.is_valid(&document));
    }
}

#[test]
fn automatic_tab_queries_use_context_position_and_numeric_name_collisions_stay_ambiguous() {
    let mut state = snapshot();
    for context in &mut state.terminal_contexts {
        for tab in &mut context.tabs {
            tab.automatic_label = true;
        }
    }
    state.terminal_contexts[0].tabs[0].automatic_label = false;
    state.terminal_contexts[0].tabs[0].label = "2".into();
    let list = project(
        &QueryCommand::TabList {
            workspace: None,
            standalone: false,
        },
        &state,
    )
    .unwrap()
    .result();
    assert_eq!(list["tabs"][0]["label"], "2");
    assert_eq!(list["tabs"][1]["label"], "2");
    assert_eq!(list["tabs"][9]["label"], "1");
    for name in ["1", "2"] {
        let error = project(&QueryCommand::TabGet(Target::Label(name.into())), &state).unwrap_err();
        assert_eq!(error.kind, FailureKind::Ambiguous);
    }
    let id = state.terminal_contexts[0].tabs[1].tab_id.to_string();
    state.terminal_contexts[0].tabs.rotate_left(1);
    let get = project(&QueryCommand::TabGet(Target::Id(id.clone())), &state)
        .unwrap()
        .result();
    assert_eq!(get["tab"]["label"], "1");
    assert_eq!(get["tab"]["tabId"], id);
    let workspace = project(&QueryCommand::WorkspaceGet(Target::Id(hex(0x40))), &state)
        .unwrap()
        .result();
    assert_eq!(workspace["workspace"]["tabs"][0]["label"], "1");
    assert_eq!(
        project(&QueryCommand::TabGet(Target::Label("dup".into())), &state)
            .unwrap_err()
            .kind,
        FailureKind::NotFound
    );
}

fn details(failure: &Failure) -> &Map<String, Value> {
    failure.details.as_ref().expect("typed failure details")
}

fn candidates(failure: &Failure) -> Vec<String> {
    details(failure)["candidates"]
        .as_array()
        .expect("candidate list")
        .iter()
        .map(|id| id.as_str().expect("typed candidate id").to_owned())
        .collect()
}

#[test]
fn ambiguity_truncates_candidates_but_keeps_the_true_total() {
    let failure = failure(&QueryCommand::TabGet(Target::Label("dup".into())));
    assert_eq!(failure.kind, FailureKind::Ambiguous);
    let details = details(&failure);
    assert_eq!(details["resource"], json!("tab"));
    assert_eq!(details["target"], json!("dup"));
    assert_eq!(details["totalCount"], json!(DUP_TABS.len()));
    // At most the first eight candidate typed ids, in canonical traversal
    // order — context order then tab order — not sorted identity order.
    assert_eq!(
        candidates(&failure),
        DUP_TABS[..8].iter().map(|&n| hex(n)).collect::<Vec<_>>(),
    );
}

#[test]
fn label_lookup_is_ambiguous_per_resource() {
    for (command, resource, expected) in [
        (
            QueryCommand::ProjectGet(Target::Label("alpha".into())),
            "project",
            vec![hex(0x10), hex(0x11)],
        ),
        (
            QueryCommand::WorkspaceGet(Target::Label("ws".into())),
            "workspace",
            vec![hex(0x40), hex(0x41)],
        ),
    ] {
        let failure = failure(&command);
        assert_eq!(failure.kind, FailureKind::Ambiguous, "{resource}");
        let details = details(&failure);
        assert_eq!(details["resource"], json!(resource));
        assert_eq!(details["totalCount"], json!(expected.len()));
        assert_eq!(candidates(&failure), expected, "{resource}");
    }
}

#[test]
fn unmatched_targets_are_not_found_without_candidates() {
    let missing = hex(0x7f);
    for (command, resource, target) in [
        (
            QueryCommand::TabGet(Target::Label("ghost".into())),
            "tab",
            "ghost".to_owned(),
        ),
        (
            QueryCommand::TabGet(Target::Id(missing.clone())),
            "tab",
            missing.clone(),
        ),
        (
            QueryCommand::WorkspaceGet(Target::Label("ghost".into())),
            "workspace",
            "ghost".to_owned(),
        ),
    ] {
        let failure = failure(&command);
        assert_eq!(failure.kind, FailureKind::NotFound, "{resource}");
        let details = details(&failure);
        assert_eq!(details["resource"], json!(resource));
        assert_eq!(details["target"], json!(target));
        assert!(details.get("candidates").is_none(), "{resource}");
    }
}

#[test]
fn workspace_list_uses_global_order_and_preserves_filtered_ownership() {
    let mut source = snapshot();
    source.navigation.workspace_order = source
        .navigation
        .workspaces
        .iter()
        .rev()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    let Projection::WorkspaceList(rows) =
        project(&QueryCommand::WorkspaceList { project: None }, &source).unwrap()
    else {
        panic!("list")
    };
    assert_eq!(
        rows.iter()
            .map(|r| r.workspace_id.as_str())
            .collect::<Vec<_>>(),
        vec![hex(0x41), hex(0x40)]
    );
    assert_eq!(rows[0].project_id, Some(hex(0x11)));
    let Projection::WorkspaceList(rows) = project(
        &QueryCommand::WorkspaceList {
            project: Some(Target::Id(hex(0x10))),
        },
        &source,
    )
    .unwrap() else {
        panic!("list")
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].workspace_id, hex(0x40));
}

#[test]
fn project_filter_hit_miss_and_ambiguity() {
    // Hit: only the resolved project's workspaces survive the filter.
    let projection = project(
        &QueryCommand::WorkspaceList {
            project: Some(Target::Id(hex(0x10))),
        },
        &snapshot(),
    )
    .expect("resolved project filter");
    let Projection::WorkspaceList(rows) = projection else {
        panic!("workspace list projection");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].workspace_id, hex(0x40));
    assert_eq!(rows[0].project_id, Some(hex(0x10)));
    assert_eq!(rows[0].terminal_context_id, hex(0x50));

    // Ambiguous filter target: the list itself is never built.
    let ambiguous = failure(&QueryCommand::WorkspaceList {
        project: Some(Target::Label("alpha".into())),
    });
    assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
    assert_eq!(details(&ambiguous)["resource"], json!("project"));
    assert_eq!(candidates(&ambiguous), vec![hex(0x10), hex(0x11)]);

    // Miss: a typed not-found, never an empty list.
    let missed = failure(&QueryCommand::WorkspaceList {
        project: Some(Target::Label("ghost".into())),
    });
    assert_eq!(missed.kind, FailureKind::NotFound);
    assert_eq!(details(&missed)["resource"], json!("project"));
    assert_eq!(details(&missed)["target"], json!("ghost"));
}

#[test]
fn workspace_filter_hit_miss_and_ambiguity() {
    // Hit: tabs from exactly the resolved workspace's own context, each
    // carrying the workspace context reference.
    let projection = project(
        &QueryCommand::TabList {
            workspace: Some(Target::Id(hex(0x41))),
            standalone: false,
        },
        &snapshot(),
    )
    .expect("resolved workspace filter");
    let Projection::TabList(rows) = projection else {
        panic!("tab list projection");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tab_id, hex(0x62));
    assert!(!rows[0].context.standalone);
    assert_eq!(rows[0].context.terminal_context_id, hex(0x51));
    assert_eq!(rows[0].context.workspace_id, Some(hex(0x41)));

    // Ambiguous: two workspaces share the label across projects.
    let ambiguous = failure(&QueryCommand::TabList {
        workspace: Some(Target::Label("ws".into())),
        standalone: false,
    });
    assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
    assert_eq!(details(&ambiguous)["resource"], json!("workspace"));
    assert_eq!(candidates(&ambiguous), vec![hex(0x40), hex(0x41)]);

    // Miss.
    let missed = failure(&QueryCommand::TabList {
        workspace: Some(Target::Label("ghost".into())),
        standalone: false,
    });
    assert_eq!(missed.kind, FailureKind::NotFound);
    assert_eq!(details(&missed)["resource"], json!("workspace"));
}

#[test]
fn tab_filter_hit_miss_and_ambiguity() {
    // Hit: panels under exactly the resolved tab, in traversal order.
    let projection = project(
        &QueryCommand::PanelList {
            tab: Some(Target::Id(hex(0x52))),
            workspace: None,
        },
        &snapshot(),
    )
    .expect("resolved tab filter");
    let Projection::PanelList(rows) = projection else {
        panic!("panel list projection");
    };
    assert_eq!(
        rows.iter()
            .map(|row| row.panel_id.clone())
            .collect::<Vec<_>>(),
        vec![hex(0x55), hex(0x58)],
    );
    assert!(rows.iter().all(|row| row.tab_id == hex(0x52)));
    assert!(rows.iter().all(|row| row.terminal_context_id == hex(0x50)));

    // Ambiguous: "main" names a tab in each workspace context.
    let ambiguous = failure(&QueryCommand::PanelList {
        tab: Some(Target::Label("main".into())),
        workspace: None,
    });
    assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
    assert_eq!(details(&ambiguous)["resource"], json!("tab"));
    assert_eq!(candidates(&ambiguous), vec![hex(0x52), hex(0x62)]);

    // Miss.
    let missed = failure(&QueryCommand::PanelList {
        tab: Some(Target::Label("ghost".into())),
        workspace: None,
    });
    assert_eq!(missed.kind, FailureKind::NotFound);
    assert_eq!(details(&missed)["resource"], json!("tab"));
}

/// A 32-hex id whose high half and trailing hex tail are controlled —
/// the selector surface is the tail, so `tid(0, 0xaa)` and
/// `tid(1, 0xaa)` share tail `aa` while staying distinct identities.
fn tid(high: u64, tail: u64) -> String {
    format!("{high:016x}{tail:016x}")
}

/// Selector fixture: projects `…00aa`/`…10aa`/`…00bb`, workspaces
/// `…00cc`/`…10cc`/`…00dd`, standalone tabs `…00ee`/`…10ee`, panels
/// `…00ff`/`…10ff` (bound terminals `…00210`/`…11210`), `…00f1`, a
/// workspace tab `…00e1` whose split holds `…00f2`/`…10f2`, and a second
/// workspace tab `…00e3` whose right-deep split holds nine panels ending
/// in `42` plus one panel `…00f4`.
fn selector_snapshot() -> Snapshot {
    let leaf = |panel: String, pres: String, terminal: Option<String>| {
        json!({
            "kind": "panel",
            "panelId": panel,
            "primaryPresentation": {
                "presentationId": pres,
                "label": "p",
                "binding": terminal.map(|terminal| json!({
                    "terminalId": terminal,
                    "workerId": WORKER,
                })),
            },
            "childPresentations": [],
        })
    };
    fn deep(tails: &[u64], seed: &mut u64) -> Value {
        let node = |panel: u64, pres: u64| {
            json!({
                "kind": "panel",
                "panelId": tid(0, panel),
                "primaryPresentation": {
                    "presentationId": tid(0xa, pres),
                    "label": "p",
                    "binding": null,
                },
                "childPresentations": [],
            })
        };
        *seed += 1;
        let slot = *seed;
        if tails.len() == 1 {
            return node(tails[0], slot + 0x100);
        }
        json!({
            "kind": "split",
            "splitId": tid(9, slot),
            "orientation": "horizontal",
            "children": [
                {"slotId": tid(8, slot), "node": node(tails[0], slot + 0x100)},
                {"slotId": tid(8, slot + 0x100), "node": deep(&tails[1..], seed)},
            ],
        })
    }
    let mut seed = 0_u64;
    let document = json!({
        "kind": "workbench.snapshot",
        "protocolVersion": 1,
        "revision": "9",
        "siteId": tid(0, 0xaa00),
        "navigation": {"projects":[{"projectId":tid(0, 0xaa), "label":"p-a"}, {"projectId":tid(0, 0xbb), "label":"face"}, {"projectId":tid(1, 0xaa), "label":"p-b"}], "standaloneTerminalContextId":tid(0, 0x01), "directories":[], "workspaces":[{"workspaceId":tid(0, 0xcc), "label":"w", "primaryDirectoryId":null, "terminalContextId":tid(0, 0x50), "projectId":tid(0, 0xaa)}, {"workspaceId":tid(0, 0xdd), "label":"w3", "primaryDirectoryId":null, "terminalContextId":tid(0, 0x60), "projectId":tid(0, 0xbb)}, {"workspaceId":tid(1, 0xcc), "label":"w", "primaryDirectoryId":null, "terminalContextId":tid(0, 0x51), "projectId":tid(1, 0xaa)}]},
        "terminalContexts": [
            {
                "terminalContextId": tid(0, 0x01),
                "groups": [],
                "tabs": [
                    {
                        "tabId": tid(0, 0xee),
                        "label": "t",
                        "pinned": false,
                        "groupId": null,
                        "root": {
                            "kind": "split",
                            "splitId": tid(9, 0x90),
                            "orientation": "horizontal",
                            "children": [
                                {"slotId": tid(8, 0x90), "node": leaf(tid(0, 0xff), tid(0xa, 0x90), Some(tid(0, 0x210)))},
                                {"slotId": tid(8, 0x91), "node": leaf(tid(1, 0xff), tid(0xa, 0x91), Some(tid(1, 0x1210)))},
                            ],
                        },
                    },
                    {
                        "tabId": tid(1, 0xee),
                        "label": "t",
                        "pinned": false,
                        "groupId": null,
                        "root": leaf(tid(0, 0xf1), tid(0xa, 0xf1), None),
                    },
                ],
            },
            {
                "terminalContextId": tid(0, 0x50),
                "groups": [],
                "tabs": [{
                    "tabId": tid(0, 0xe8),
                    "label": "w1",
                    "pinned": false,
                    "groupId": null,
                    "root": leaf(tid(0, 0xf8), tid(0xa, 0xf8), None),
                }],
            },
            {
                "terminalContextId": tid(0, 0x51),
                "groups": [],
                "tabs": [{
                    "tabId": tid(0, 0xe9),
                    "label": "w2",
                    "pinned": false,
                    "groupId": null,
                    "root": leaf(tid(0, 0xf9), tid(0xa, 0xf9), None),
                }],
            },
            {
                "terminalContextId": tid(0, 0x60),
                "groups": [],
                "tabs": [
                    {
                        "tabId": tid(0, 0xe1),
                        "label": "t3",
                        "pinned": false,
                        "groupId": null,
                        "root": {
                            "kind": "split",
                            "splitId": tid(9, 0x92),
                            "orientation": "horizontal",
                            "children": [
                                {"slotId": tid(8, 0x92), "node": leaf(tid(0, 0xf2), tid(0xa, 0xf2), None)},
                                {"slotId": tid(8, 0x93), "node": leaf(tid(1, 0xf2), tid(0xa, 0xf3), None)},
                            ],
                        },
                    },
                    {
                        "tabId": tid(0, 0xe3),
                        "label": "t4",
                        "pinned": false,
                        "groupId": null,
                        "root": {
                            "kind": "split",
                            "splitId": tid(9, 0x94),
                            "orientation": "horizontal",
                            "children": [
                                {"slotId": tid(8, 0x94), "node": leaf(tid(0, 0xf4), tid(0xa, 0xf4), None)},
                                {"slotId": tid(8, 0x95), "node": deep(&[0x42, 0x142, 0x242, 0x342, 0x442, 0x542, 0x642, 0x742, 0x842], &mut seed)},
                            ],
                        },
                    },
                ],
            },
        ],
    });
    parse_snapshot(&serde_json::to_vec(&document).expect("fixture encodes"))
        .expect("selector fixture snapshot validates")
}

#[test]
fn selectors_resolve_unique_tails_to_full_ids() {
    let snapshot = selector_snapshot();
    // One unique-tail hit per selector kind; the resolved document carries
    // the full canonical id, never the selector token.
    let projection = project(
        &QueryCommand::ProjectGet(Target::Selector("@project:bb".into())),
        &snapshot,
    )
    .expect("project selector");
    let Projection::ProjectGet(document) = projection else {
        panic!("project get projection");
    };
    assert_eq!(document.row.project_id, tid(0, 0xbb));

    let projection = project(
        &QueryCommand::WorkspaceGet(Target::Selector("@workspace:dd".into())),
        &snapshot,
    )
    .expect("workspace selector");
    let Projection::WorkspaceGet(document) = projection else {
        panic!("workspace get projection");
    };
    assert_eq!(document.row.workspace_id, tid(0, 0xdd));

    let projection = project(
        &QueryCommand::TabGet(Target::Selector("@tab:e1".into())),
        &snapshot,
    )
    .expect("tab selector");
    let Projection::TabGet(document) = projection else {
        panic!("tab get projection");
    };
    assert_eq!(document.row.tab_id, tid(0, 0xe1));

    let projection = project(
        &QueryCommand::PanelGet(Target::Selector("@panel:f4".into())),
        &snapshot,
    )
    .expect("panel selector");
    let Projection::PanelGet(document) = projection else {
        panic!("panel get projection");
    };
    assert_eq!(document.row.panel_id, tid(0, 0xf4));

    // Terminals resolve over the snapshot's complete bound set.
    let row = terminal_select(&snapshot, &Target::Selector("@terminal:00210".into()))
        .expect("terminal selector");
    assert_eq!(row.terminal_id, tid(0, 0x210));
}

#[test]
fn selectors_match_inside_their_own_kind_only() {
    let snapshot = selector_snapshot();
    // Tail `aa` exists on two projects but no workspace — a foreign-kind
    // selector is a miss, never a cross-kind hit.
    let failure = project(
        &QueryCommand::WorkspaceGet(Target::Selector("@workspace:aa".into())),
        &snapshot,
    )
    .expect_err("cross-kind tail must not resolve");
    assert_eq!(failure.kind, FailureKind::NotFound);
    assert_eq!(details(&failure)["resource"], json!("workspace"));
    assert_eq!(details(&failure)["target"], json!("@workspace:aa"));
    // The same tail the terminals carry (`210`) is not a panel tail.
    let failure = project(
        &QueryCommand::PanelGet(Target::Selector("@panel:210".into())),
        &snapshot,
    )
    .expect_err("terminal tails are not panel candidates");
    assert_eq!(failure.kind, FailureKind::NotFound);
    // A hex-shaped label still resolves as a label; a selector with the
    // same spelling only looks at identifier tails.
    let projection = project(
        &QueryCommand::ProjectGet(Target::Label("face".into())),
        &snapshot,
    )
    .expect("hex-looking label");
    let Projection::ProjectGet(document) = projection else {
        panic!("project get projection");
    };
    assert_eq!(document.row.project_id, tid(0, 0xbb));
    let failure = project(
        &QueryCommand::ProjectGet(Target::Selector("@project:face".into())),
        &snapshot,
    )
    .expect_err("no project id ends in `face`");
    assert_eq!(failure.kind, FailureKind::NotFound);
}

#[test]
fn selector_ambiguity_reports_bounded_candidates_and_the_true_total() {
    let snapshot = selector_snapshot();
    // Two-member collisions per kind, in canonical traversal order.
    for (command, resource, expected) in [
        (
            QueryCommand::ProjectGet(Target::Selector("@project:aa".into())),
            "project",
            vec![tid(0, 0xaa), tid(1, 0xaa)],
        ),
        (
            QueryCommand::WorkspaceGet(Target::Selector("@workspace:cc".into())),
            "workspace",
            vec![tid(0, 0xcc), tid(1, 0xcc)],
        ),
        (
            QueryCommand::TabGet(Target::Selector("@tab:ee".into())),
            "tab",
            vec![tid(0, 0xee), tid(1, 0xee)],
        ),
        (
            QueryCommand::PanelGet(Target::Selector("@panel:ff".into())),
            "panel",
            vec![tid(0, 0xff), tid(1, 0xff)],
        ),
    ] {
        let failure = project(&command, &snapshot).expect_err("colliding selector tail");
        assert_eq!(failure.kind, FailureKind::Ambiguous, "{resource}");
        assert_eq!(details(&failure)["resource"], json!(resource));
        assert_eq!(details(&failure)["totalCount"], json!(2));
        assert_eq!(candidates(&failure), expected, "{resource}");
    }
    // Terminals share the same rule over bound rows.
    let failure = terminal_select(&snapshot, &Target::Selector("@terminal:210".into()))
        .expect_err("colliding terminal tail");
    assert_eq!(failure.kind, FailureKind::Ambiguous);
    assert_eq!(candidates(&failure), vec![tid(0, 0x210), tid(1, 0x1210)],);
    // Nine panels end in `42`: the envelope caps the candidate list at
    // eight while the total stays truthful.
    let failure = project(
        &QueryCommand::PanelGet(Target::Selector("@panel:42".into())),
        &snapshot,
    )
    .expect_err("nine-way tail collision");
    assert_eq!(failure.kind, FailureKind::Ambiguous);
    assert_eq!(details(&failure)["totalCount"], json!(9));
    assert_eq!(
        candidates(&failure),
        [0x42, 0x142, 0x242, 0x342, 0x442, 0x542, 0x642, 0x742]
            .iter()
            .map(|&tail| tid(0, tail))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn selector_misses_are_not_found_without_candidates() {
    let snapshot = selector_snapshot();
    for (command, resource, target) in [
        (
            QueryCommand::TabGet(Target::Selector("@tab:999".into())),
            "tab",
            "@tab:999",
        ),
        (
            QueryCommand::PanelGet(Target::Selector("@panel:999".into())),
            "panel",
            "@panel:999",
        ),
    ] {
        let failure = project(&command, &snapshot).expect_err("selector miss");
        assert_eq!(failure.kind, FailureKind::NotFound, "{resource}");
        assert_eq!(details(&failure)["resource"], json!(resource));
        assert_eq!(details(&failure)["target"], json!(target));
        assert!(details(&failure).get("candidates").is_none());
    }
    let failure = terminal_select(&snapshot, &Target::Selector("@terminal:999".into()))
        .expect_err("terminal selector miss");
    assert_eq!(failure.kind, FailureKind::NotFound);
    // A full 32-hex id is still an exact identity lookup, unchanged.
    let projection = project(&QueryCommand::TabGet(Target::Id(tid(0, 0xe1))), &snapshot)
        .expect("full id resolves");
    let Projection::TabGet(document) = projection else {
        panic!("tab get projection");
    };
    assert_eq!(document.row.tab_id, tid(0, 0xe1));
}

#[test]
fn selector_filters_share_operand_semantics() {
    let snapshot = selector_snapshot();
    // `--project` on `workspace list` resolves the same suffix grammar.
    let projection = project(
        &QueryCommand::WorkspaceList {
            project: Some(Target::Selector("@project:bb".into())),
        },
        &snapshot,
    )
    .expect("filtered workspace list");
    let Projection::WorkspaceList(rows) = projection else {
        panic!("workspace list projection");
    };
    assert_eq!(
        rows.iter()
            .map(|row| row.workspace_id.clone())
            .collect::<Vec<_>>(),
        vec![tid(0, 0xdd)],
    );
    // `--workspace` on `tab list` and `--tab` on `panel list` likewise.
    let projection = project(
        &QueryCommand::TabList {
            workspace: Some(Target::Selector("@workspace:dd".into())),
            standalone: false,
        },
        &snapshot,
    )
    .expect("filtered tab list");
    let Projection::TabList(rows) = projection else {
        panic!("tab list projection");
    };
    assert_eq!(
        rows.iter()
            .map(|row| row.tab_id.clone())
            .collect::<Vec<_>>(),
        vec![tid(0, 0xe1), tid(0, 0xe3)],
    );
    let projection = project(
        &QueryCommand::PanelList {
            tab: Some(Target::Selector("@tab:e1".into())),
            workspace: None,
        },
        &snapshot,
    )
    .expect("filtered panel list");
    let Projection::PanelList(rows) = projection else {
        panic!("panel list projection");
    };
    assert_eq!(
        rows.iter()
            .map(|row| row.panel_id.clone())
            .collect::<Vec<_>>(),
        vec![tid(0, 0xf2), tid(1, 0xf2)],
    );
}

#[test]
fn unfiltered_lists_follow_canonical_traversal_order() {
    // Workspaces flatten across projects in project/workspace order.
    let projection = project(&QueryCommand::WorkspaceList { project: None }, &snapshot())
        .expect("workspace list");
    let Projection::WorkspaceList(rows) = projection else {
        panic!("workspace list projection");
    };
    assert_eq!(
        rows.iter()
            .map(|row| row.workspace_id.clone())
            .collect::<Vec<_>>(),
        vec![hex(0x40), hex(0x41)],
    );

    // Tabs concatenate contexts in context order, then document order:
    // the scrambled standalone ids lead, then each workspace's tab.
    let projection = project(
        &QueryCommand::TabList {
            workspace: None,
            standalone: false,
        },
        &snapshot(),
    )
    .expect("tab list");
    let Projection::TabList(rows) = projection else {
        panic!("tab list projection");
    };
    assert_eq!(rows.len(), DUP_TABS.len() + 2);
    assert_eq!(
        rows.iter()
            .map(|row| row.tab_id.clone())
            .collect::<Vec<_>>(),
        DUP_TABS
            .iter()
            .chain([0x52, 0x62].iter())
            .map(|&n| hex(n))
            .collect::<Vec<_>>(),
    );
    // Context references pin the owning workspace only where one exists.
    assert!(
        rows[..9]
            .iter()
            .all(|row| row.context.standalone && row.context.workspace_id.is_none())
    );
    assert_eq!(rows[9].context.workspace_id, Some(hex(0x40)));
    assert_eq!(rows[10].context.workspace_id, Some(hex(0x41)));

    // `--standalone` selects exactly the standalone context's tabs.
    let projection = project(
        &QueryCommand::TabList {
            workspace: None,
            standalone: true,
        },
        &snapshot(),
    )
    .expect("standalone tab list");
    let Projection::TabList(rows) = projection else {
        panic!("tab list projection");
    };
    assert_eq!(rows.len(), DUP_TABS.len());
    assert!(rows.iter().all(|row| row.context.standalone));
}

/// `terminal read` answer bounds over decoder-legal frame states: every
/// constructed frame honors the wire invariants the OBTF decoder enforces
/// (row index < `rows`, `cells == columns`, adjacent wrap-flag agreement),
/// so each case is a state a real record can deliver. The same shapes
/// cross the wire in the scripted-peer witnesses in `tests/cli_query.rs`.
#[cfg(any(unix, windows))]
mod terminal_read {
    use super::super::{Projection, ReadPresentation, TerminalRow, terminal_read_doc};
    use crate::cli::{output::render_text, present};
    use crate::invocation::{QueryCommand, Target};
    use crate::localization::Locale;
    use orchbee_terminal_protocol::{
        AttachmentId, HostEpoch, ProtocolErrorCode, TerminalFrameRoute, TerminalInteraction,
        TerminalPixelGeometry, TerminalTextCell, TerminalTextCursor, TerminalTextFrame,
        TerminalTextRow, TerminalTextStyle, decode_terminal_text_frame,
    };
    use std::num::NonZeroU64;
    use std::sync::Arc;

    const COLUMNS: u16 = 64;

    fn hex(n: u8) -> String {
        format!("{n:032x}")
    }

    fn terminal() -> TerminalRow {
        TerminalRow {
            terminal_id: hex(0x01),
            worker_id: "6f7263686265652d6c6f63616c2d7631".to_owned(),
            presentation_id: hex(0x20),
            panel_id: hex(0x21),
            tab_id: hex(0x22),
            terminal_context_id: hex(0x50),
        }
    }

    fn cell(text: &Arc<str>, width: u8) -> TerminalTextCell {
        TerminalTextCell {
            text: text.clone(),
            style: TerminalTextStyle {
                foreground: 0,
                background: 0,
                underline_color: 0,
                flags: 0,
                underline: 0,
            },
            width,
        }
    }

    fn frame(rows: Vec<TerminalTextRow>) -> TerminalTextFrame {
        frame_with_columns(rows, COLUMNS)
    }

    fn frame_with_columns(rows: Vec<TerminalTextRow>, columns: u16) -> TerminalTextFrame {
        TerminalTextFrame {
            route: TerminalFrameRoute {
                attachment_id: AttachmentId::from_bytes([7; 16]).expect("attachment"),
                host_epoch: HostEpoch::from_bytes([9; 16]).expect("epoch"),
                sequence: NonZeroU64::new(1).expect("sequence"),
                generation: NonZeroU64::new(1).expect("generation"),
                revision: 1,
                base_revision: None,
            },
            geometry: TerminalPixelGeometry {
                cell_pixel_height: 1,
                cell_pixel_width: 8,
                columns,
                rows: u16::try_from(rows.len()).expect("row count fits u16"),
            },
            cursor: TerminalTextCursor {
                column: 0,
                row: 0,
                visible: true,
            },
            alternate_screen: false,
            interaction: TerminalInteraction::default(),
            foreground: 0,
            background: 0,
            theme_revision: 1,
            scrollback_rows: 0,
            progress: None,
            graphics_present: None,
            command_completion: None,
            command_observation: None,
            title: None,
            current_directory: None,
            rows,
        }
    }

    /// Minimal OBTF snapshot serializer for real-decoder round-trip
    /// checks: cell texts are interned in first-use order, every cell
    /// shares style 0, and a cell's width code goes into the wire
    /// cell-flags byte. Rows are serialized with their actual cell
    /// count, so an under-width row fails the decoder's
    /// `cells_count == columns` rule instead of being padded silently.
    fn wire(frame: &TerminalTextFrame) -> Vec<u8> {
        let mut strings: Vec<Arc<str>> = Vec::new();
        for row in &frame.rows {
            for cell in &row.cells {
                if !strings.iter().any(|text| text == &cell.text) {
                    strings.push(cell.text.clone());
                }
            }
        }
        let route = &frame.route;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"OBTF");
        bytes.extend_from_slice(&[0, 0]); // format major/minor
        bytes.push(1); // snapshot kind: no base revision
        bytes.push(0); // routing flags
        bytes.extend_from_slice(&[0; 4]); // total length, patched below
        bytes.extend_from_slice(&160_u16.to_le_bytes()); // header bytes
        bytes.extend_from_slice(&[0; 2]); // reserved
        bytes.extend_from_slice(route.host_epoch.as_bytes());
        bytes.extend_from_slice(route.attachment_id.as_bytes());
        bytes.extend_from_slice(&route.sequence.get().to_le_bytes());
        bytes.extend_from_slice(&u64::MAX.to_le_bytes()); // base revision: none
        bytes.extend_from_slice(&route.revision.to_le_bytes());
        bytes.extend_from_slice(&route.generation.get().to_le_bytes());
        bytes.extend_from_slice(&frame.scrollback_rows.to_le_bytes());
        bytes.extend_from_slice(&frame.geometry.columns.to_le_bytes());
        bytes.extend_from_slice(&frame.geometry.rows.to_le_bytes());
        bytes.extend_from_slice(&frame.cursor.column.to_le_bytes());
        bytes.extend_from_slice(&frame.cursor.row.to_le_bytes());
        bytes.push(1); // cursor shape
        bytes.push(u8::from(frame.cursor.visible)); // cursor flags
        bytes.push(if frame.alternate_screen { 2 } else { 1 }); // screen
        bytes.push(0); // reserved
        bytes.extend_from_slice(&0_u32.to_le_bytes()); // interaction
        bytes.extend_from_slice(&frame.foreground.to_le_bytes());
        bytes.extend_from_slice(&frame.background.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes()); // cursor color
        bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // title: none
        bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // directory: none
        bytes.extend_from_slice(&(strings.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes()); // one style entry
        bytes.extend_from_slice(&frame.geometry.rows.to_le_bytes()); // all rows
        bytes.extend_from_slice(&[0; 2]); // no extensions
        bytes.extend_from_slice(&[0; 2]); // reserved tables
        bytes.extend_from_slice(&frame.geometry.cell_pixel_width.to_le_bytes());
        bytes.extend_from_slice(&frame.geometry.cell_pixel_height.to_le_bytes());
        bytes.extend_from_slice(
            &(u32::from(frame.geometry.columns) * u32::from(frame.geometry.rows)).to_le_bytes(),
        );
        bytes.push(3); // OBTF version byte checked by the route inspector
        bytes.extend_from_slice(&frame.theme_revision.to_le_bytes());
        bytes.extend_from_slice(&[0; 7]); // trailing reserved header
        for text in &strings {
            bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
        // One style entry: default colors, no link, no flags or underline.
        bytes.extend_from_slice(&[0; 12]); // foreground/background/underline
        bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // link: none
        bytes.extend_from_slice(&[0; 4]); // flags, underline, reserved
        for row in &frame.rows {
            bytes.extend_from_slice(&row.index.to_le_bytes());
            bytes.push(row.flags);
            bytes.push(0); // prompt: none
            bytes.extend_from_slice(&(row.cells.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&[0; 2]); // reserved
            for cell in &row.cells {
                let string = strings
                    .iter()
                    .position(|text| text == &cell.text)
                    .expect("interned") as u32;
                bytes.extend_from_slice(&string.to_le_bytes());
                bytes.extend_from_slice(&0_u16.to_le_bytes()); // style 0
                bytes.push(cell.width); // cell flags carry the width code
                bytes.push(0); // reserved
            }
        }
        let total = u32::try_from(bytes.len()).expect("frame length fits u32");
        bytes[8..12].copy_from_slice(&total.to_le_bytes());
        bytes
    }

    /// One row of 64 cells that all share a single string.
    fn shared_row(index: u16, flags: u8, text: &Arc<str>) -> TerminalTextRow {
        TerminalTextRow {
            prompt_class: 0,
            index,
            flags,
            cells: vec![cell(text, 0); usize::from(COLUMNS)],
        }
    }

    /// The exact-boundary frame: 64 cells sharing one 16 KiB string join
    /// into a 1,048,576-char text whose JSON encoding debits 1,048,578
    /// bytes. Three such rows debit 3,145,734; row 3 keeps 63 shared
    /// cells plus a mixed tail (`中`×4000 = 12,000 encoded bytes, ESC×500
    /// = 3,000, `x`×1376 = 1,376 — 16,376 total), encoding to exactly
    /// 1,048,570 and landing the sum on READ_TEXT_BYTES. This pins the
    /// `>`-not-`>=` debit check: a boundary row must still be emitted.
    #[test]
    fn exact_budget_emits_every_row() {
        let full: Arc<str> = Arc::from("x".repeat(16384).as_str());
        let tail_text = format!(
            "{}{}{}",
            "中".repeat(4000),
            "\u{1b}".repeat(500),
            "x".repeat(1376)
        );
        let tail: Arc<str> = Arc::from(tail_text.as_str());
        let mut last_cells = vec![cell(&full, 0); 63];
        last_cells.push(cell(&tail, 0));
        let frame = frame(vec![
            shared_row(0, 0, &full),
            shared_row(1, 0, &full),
            shared_row(2, 0, &full),
            TerminalTextRow {
                prompt_class: 0,
                index: 3,
                flags: 0,
                cells: last_cells,
            },
        ]);
        let doc =
            terminal_read_doc(terminal(), &frame, ReadPresentation::default()).expect("document");
        assert_eq!(doc.row_count, 4);
        assert_eq!(doc.rows.len(), 4);
        assert_eq!(doc.omitted_rows, 0);
        let full_text = "x".repeat(usize::from(COLUMNS) * 16384);
        for (index, row) in doc.rows[..3].iter().enumerate() {
            assert_eq!(row.index, index as u16);
            assert_eq!(row.text, full_text);
        }
        let mut last_text = "x".repeat(63 * 16384);
        last_text.push_str(&tail);
        assert_eq!(doc.rows[3].index, 3);
        assert_eq!(doc.rows[3].text, last_text);
        let content = &Projection::TerminalRead(doc).result()["content"];
        assert_eq!(content["truncated"], false);
        assert_eq!(content["omittedRows"], 0);
        assert_eq!(content["rowCount"], 4);
    }

    /// One encoded byte past the remaining budget omits the whole row:
    /// the mixed tail's `x` run grows to 1,377 so row 3 encodes to
    /// 1,048,571 against 1,048,570 left. The kept rows carry a wrap pair
    /// — bit 0 `intoNext` on row 0 meets bit 1 `fromPrevious` on row 1 —
    /// so a flag-swap mutation fails here too.
    #[test]
    fn one_byte_over_budget_omits_the_row() {
        let full: Arc<str> = Arc::from("x".repeat(16384).as_str());
        let tail: Arc<str> = Arc::from(
            format!(
                "{}{}{}",
                "中".repeat(4000),
                "\u{1b}".repeat(500),
                "x".repeat(1377)
            )
            .as_str(),
        );
        let mut last_cells = vec![cell(&full, 0); 63];
        last_cells.push(cell(&tail, 0));
        let frame = frame(vec![
            shared_row(0, 1, &full),
            shared_row(1, 2, &full),
            shared_row(2, 0, &full),
            TerminalTextRow {
                prompt_class: 0,
                index: 3,
                flags: 0,
                cells: last_cells,
            },
        ]);
        let doc =
            terminal_read_doc(terminal(), &frame, ReadPresentation::default()).expect("document");
        assert_eq!(doc.row_count, 4);
        assert_eq!(doc.rows.len(), 3);
        assert_eq!(doc.omitted_rows, 1);
        assert!(doc.rows[0].into_next && !doc.rows[0].from_previous);
        assert!(!doc.rows[1].into_next && doc.rows[1].from_previous);
        assert!(!doc.rows[2].into_next && !doc.rows[2].from_previous);
        let content = &Projection::TerminalRead(doc).result()["content"];
        assert_eq!(content["truncated"], true);
        assert_eq!(content["omittedRows"], 1);
    }

    /// The decoder maximum of 4,096 rows with a first row already over
    /// budget: 64 cells sharing a 16 KiB control-string join to
    /// 1,048,576 chars that JSON-escape to 6,291,458 bytes, so every row
    /// is omitted and the count stays honest at the schema's upper bound.
    /// A miscounted `omitted_rows` or a budget that only checked `text`
    /// length before escaping fails here.
    #[test]
    fn over_budget_first_row_omits_all_rows() {
        let control: Arc<str> = Arc::from("\u{1}".repeat(16384).as_str());
        let rows = (0..4096)
            .map(|index| shared_row(index, 0, &control))
            .collect();
        let frame = frame(rows);
        let doc =
            terminal_read_doc(terminal(), &frame, ReadPresentation::default()).expect("document");
        assert!(doc.rows.is_empty());
        assert_eq!(doc.row_count, 4096);
        assert_eq!(doc.omitted_rows, 4096);
        let content = &Projection::TerminalRead(doc).result()["content"];
        assert_eq!(content["truncated"], true);
        assert_eq!(content["omittedRows"], 4096);
        assert_eq!(content["rows"].as_array().expect("rows").len(), 0);
    }

    /// Grid-cell rendering inside the budget path: a spacer tail is
    /// collapsed into its wide cell, a spacer head or an empty cell
    /// renders one space, trailing padding trims, and a blank-only row
    /// emits an empty line. The rows obey the decoder's topology: a row
    /// ending in a spacer head sets `intoNext`, so the next row opens
    /// with a wide cell and `fromPrevious`. The dedicated 3-column
    /// geometry keeps every row at exactly `columns` cells so the real
    /// decoder accepts the frame.
    #[test]
    fn spacer_and_blank_cells_render_as_grid_content() {
        let wide_text: Arc<str> = Arc::from("中");
        let blank: Arc<str> = Arc::from("");
        let narrow: Arc<str> = Arc::from("x");
        let rows = vec![
            TerminalTextRow {
                prompt_class: 0,
                index: 0,
                flags: 1,
                cells: vec![cell(&narrow, 0), cell(&narrow, 0), cell(&blank, 3)],
            },
            TerminalTextRow {
                prompt_class: 0,
                index: 1,
                flags: 2,
                cells: vec![
                    cell(&wide_text, 1),
                    cell(&blank, 2),
                    cell(&Arc::from("界"), 0),
                ],
            },
            TerminalTextRow {
                prompt_class: 0,
                index: 2,
                flags: 0,
                cells: vec![cell(&blank, 0), cell(&narrow, 0), cell(&blank, 0)],
            },
            TerminalTextRow {
                prompt_class: 0,
                index: 3,
                flags: 0,
                cells: vec![cell(&blank, 0), cell(&blank, 0), cell(&blank, 0)],
            },
        ];
        // The pre-repair shape: the same rows under the shared 64-column
        // geometry carry fewer than `columns` cells, so the real decoder
        // rejects the frame as `InvalidField`.
        let mut under_width = rows.clone();
        under_width[3].cells.pop();
        let error = decode_terminal_text_frame(&wire(&frame(under_width)))
            .expect_err("under-width rows must be rejected");
        assert_eq!(error.code(), ProtocolErrorCode::InvalidField);
        let frame = frame_with_columns(rows, 3);
        let decoded = decode_terminal_text_frame(&wire(&frame)).expect("fixture decodes");
        assert_eq!(decoded.geometry, frame.geometry);
        assert_eq!(decoded.rows, frame.rows);
        let doc =
            terminal_read_doc(terminal(), &frame, ReadPresentation::default()).expect("document");
        let texts: Vec<&str> = doc.rows.iter().map(|row| row.text.as_str()).collect();
        assert_eq!(texts, ["xx", "中界", " x", ""]);
        assert!(doc.rows[0].into_next && doc.rows[1].from_previous);
        assert_eq!(doc.omitted_rows, 0);
    }

    /// Negative control for the budget stop: emitted rows are a strict
    /// prefix of the frame, never a greedy subset. The middle row alone
    /// overflows the remaining budget — 64 cells sharing one 16 KiB
    /// control string JSON-escape to 6,291,458 bytes — so the small tail
    /// row behind it must be omitted even though it would fit on its
    /// own. A `break`→`continue` mutation would emit that tail and fail
    /// here. This pins the CLI/decoder boundary only; no real host is
    /// involved.
    #[test]
    fn rows_after_the_budget_stop_stay_omitted() {
        let head: Arc<str> = Arc::from("head\u{1b}row");
        let control: Arc<str> = Arc::from("\u{1}".repeat(16384).as_str());
        let tail: Arc<str> = Arc::from("tail-marker-row");
        // One literal cell plus narrow blank cells: the joined line is
        // the literal text after trailing-blank trim, and each row
        // carries exactly `columns` cells as the decoder requires. A
        // spacer head is not legal padding — it is only valid as the
        // final cell of a row that wraps `intoNext`.
        let blank: Arc<str> = Arc::from("");
        let text_row = |index: u16, text: &Arc<str>| {
            let mut cells = vec![cell(text, 0)];
            cells.extend((1..COLUMNS).map(|_| cell(&blank, 0)));
            TerminalTextRow {
                prompt_class: 0,
                index,
                flags: 0,
                cells,
            }
        };
        // The pre-repair shape padded with spacer heads whose flags were
        // 0: mid-row spacer heads and a spacer head on a non-wrapping row
        // are both `InvalidField` to the real decoder.
        let spacer_head_row = |index: u16, text: &Arc<str>| {
            let mut cells = vec![cell(text, 0)];
            cells.extend((1..COLUMNS).map(|_| cell(&blank, 3)));
            TerminalTextRow {
                prompt_class: 0,
                index,
                flags: 0,
                cells,
            }
        };
        let illegal = frame(vec![
            spacer_head_row(0, &head),
            shared_row(1, 0, &control),
            spacer_head_row(2, &tail),
        ]);
        let error = decode_terminal_text_frame(&wire(&illegal))
            .expect_err("spacer-head padding must be rejected");
        assert_eq!(error.code(), ProtocolErrorCode::InvalidField);
        let frame = frame(vec![
            text_row(0, &head),
            shared_row(1, 0, &control),
            text_row(2, &tail),
        ]);
        let decoded = decode_terminal_text_frame(&wire(&frame)).expect("fixture decodes");
        assert_eq!(decoded.geometry, frame.geometry);
        assert_eq!(decoded.rows, frame.rows);
        let doc =
            terminal_read_doc(terminal(), &frame, ReadPresentation::default()).expect("document");
        assert_eq!(doc.row_count, 3);
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(doc.rows[0].index, 0);
        assert_eq!(doc.rows[0].text, "head\u{1b}row");
        assert_eq!(doc.omitted_rows, 2);

        let projection = Projection::TerminalRead(doc);
        let content = &projection.result()["content"];
        assert_eq!(content["rowCount"], 3);
        assert_eq!(content["truncated"], true);
        assert_eq!(content["omittedRows"], 2);
        let rows = content["rows"].as_array().expect("rows array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["index"], 0);
        assert_eq!(rows[0]["text"], "head\u{1b}row");

        // The real text path emits only the prefix: the head's control
        // character stays escaped, the affordable tail never leaks, and
        // the localized marker counts both omitted rows.
        let command = QueryCommand::TerminalRead {
            target: Target::Id(hex(0x01)),
            history: None,
            unwrap: false,
            lines: None,
        };
        let body = render_text(|out| present::render(out, &projection, &command, Locale::EnUs))
            .expect("text output fits the preparation bound");
        let body = String::from_utf8(body).expect("utf8 output");
        assert!(body.contains("head\\u{001b}row"));
        assert!(!body.contains("tail-marker-row"));
        assert!(body.ends_with("… 2 more rows truncated at the read limit\n"));
    }

    /// A narrow-column frame of short text rows with explicit wrap bits:
    /// bit 0 `intoNext`, bit 1 `fromPrevious`.
    fn narrow_frame(rows: &[(u16, u8, &str)]) -> TerminalTextFrame {
        let blank: Arc<str> = Arc::from("");
        frame_with_columns(
            rows.iter()
                .map(|(index, flags, text)| {
                    let mut cells = vec![cell(&Arc::from(*text), 0)];
                    cells.extend((1..3).map(|_| cell(&blank, 0)));
                    TerminalTextRow {
                        prompt_class: 0,
                        index: *index,
                        flags: *flags,
                        cells,
                    }
                })
                .collect(),
            3,
        )
    }

    /// `--unwrap`: only verified soft-wrap chains join — hard breaks and
    /// empty rows stay their own logical lines, physical row indexes
    /// span the join, and a window opening or ending inside a wrap marks
    /// the fragment instead of claiming a complete line.
    #[test]
    fn unwrap_joins_wraps_and_marks_fragments() {
        let frame = narrow_frame(&[
            (0, 2, "tail of wrapped"), // continues from an unfetched row
            (1, 0, "hard"),
            (2, 1, "soft-"),
            (3, 3, "wrap-"),
            (4, 2, "done"),
            (5, 0, ""),
            (6, 1, "open"),
        ]);
        let doc = terminal_read_doc(
            terminal(),
            &frame,
            ReadPresentation {
                unwrap: true,
                tail: None,
            },
        )
        .expect("document");
        // Physical facts stay: 7 rows applied, 0 omitted.
        assert_eq!(doc.row_count, 7);
        assert_eq!(doc.rows.len(), 7);
        let lines = doc.lines.as_ref().expect("lines emitted");
        assert_eq!(doc.line_count, Some(5));
        assert_eq!(
            lines
                .iter()
                .map(|line| (
                    line.first_row,
                    line.last_row,
                    line.text.as_str(),
                    line.starts_mid_line,
                    line.ends_mid_line,
                ))
                .collect::<Vec<_>>(),
            [
                (0, 0, "tail of wrapped", true, false),
                (1, 1, "hard", false, false),
                (2, 4, "soft-wrap-done", false, false),
                (5, 5, "", false, false),
                (6, 6, "open", false, true),
            ]
        );
        let content = &Projection::TerminalRead(doc).result()["content"];
        assert_eq!(content["rowCount"], 7);
        assert_eq!(content["lineCount"], 5);
        assert!(content.get("rows").is_none());
        let emitted = content["lines"].as_array().expect("lines array");
        assert_eq!(emitted[0]["firstRow"], 0);
        assert_eq!(emitted[0]["startsMidLine"], true);
        assert_eq!(emitted[4]["endsMidLine"], true);
    }

    /// `--lines` without `--unwrap` tails physical rows and reports the
    /// presentation omission separately from the budget's `omittedRows`.
    #[test]
    fn lines_tails_physical_rows() {
        let frame = narrow_frame(&[(0, 0, "a"), (1, 0, "b"), (2, 0, "c")]);
        let doc = terminal_read_doc(
            terminal(),
            &frame,
            ReadPresentation {
                unwrap: false,
                tail: Some(2),
            },
        )
        .expect("document");
        assert_eq!(doc.row_count, 3);
        assert_eq!(
            doc.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
            [1, 2]
        );
        let tail = doc.tail.as_ref().expect("tail accounting");
        assert_eq!((tail.requested, tail.omitted), (2, 1));
        let content = &Projection::TerminalRead(doc).result()["content"];
        assert_eq!(content["tail"]["requested"], 2);
        assert_eq!(content["tail"]["omitted"], 1);
        assert_eq!(content["tail"]["entries"], "rows");
        assert_eq!(content["rowCount"], 3);
        assert_eq!(content["omittedRows"], 0);
        // A tail wider than the window omits nothing.
        let doc = terminal_read_doc(
            terminal(),
            &frame,
            ReadPresentation {
                unwrap: false,
                tail: Some(9),
            },
        )
        .expect("document");
        assert_eq!(doc.rows.len(), 3);
        assert_eq!(doc.tail.expect("tail").omitted, 0);
    }

    /// `--unwrap --lines`: the tail counts logical lines, including the
    /// observed edge fragments, never the retained-history total.
    #[test]
    fn lines_tails_logical_lines() {
        let frame = narrow_frame(&[
            (0, 1, "wrap "),
            (1, 2, "head"),
            (2, 0, "mid"),
            (3, 0, "last"),
        ]);
        let doc = terminal_read_doc(
            terminal(),
            &frame,
            ReadPresentation {
                unwrap: true,
                tail: Some(2),
            },
        )
        .expect("document");
        let lines = doc.lines.as_ref().expect("lines");
        assert_eq!(doc.line_count, Some(3));
        assert_eq!(
            lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            ["mid", "last"]
        );
        let tail = doc.tail.as_ref().expect("tail");
        assert_eq!((tail.requested, tail.omitted), (2, 1));
        let content = &Projection::TerminalRead(doc).result()["content"];
        assert_eq!(content["tail"]["entries"], "lines");
        assert_eq!(content["tail"]["omitted"], 1);
        assert_eq!(content["lineCount"], 3);
    }
}

/// `terminal wait-output` matching: soft-wrapped rows join into logical
/// lines before the literal substring check, the excerpt always contains
/// the complete pattern within 4096 UTF-8 bytes on char boundaries, and
/// the observed-window projection reports the honest truncation flag.
#[cfg(any(unix, windows))]
mod terminal_wait {
    use super::super::{
        MAXIMUM_EXCERPT_BYTES, TerminalRow, WaitMatch, WaitMatcher, WaitObservation, match_rows,
        match_window, terminal_wait_output_doc,
    };
    use orchbee_terminal_protocol::{
        AttachmentId, HostEpoch, TerminalRowPromptClass, TerminalTextRowWindow,
        TerminalTextWindowRow,
    };
    use std::num::NonZeroU64;

    fn row(index: u64, wrap: bool, continuation: bool, text: &str) -> TerminalTextWindowRow {
        TerminalTextWindowRow {
            index,
            prompt_class: TerminalRowPromptClass::None,
            wrap,
            wrap_continuation: continuation,
            identity_ascii: true,
            text: text.into(),
            mappings: Vec::new(),
        }
    }

    fn window(rows: Vec<TerminalTextWindowRow>, total_rows: u64) -> TerminalTextRowWindow {
        TerminalTextRowWindow {
            attachment_id: AttachmentId::from_bytes([7; 16]).expect("attachment"),
            host_epoch: HostEpoch::from_bytes([9; 16]).expect("epoch"),
            sequence: NonZeroU64::new(1).expect("sequence"),
            revision: 4,
            generation: NonZeroU64::new(2).expect("generation"),
            total_rows,
            start_row: rows.first().map(|row| row.index).unwrap_or(0),
            columns: 80,
            ignored_optional_flags: 0,
            rows,
        }
    }

    /// A match that only exists inside the joined logical line: no single
    /// physical row contains the pattern, so a per-row search would miss.
    #[test]
    fn wrap_chain_joins_before_matching() {
        let hit = match_window(
            &window(
                vec![
                    row(5, true, false, "prompt$ echo abc"),
                    row(6, false, true, "def done"),
                    row(7, false, false, "unrelated"),
                ],
                8,
            ),
            &WaitMatcher::Literal("abcdef"),
        )
        .expect("joined line matches");
        assert_eq!(hit.row_start, 5);
        assert_eq!(hit.row_end, 6);
        assert_eq!(hit.excerpt, "prompt$ echo abcdef done");
        assert!(!hit.excerpt_truncated);
        assert_eq!(hit.match_offset, 13);
        assert_eq!(hit.excerpt_offset, 0);
    }

    #[test]
    fn physical_source_does_not_join_wraps_and_keeps_original_edge_facts() {
        let rows = [(5, "HEAD", false, true), (6, "TAIL", true, false)];
        let matcher = WaitMatcher::Literal("HEADTAIL");
        assert!(match_rows(rows, &matcher, false).is_none());
        let joined = match_rows(rows, &matcher, true).unwrap();
        assert_eq!((joined.row_start, joined.row_end), (5, 6));
        let regex = regex::Regex::new("^HEAD$").unwrap();
        let physical = match_rows(rows, &WaitMatcher::Regex(&regex), false).unwrap();
        assert_eq!((physical.row_start, physical.row_end), (5, 5));
        let detail = physical.regex.unwrap();
        assert!(!detail.starts_mid_line && detail.ends_mid_line);
        assert!(
            match_rows(
                [],
                &WaitMatcher::Regex(&regex::Regex::new("^$").unwrap()),
                true
            )
            .is_none()
        );
    }

    /// The first matching logical line wins; hard boundaries never join.
    #[test]
    fn first_logical_line_wins_across_hard_boundaries() {
        let hit = match_window(
            &window(
                vec![
                    row(0, false, false, "first hit here"),
                    row(1, true, false, "second "),
                    row(2, false, true, "hit joined"),
                ],
                3,
            ),
            &WaitMatcher::Literal("hit"),
        )
        .expect("earliest line matches");
        assert_eq!(hit.row_start, 0);
        assert_eq!(hit.row_end, 0);
        assert_eq!(hit.excerpt, "first hit here");
    }

    /// Matching is a literal, case-sensitive substring: regex syntax and
    /// case folds never match.
    #[test]
    fn literal_case_sensitive_substring() {
        let window = window(vec![row(0, false, false, "Foo a.b")], 1);
        assert!(match_window(&window, &WaitMatcher::Literal("foo")).is_none());
        assert!(match_window(&window, &WaitMatcher::Literal("a.b")).is_some());
        assert!(match_window(&window, &WaitMatcher::Literal("a[x]b")).is_none());
        assert!(match_window(&window, &WaitMatcher::Literal("a.*b")).is_none());
    }

    /// A window that opens mid-line (leading `wrap_continuation`) still
    /// joins that fragment with the following continuation rows.
    #[test]
    fn leading_continuation_fragment_joins() {
        let hit = match_window(
            &window(
                vec![
                    row(9, true, true, "tail of earlier "),
                    row(10, false, true, "marker"),
                    row(11, false, false, "other"),
                ],
                12,
            ),
            &WaitMatcher::Literal("earlier marker"),
        )
        .expect("fragment plus continuation matches");
        assert_eq!(hit.row_start, 9);
        assert_eq!(hit.row_end, 10);
    }

    /// Long lines excerpt to ≤4096 bytes with the complete pattern kept;
    /// a tail hit re-anchors instead of losing the match to alignment.
    #[test]
    fn excerpt_keeps_complete_pattern_within_budget() {
        let padding = "x".repeat(6000);
        let line = format!("{padding} needle");
        let (excerpt, truncated, offset) = super::super::match_excerpt(&line, 6001, "needle".len());
        assert!(truncated);
        assert!(excerpt.len() <= MAXIMUM_EXCERPT_BYTES);
        assert!(excerpt.contains("needle"));
        let start = usize::try_from(offset).expect("offset");
        assert!(start <= 6001);
        assert!(6001 + "needle".len() <= start + excerpt.len());

        // Under the bound the whole line is the excerpt.
        let short = "short line with needle";
        let (excerpt, truncated, offset) = super::super::match_excerpt(short, 17, "needle".len());
        assert!(!truncated);
        assert_eq!(excerpt, short);
        assert_eq!(offset, 0);
    }

    /// Excerpt slicing never splits a UTF-8 code point: a CJK-dense line
    /// still yields a valid, pattern-containing window.
    #[test]
    fn excerpt_respects_char_boundaries() {
        let line = format!("{}目标", "中".repeat(2000));
        let start = line.find("目标").expect("pattern");
        let (excerpt, truncated, offset) = super::super::match_excerpt(&line, start, 6);
        assert!(truncated);
        assert!(excerpt.len() <= MAXIMUM_EXCERPT_BYTES);
        assert!(excerpt.contains("目标"));
        assert!(line.is_char_boundary(usize::try_from(offset).expect("offset")));
    }

    /// Review F3: the re-anchor arm is reachable — a 3-byte CJK prefix
    /// pushes the centered start down to 0, so the aligned end lands at
    /// 4096 < 4097 below the match end. Re-anchoring on the match
    /// boundary keeps the complete 4094-byte pattern inside the budget.
    #[test]
    fn excerpt_reanchors_when_cjk_prefix_drops_the_aligned_end() {
        let pattern = "a".repeat(4094);
        let line = format!("中{pattern}{}", "z".repeat(10));
        let match_start = line.find(&pattern).expect("pattern");
        assert_eq!(match_start, 3);
        let (excerpt, truncated, offset) =
            super::super::match_excerpt(&line, match_start, pattern.len());
        assert!(truncated);
        assert!(excerpt.len() <= MAXIMUM_EXCERPT_BYTES);
        // The re-anchored excerpt is exactly the whole pattern.
        assert_eq!(excerpt, pattern);
        assert_eq!(usize::try_from(offset).expect("offset"), 3);
        assert!(line.is_char_boundary(3));
    }

    /// `windowTruncated` is the byte-capacity clamp fact: true only when
    /// the searched window is smaller than the requested-or-retained
    /// extent, never a guess.
    #[test]
    fn doc_reports_window_truncation_honestly() {
        let matched = WaitMatch {
            excerpt: "hit".to_owned(),
            excerpt_truncated: false,
            excerpt_offset: 0,
            match_offset: 0,
            row_start: 3,
            row_end: 3,
            regex: None,
        };
        let doc = |window_rows, total_rows, requested| {
            terminal_wait_output_doc(
                TerminalRow {
                    terminal_id: "0".repeat(32),
                    worker_id: "0".repeat(32),
                    presentation_id: "0".repeat(32),
                    panel_id: "0".repeat(32),
                    tab_id: "0".repeat(32),
                    terminal_context_id: "0".repeat(32),
                },
                AttachmentId::from_bytes([7; 16]).expect("attachment"),
                HostEpoch::from_bytes([9; 16]).expect("epoch"),
                WaitObservation {
                    matched: matched.clone(),
                    revision: 4,
                    generation: 2,
                    window_start: 0,
                    window_rows,
                    total_rows,
                },
                super::super::WaitReport {
                    observations: 1,
                    requested_rows: requested,
                    timeout_ms: Some(30_000),
                    elapsed_ms: 12,
                },
            )
        };
        // 500 rows searched but 1000 requested and 1000 retained: clamped.
        assert!(doc(500, 2000, 1000).window_truncated);
        // The full retained window was searched.
        assert!(!doc(24, 24, 1000).window_truncated);
        // Fewer requested rows than retained: the request bound the window.
        assert!(!doc(10, 24, 10).window_truncated);
    }

    fn program(pattern: &str) -> regex::Regex {
        regex::Regex::new(pattern).expect("regex compiles")
    }

    /// `--regex` searches the same joined fragments: the hit is the
    /// engine's leftmost match inside the first matching fragment, and
    /// the extended members carry the complete length, the bounded
    /// matched text, and the honest fragment-edge flags.
    #[test]
    fn regex_matches_joined_fragment() {
        let program = program("[a-z]+ done");
        let hit = match_window(
            &window(
                vec![
                    row(5, true, false, "prompt$ echo abc"),
                    row(6, false, true, "def done"),
                    row(7, false, false, "unrelated"),
                ],
                8,
            ),
            &WaitMatcher::Regex(&program),
        )
        .expect("joined fragment matches");
        assert_eq!((hit.row_start, hit.row_end), (5, 6));
        assert_eq!(hit.match_offset, 13);
        let detail = hit.regex.expect("regex detail");
        assert_eq!(detail.match_length, 11);
        assert_eq!(detail.matched, "abcdef done");
        assert!(!detail.matched_truncated);
        assert!(!detail.starts_mid_line && !detail.ends_mid_line);
    }

    /// A window-edge fragment flags both ends: `^`/`$` anchors bind only
    /// the observed fragment, never unseen text outside it.
    #[test]
    fn regex_fragment_edges_flagged() {
        let program = program("^earlier marker$");
        let hit = match_window(
            &window(
                vec![
                    row(9, true, true, "earlier "),
                    row(10, true, true, "marker"),
                ],
                12,
            ),
            &WaitMatcher::Regex(&program),
        )
        .expect("anchored fragment matches");
        let detail = hit.regex.expect("regex detail");
        assert!(detail.starts_mid_line && detail.ends_mid_line);
        assert_eq!(hit.excerpt, "earlier marker");
    }

    /// An empty observed line is a real fragment: `^$` matches it at
    /// offset 0 with length 0 — zero-length matches are hits.
    #[test]
    fn regex_empty_match_on_empty_line() {
        let program = program("^$");
        let hit = match_window(
            &window(
                vec![
                    row(0, false, false, "content"),
                    row(1, false, false, ""),
                    row(2, false, false, "later"),
                ],
                3,
            ),
            &WaitMatcher::Regex(&program),
        )
        .expect("empty line matches ^$");
        assert_eq!((hit.row_start, hit.row_end), (1, 1));
        assert_eq!(hit.match_offset, 0);
        let detail = hit.regex.expect("regex detail");
        assert_eq!(detail.match_length, 0);
        assert_eq!(detail.matched, "");
        assert!(!detail.matched_truncated);
    }

    /// Unicode text: offsets are UTF-8 byte offsets into the joined
    /// fragment, and a first-fragment hit wins ordering.
    #[test]
    fn regex_unicode_offsets_and_first_fragment() {
        let program = program("目标\\d+");
        let hit = match_window(
            &window(
                vec![
                    row(0, false, false, "前面目标42后面"),
                    row(1, false, false, "目标99 second"),
                ],
                2,
            ),
            &WaitMatcher::Regex(&program),
        )
        .expect("first fragment wins");
        assert_eq!(hit.row_start, 0);
        // "前面" is 6 bytes; the match starts at byte 6.
        assert_eq!(hit.match_offset, 6);
        let detail = hit.regex.expect("regex detail");
        assert_eq!(detail.matched, "目标42");
        assert_eq!(detail.match_length, 8);
    }

    /// A regex match longer than the excerpt bound reports its complete
    /// length honestly, a bounded `matched` prefix, and an excerpt that
    /// leads with the match — never an underflow or a false whole.
    #[test]
    fn regex_oversized_match_is_bounded() {
        let program = program("x+");
        let line = format!("pre{}", "x".repeat(8000));
        let hit = match_window(
            &window(vec![row(0, false, false, &line)], 1),
            &WaitMatcher::Regex(&program),
        )
        .expect("long match hits");
        assert_eq!(hit.match_offset, 3);
        let detail = hit.regex.expect("regex detail");
        assert_eq!(detail.match_length, 8000);
        assert!(detail.matched_truncated);
        assert!(detail.matched.len() <= MAXIMUM_EXCERPT_BYTES);
        assert!(detail.matched.bytes().all(|byte| byte == b'x'));
        // The excerpt leads with the match start, explicitly truncated.
        assert!(hit.excerpt_truncated);
        assert_eq!(hit.excerpt_offset, 3);
        assert!(hit.excerpt.len() <= MAXIMUM_EXCERPT_BYTES);
    }
}

/// Every handle a list prints resolves through the matching `get` to the same
/// member, including members whose short suffixes collide, and the parser
/// accepts it; no handle is the rejected `@kind:<32 digits>` form.
#[test]
fn list_handles_round_trip_through_get() {
    let snapshot = selector_snapshot();
    let target = |handle: &str| {
        if handle.starts_with('@') {
            Target::Selector(handle.to_owned())
        } else {
            Target::Id(handle.to_owned())
        }
    };
    let parses = |verb: [&str; 2], handle: &str| {
        crate::invocation::parse_request(
            [verb[0], verb[1], handle]
                .into_iter()
                .map(std::ffi::OsString::from),
        )
        .is_ok()
    };
    let Projection::ProjectList(projects) = project(&QueryCommand::ProjectList, &snapshot).unwrap()
    else {
        panic!("project list");
    };
    for row in &projects {
        assert!(row.handle.len() < 32 + "@project:".len(), "{}", row.handle);
        assert!(parses(["project", "get"], &row.handle), "{}", row.handle);
        let Projection::ProjectGet(doc) =
            project(&QueryCommand::ProjectGet(target(&row.handle)), &snapshot).unwrap()
        else {
            panic!("project get");
        };
        assert_eq!(doc.row.project_id, row.project_id);
    }
    // A filtered list keeps handles unique over every Workspace.
    let Projection::WorkspaceList(workspaces) = project(
        &QueryCommand::WorkspaceList {
            project: Some(Target::Id(tid(0, 0xaa))),
        },
        &snapshot,
    )
    .unwrap() else {
        panic!("workspace list");
    };
    assert_eq!(workspaces.len(), 1);
    for row in &workspaces {
        assert_eq!(row.handle, "@workspace:000000000000000cc");
        assert!(parses(["workspace", "get"], &row.handle), "{}", row.handle);
        let Projection::WorkspaceGet(doc) =
            project(&QueryCommand::WorkspaceGet(target(&row.handle)), &snapshot).unwrap()
        else {
            panic!("workspace get");
        };
        assert_eq!(doc.row.workspace_id, row.workspace_id);
    }
    let Projection::TabList(tabs) = project(
        &QueryCommand::TabList {
            workspace: None,
            standalone: false,
        },
        &snapshot,
    )
    .unwrap() else {
        panic!("tab list");
    };
    for row in &tabs {
        assert!(parses(["tab", "get"], &row.handle), "{}", row.handle);
        let Projection::TabGet(doc) =
            project(&QueryCommand::TabGet(target(&row.handle)), &snapshot).unwrap()
        else {
            panic!("tab get");
        };
        assert_eq!(doc.row.tab_id, row.tab_id);
    }
}

/// The text ambiguity diagnostic lists bounded candidates as list handles
/// with their labels and the true total; JSON keeps its identifier details.
#[test]
fn ambiguity_text_lists_candidate_handles_with_labels() {
    let snapshot = selector_snapshot();
    let failure = project(
        &QueryCommand::WorkspaceGet(Target::Label("w".into())),
        &snapshot,
    )
    .unwrap_err();
    let details = failure.details.clone().unwrap();
    assert_eq!(details["totalCount"], 2);
    assert_eq!(details["candidates"], json!([tid(0, 0xcc), tid(1, 0xcc)]));
    let english = crate::localization::Locale::EnUs.cli_failure(&failure);
    assert_eq!(
        english,
        "2 workspace candidates match \"w\": @workspace:000000000000000cc (\"w\"), @workspace:100000000000000cc (\"w\"). Use one of these handles or the full ID.\n"
    );
    let chinese = crate::localization::Locale::ZhCn.cli_failure(&failure);
    assert!(
        chinese.contains("@workspace:100000000000000cc (\"w\")"),
        "{chinese}"
    );
    assert!(chinese.contains("共有 2 个"), "{chinese}");
}

/// `terminal list` filters resolve their member like the matching `get`:
/// an absent member is not-found, an existing one may yield an empty list,
/// and every row keeps location handles unique over the whole snapshot.
#[test]
fn terminal_list_filters_by_workspace_tab_and_panel() {
    use crate::invocation::TerminalListFilter;
    let snapshot = selector_snapshot();
    let list = |filter| match project(&QueryCommand::TerminalList { filter }, &snapshot) {
        Ok(Projection::TerminalList(doc)) => Ok(doc),
        Ok(_) => panic!("terminal list"),
        Err(failure) => Err(failure.kind),
    };
    let all = list(None).unwrap();
    assert_eq!(all.rows.len(), all.details.len());
    let standalone_tab = list(Some(TerminalListFilter::Tab(Target::Id(tid(0, 0xee))))).unwrap();
    assert_eq!(
        standalone_tab
            .rows
            .iter()
            .map(|row| row.terminal_id.as_str())
            .collect::<Vec<_>>(),
        [tid(0, 0x210), tid(1, 0x1210)]
    );
    for details in &standalone_tab.details {
        let placement = details.placement.as_ref().unwrap();
        assert_eq!(placement.workspace, None);
        assert!(placement.handle.starts_with("@terminal:"));
        assert!(placement.panel.starts_with("@panel:"));
    }
    let panel = list(Some(TerminalListFilter::Panel(Target::Id(tid(1, 0xff))))).unwrap();
    assert_eq!(panel.rows.len(), 1);
    assert_eq!(panel.rows[0].terminal_id, tid(1, 0x1210));
    // An existing Workspace without bound terminals is an empty success.
    let empty = list(Some(TerminalListFilter::Workspace(Target::Id(tid(
        0, 0xdd,
    )))))
    .unwrap();
    assert!(empty.rows.is_empty());
    assert_eq!(
        list(Some(TerminalListFilter::Workspace(Target::Id(tid(
            7, 0x77
        )))))
        .unwrap_err(),
        FailureKind::NotFound
    );
    let Projection::PanelList(panels) = project(
        &QueryCommand::PanelList {
            tab: None,
            workspace: Some(Target::Id(tid(0, 0xdd))),
        },
        &snapshot,
    )
    .unwrap() else {
        panic!("panel list");
    };
    let workspace_context = snapshot
        .navigation
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id.as_str() == tid(0, 0xdd))
        .unwrap()
        .terminal_context_id
        .clone();
    assert!(
        panels
            .iter()
            .all(|row| row.terminal_context_id == workspace_context.as_str())
    );
}

/// `workspace get` text names the Workspace first and shows the primary
/// directory path rather than its identity.
#[test]
fn workspace_get_text_leads_with_the_workspace_and_its_directory_path() {
    let snapshot = selector_snapshot();
    let command = QueryCommand::WorkspaceGet(Target::Selector("@workspace:dd".into()));
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let projection = project(&command, &snapshot).unwrap();
        let Projection::WorkspaceGet(doc) = &projection else {
            panic!("workspace get projection");
        };
        let summary = locale.cli_workspace_summary(
            &crate::cli::escape::quoted(&doc.row.label),
            doc.row.number,
            doc.row.tab_count,
            doc.row.panel_count,
        );
        let path = doc.primary_directory_path.clone();
        let directory_id = doc.row.primary_directory_id.clone();
        let output = crate::cli::success_output(
            projection,
            serde_json::json!({"kind":"workbench-snapshot","siteId":tid(0, 1),"revision":"1"}),
            &command,
            crate::invocation::OutputFormat::Text,
            locale,
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.starts_with(&summary), "{text}");
        if let Some(path) = path {
            assert!(text.contains(&path), "{text}");
        }
        if let Some(directory_id) = directory_id {
            assert!(!text.contains(&directory_id), "{text}");
        }
        assert!(
            text.contains(locale.cli_ungrouped()) || text.contains("("),
            "{text}"
        );
    }
}
