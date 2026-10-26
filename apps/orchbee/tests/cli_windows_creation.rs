//! Windows creation: one canonical effect, optional guarded Client phases, no replay.
#![cfg(windows)]
#[path = "support/windows_agent_peer/cohort_fixture.rs"]
mod cohort_fixture;
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
use cohort_fixture::CohortFixture as Fixture;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use peer_support::{assert_closed, peer, validate};
use serde_json::{Value, json};

#[test]
fn fixture_child_route_is_default_unless_explicitly_overridden() {
    use std::ffi::OsStr;

    let fixture = Fixture::new();
    let command = fixture.command(&["--help"], None, None, &[]);
    for name in ["ORCHBEE_RUNTIME_SCOPE", "ORCHBEE_SESSION"] {
        assert_eq!(
            command.get_envs().find(|(key, _)| *key == name),
            Some((OsStr::new(name), None)),
            "the default peer route must not inherit {name}"
        );
    }
    let overrides = [
        ("ORCHBEE_RUNTIME_SCOPE", OsStr::new("explicit-scope")),
        ("ORCHBEE_SESSION", OsStr::new("explicit-site")),
    ];
    let command = fixture.command(&["--help"], None, None, &overrides);
    for (name, value) in overrides {
        assert_eq!(
            command.get_envs().find(|(key, _)| *key == name),
            Some((OsStr::new(name), Some(value)))
        );
    }
}

fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn panel(n: u64) -> Value {
    json!({"kind":"panel","panelId":id(n),"primaryPresentation":{"customLabel":null,"presentationId":id(n+100),"label":"shell","binding":null},"childPresentations":[]})
}
fn tree(split: u64, left: Value, right: Value) -> Value {
    json!({"kind":"split","splitId":id(split),"orientation":"horizontal","children":[{"slotId":id(split+1),"node":left},{"slotId":id(split+2),"node":right}]})
}
fn snapshot(created: bool, split: bool) -> Snapshot {
    let root = tree(
        50,
        if created && split {
            tree(30, panel(6), panel(20))
        } else {
            panel(6)
        },
        panel(8),
    );
    let mut tabs =
        vec![json!({"tabId":id(5),"label":"one","pinned":false,"groupId":null,"root":root})];
    if created && !split {
        tabs.push(
            json!({"tabId":id(19),"label":"2","pinned":false,"groupId":null,"root":panel(20)}),
        );
    }
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":if created {"13"} else {"12"},"navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},"terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":tabs}]})).unwrap()
}
fn record(created: bool, split: bool, focused: bool) -> Value {
    json!({"clientId":id(7),"heartbeat":"1","viewRevision":if created {"3"} else {"2"},"availability":"current","view":{"workbenchRevision":if created {"13"} else {"12"},"phase":"ready","selection":{"terminalContextId":id(4),"tabId":id(if focused && !split {19} else {5}),"panelId":id(if focused {20} else {6}),"zoomed":false,"panelCount":if focused && !split {1} else if created && split {3} else {2}}}})
}
fn snapshot_reply(stream: &mut LocalStream, snapshot: Snapshot) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot");
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }),
    )
    .unwrap();
}
fn view_request(stream: &mut LocalStream) -> WorkbenchClientViewRequest {
    let WorkbenchRequest::ClientView(request) = read_workbench_request(stream).unwrap() else {
        panic!("view");
    };
    request
}
fn view_reply(stream: &mut LocalStream, request: &WorkbenchClientViewRequest, outcome: Value) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cut {
    Complete,
    CanonicalLost,
    Refused,
    SnapshotLost,
    IdentityChanged,
    SelectionChanged,
    CountChanged,
    RatioLost,
    RatioRejected,
    FocusLost,
    FocusRejected,
    WaitLost,
    WaitTimeout,
}
fn create(stream: &mut LocalStream, split: bool, cut: Cut) -> bool {
    let WorkbenchRequest::Command(command) = read_workbench_request(stream).unwrap() else {
        panic!("single canonical create");
    };
    assert_eq!(command.expected_revision, WorkbenchRevision::new(12));
    let action = json!(command.action);
    assert_eq!(
        action["kind"],
        if split { "panel.split" } else { "tab.create" }
    );
    assert_eq!(action["terminalContextId"], id(4));
    assert_eq!(action["label"], if split { "3" } else { "2" });
    if split {
        assert_eq!(action["panelId"], id(6));
        assert_eq!(action["direction"], "right");
    }
    if cut == Cut::CanonicalLost {
        return false;
    }
    let outcome = if cut == Cut::Refused {
        json!({"kind":"rejected","revision":"14","reason":"revision-mismatch"})
    } else if split {
        json!({"kind":"panel-split-pending","revision":"13","terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"newPanelId":id(20),"splitId":id(30),"firstSlotId":id(31),"secondSlotId":id(32),"presentationId":id(120)})
    } else {
        json!({"kind":"tab-create-pending","revision":"13","terminalContextId":id(4),"tabId":id(19),"panelId":id(20),"presentationId":id(120)})
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
    if cut == Cut::Refused {
        assert_closed(stream);
        return false;
    }
    true
}
fn observe(stream: &mut LocalStream, split: bool, cut: Cut) -> bool {
    let request = view_request(stream);
    assert!(matches!(request.action, ClientViewAction::Get { .. }));
    let mut current = record(true, split, false);
    if cut == Cut::SelectionChanged {
        current["view"]["selection"]["panelId"] = json!(id(8));
    }
    if cut == Cut::CountChanged {
        current["view"]["selection"]["panelCount"] = json!(4);
    }
    view_reply(stream, &request, json!({"kind":"found","client":current}));
    if matches!(cut, Cut::SelectionChanged | Cut::CountChanged) {
        assert_closed(stream);
        false
    } else {
        true
    }
}
fn submit(stream: &mut LocalStream, split: bool, ratio: bool, cut: Cut) -> bool {
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("one Client submission");
    };
    assert_eq!(json!(command.expected_view_revision), "3");
    assert_eq!(command.client_id.as_str(), id(7));
    assert_eq!(
        command.minimum_workbench_revision,
        WorkbenchRevision::new(13)
    );
    if ratio {
        assert_eq!(
            json!(command.intent),
            json!({"kind":"set-split-ratio","terminalContextId":id(4),"tabId":id(5),"path":[0],"ratio":0.7,"expectedSplit":{"splitId":id(30),"firstSlotId":id(31),"secondSlotId":id(32)}})
        );
    } else {
        assert_eq!(
            json!(command.intent),
            json!({"kind":"panel","terminalContextId":id(4),"tabId":id(if split {5} else {19}),"panelId":id(20)})
        );
    }
    if (ratio && cut == Cut::RatioLost) || (!ratio && cut == Cut::FocusLost) {
        return false;
    }
    let rejected = (ratio && cut == Cut::RatioRejected) || (!ratio && cut == Cut::FocusRejected);
    let waiting = !ratio && matches!(cut, Cut::WaitLost | Cut::WaitTimeout);
    let status = if rejected {
        json!({"kind":"rejected","reason":"stale-view"})
    } else if waiting {
        json!({"kind":"queued"})
    } else if ratio {
        json!({"kind":"layout-adjusted","view":record(true,split,false)["view"],"adjustment":{"terminalContextId":id(4),"tabId":id(5),"workbenchRevision":"13","selected":true,"change":{"kind":"split","split":{"splitId":id(30),"firstSlotId":id(31),"secondSlotId":id(32),"previousRatio":0.5,"ratio":0.7}}}})
    } else {
        json!({"kind":"applied","view":record(true,split,true)["view"]})
    };
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":status}}),
    );
    if rejected {
        assert_closed(stream);
        return false;
    }
    if waiting {
        let next = view_request(stream);
        assert!(matches!(next.action, ClientViewAction::Read { .. }));
        if cut == Cut::WaitTimeout {
            std::thread::sleep(std::time::Duration::from_millis(1800));
            assert_closed(stream);
        }
        return false;
    }
    true
}
fn workflow(
    stream: &mut LocalStream,
    split: bool,
    ratio: bool,
    focus: bool,
    listed: bool,
    cut: Cut,
) {
    snapshot_reply(stream, snapshot(false, split));
    if listed {
        let request = view_request(stream);
        assert!(matches!(request.action, ClientViewAction::List {}));
        view_reply(
            stream,
            &request,
            json!({"kind":"listed","clients":[record(false,split,false)]}),
        );
    }
    if !create(stream, split, cut) {
        return;
    }
    if ratio {
        if cut == Cut::SnapshotLost {
            let _ = read_workbench_request(stream).unwrap();
            return;
        }
        let mut fresh = snapshot(true, true);
        if cut == Cut::IdentityChanged {
            let PanelNode::Split { children, .. } = &mut fresh.terminal_contexts[0].tabs[0].root
            else {
                panic!();
            };
            let PanelNode::Split { split_id, .. } = &mut children[0].node else {
                panic!();
            };
            *split_id = WorkbenchSplitId::parse(&id(40)).unwrap();
        }
        snapshot_reply(stream, fresh);
        if cut == Cut::IdentityChanged {
            assert_closed(stream);
            return;
        }
        if !observe(stream, split, cut) || !submit(stream, split, true, cut) {
            return;
        }
    }
    if focus && (!observe(stream, split, cut) || !submit(stream, split, false, cut)) {
        return;
    }
    assert_closed(stream);
}
fn run(
    split: bool,
    ratio: bool,
    focus: bool,
    implicit: bool,
    cut: Cut,
) -> (std::process::Output, Value) {
    let fixture = if cut == Cut::WaitTimeout {
        Fixture::deadline_sensitive()
    } else {
        Fixture::new()
    };
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        workflow(
            &mut stream,
            split,
            ratio,
            focus,
            implicit || ratio || focus,
            cut,
        )
    });
    let mut args = if split {
        vec!["panel", "split", "--direction=right"]
    } else {
        vec!["tab", "create"]
    };
    if !implicit {
        if split {
            args.push("@panel:0x6");
        } else {
            args.extend(["--workspace", "Workspace"]);
        }
    }
    if ratio {
        args.push("--ratio=0.7");
    }
    if focus {
        args.push("--focus");
    } else {
        args.push("--no-focus");
    }
    let client = id(7);
    if ratio || focus {
        args.extend(["--client", &client]);
    }
    args.push(if cut == Cut::WaitTimeout {
        "--timeout=1500"
    } else {
        "--timeout=30000"
    });
    args.push("--format=json");
    let output = fixture.run(&args);
    assert!(
        handler.join().is_ok(),
        "{split}/{ratio}/{focus}/{cut:?}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        matches!(cut, Cut::CanonicalLost | Cut::Refused) || output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    if !matches!(cut, Cut::CanonicalLost | Cut::Refused) {
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.creation.result.json"),
        );
        assert_eq!(document["result"]["revision"], "13");
        assert_eq!(document["result"]["terminal"]["outcome"], "pending");
    }
    (output, document)
}
#[test]
fn creation_default_headless_current_focus_and_ratio_are_independent() {
    for split in [false, true] {
        for implicit in [false, true] {
            let (output, doc) = run(split, false, false, implicit, Cut::Complete);
            assert!(output.status.success(), "{doc}");
            assert_eq!(doc["postCreate"]["outcome"], "not-requested");
            assert!(doc["postCreate"]["focus"].is_null());
        }
        let (output, doc) = run(split, false, true, false, Cut::Complete);
        assert!(output.status.success(), "{doc}");
        assert_eq!(doc["postCreate"]["outcome"], "completed");
    }
    let (output, doc) = run(true, true, false, false, Cut::Complete);
    assert!(output.status.success(), "{doc}");
    assert!(doc["postCreate"]["focus"].is_null());
}

#[test]
fn creation_client_preflight_refuses_missing_ambiguous_and_unready_before_mutation() {
    for mode in ["missing", "ambiguous", "unready", "stale"] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream, snapshot(false, false));
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::List {}));
            let mut clients = vec![];
            if mode != "missing" {
                let mut first = record(false, false, false);
                if mode == "unready" {
                    first["view"]["phase"] = json!("busy");
                }
                if mode == "stale" {
                    first["availability"] = json!("stale");
                }
                clients.push(first);
                if mode == "ambiguous" {
                    let mut second = record(false, false, false);
                    second["clientId"] = json!(id(9));
                    clients.push(second);
                }
            }
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"listed","clients":clients}),
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["tab", "create", "--focus", "--format=json"]);
        assert!(handler.join().is_ok());
        assert_eq!(
            output.status.code(),
            Some(if mode == "unready" { 4 } else { 3 })
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(document.get("postCreate").is_none());
    }
}

#[test]
fn first_tab_focus_drives_refresh_from_an_unchanged_empty_client() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |mut stream| {
        let mut empty = snapshot(false, false);
        empty.terminal_contexts[0].tabs.clear();
        snapshot_reply(&mut stream, empty);
        let mut current = record(false, false, false);
        current["view"]["selection"] = json!({"terminalContextId":id(4),"tabId":null,"panelId":null,"panelCount":0,"zoomed":false});
        let list = view_request(&mut stream);
        assert!(matches!(list.action, ClientViewAction::List {}));
        view_reply(
            &mut stream,
            &list,
            json!({"kind":"listed","clients":[current.clone()]}),
        );
        assert!(create(&mut stream, false, Cut::Complete));
        let get = view_request(&mut stream);
        assert!(matches!(get.action, ClientViewAction::Get { .. }));
        // No terminal exists to poll the new publication: the view remains at 12.
        view_reply(&mut stream, &get, json!({"kind":"found","client":current}));
        let request = view_request(&mut stream);
        let ClientViewAction::Submit { command } = &request.action else {
            panic!("focus must drive refresh");
        };
        assert_eq!(json!(command.expected_view_revision), "2");
        assert_eq!(
            command.minimum_workbench_revision,
            WorkbenchRevision::new(13)
        );
        assert_eq!(
            json!(command.intent),
            json!({"kind":"panel","terminalContextId":id(4),"tabId":id(19),"panelId":id(20)})
        );
        view_reply(
            &mut stream,
            &request,
            json!({"kind":"operation","operation":{"command":command,"status":{"kind":"applied","view":record(true,false,true)["view"]}}}),
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "tab",
        "create",
        "--label=2",
        "--focus",
        "--client",
        &id(7),
        "--format=json",
    ]);
    assert!(handler.join().is_ok());
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(output.status.success(), "{document}");
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.creation.result.json"),
    );
    assert_eq!(document["postCreate"]["outcome"], "completed");
}
#[test]
fn creation_phase_faults_preserve_known_effects_and_stop_later_submissions() {
    for cut in [
        Cut::Complete,
        Cut::CanonicalLost,
        Cut::Refused,
        Cut::SnapshotLost,
        Cut::IdentityChanged,
        Cut::SelectionChanged,
        Cut::CountChanged,
        Cut::RatioLost,
        Cut::RatioRejected,
        Cut::FocusLost,
        Cut::FocusRejected,
        Cut::WaitLost,
        Cut::WaitTimeout,
    ] {
        let (output, doc) = run(true, true, true, false, cut);
        let code = match cut {
            Cut::Complete => 0,
            Cut::CanonicalLost | Cut::RatioLost | Cut::FocusLost => 8,
            Cut::SnapshotLost => 4,
            Cut::WaitLost => 5,
            Cut::WaitTimeout => 7,
            _ => 3,
        };
        assert_eq!(output.status.code(), Some(code), "{cut:?}: {doc}");
        if matches!(cut, Cut::CanonicalLost | Cut::Refused) {
            assert!(doc.get("postCreate").is_none());
            continue;
        }
        assert_eq!(
            doc["postCreate"]["outcome"],
            if cut == Cut::Complete {
                "completed"
            } else {
                "incomplete"
            }
        );
        if matches!(
            cut,
            Cut::FocusLost | Cut::FocusRejected | Cut::WaitLost | Cut::WaitTimeout
        ) {
            assert_eq!(
                doc["postCreate"]["ratio"]["observation"]["reply"]["outcome"]["operation"]["status"]
                    ["kind"],
                "layout-adjusted"
            );
        }
    }
}

#[allow(dead_code)]
#[path = "support/terminal_metadata_peer.rs"]
mod terminal_metadata_peer;

#[path = "cli_windows_creation/plugin_panes.rs"]
mod plugin_panes;
