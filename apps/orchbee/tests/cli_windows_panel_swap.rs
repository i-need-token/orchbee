//! Real Windows CLI phase-order and retained-effect witnesses over named pipes.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use peer_support::{Fixture, peer, validate};
use serde_json::{Value, json};
use std::io::Read;

fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn panel(n: u64) -> Value {
    json!({"kind":"panel","panelId":id(n),"primaryPresentation":{"customLabel":null,
        "presentationId":id(n+100),"label":format!("panel {n}"),"binding":null},"childPresentations":[]})
}
fn snapshot() -> Snapshot {
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":"12",
        "navigation":{"projects":[], "standaloneTerminalContextId":id(4), "directories":[], "workspaces":[]},
        "terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[
            {"tabId":id(5),"label":"one","pinned":false,"groupId":null,"root":{
                "kind":"split","splitId":id(50),"orientation":"horizontal","children":[
                    {"slotId":id(51),"node":panel(6)}, {"slotId":id(52),"node":panel(8)}]}},
            {"tabId":id(9),"label":"two","pinned":false,"groupId":null,"root":panel(10)}]}]}))
    .unwrap()
}
fn client(revision: &str, selected: u64) -> Value {
    json!({"clientId":id(7),"heartbeat":"1","viewRevision":if revision=="12" {"2"} else {"3"},
        "view":{"workbenchRevision":revision,"phase":"ready","selection":{
            "terminalContextId":id(4),"tabId":id(5),"panelId":id(selected),"zoomed":false,"panelCount":2}},
        "availability":"current"})
}
fn read_view(stream: &mut LocalStream) -> WorkbenchClientViewRequest {
    let WorkbenchRequest::ClientView(request) = read_workbench_request(stream).unwrap() else {
        panic!("Client view request");
    };
    request
}
fn view_reply(stream: &mut LocalStream, request: &WorkbenchClientViewRequest, outcome: Value) {
    let result = WorkbenchClientViewResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: serde_json::from_value(outcome).unwrap(),
    };
    write_workbench_response(stream, &WorkbenchResponse::ClientView(result)).unwrap();
}
fn setup(stream: &mut LocalStream) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot first");
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(),
            },
        }),
    )
    .unwrap();
    let request = read_view(stream);
    assert!(matches!(request.action, ClientViewAction::List {}));
    view_reply(
        stream,
        &request,
        json!({"kind":"listed","clients":[client("12",8)]}),
    );
}
fn eof(stream: &mut LocalStream) {
    assert_eq!(
        stream.read(&mut [0]).unwrap(),
        0,
        "no retry, compensation, or later phase"
    );
}

#[derive(Clone, Copy, Debug)]
enum Cut {
    Complete,
    CanonicalLost,
    Refused,
    ObservationLost,
    SelectionChanged,
    FocusLost,
    FocusRejected,
    WaitLost,
    WaitTimedOut,
}

fn workflow(stream: &mut LocalStream, cut: Cut) {
    setup(stream);
    exchange_and_focus(stream, cut);
}
fn exchange_and_focus(stream: &mut LocalStream, cut: Cut) {
    let WorkbenchRequest::Command(command) = read_workbench_request(stream).unwrap() else {
        panic!("swap second");
    };
    assert_eq!(command.expected_revision, WorkbenchRevision::new(12));
    assert_eq!(
        json!(command.action),
        json!({"kind":"panel.swap","terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"targetPanelId":id(8)})
    );
    if matches!(cut, Cut::CanonicalLost) {
        return;
    }
    let outcome = if matches!(cut, Cut::Refused) {
        json!({"kind":"rejected","revision":"14","reason":"revision-mismatch"})
    } else {
        json!({"kind":"panel-swapped","revision":"13","terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"targetPanelId":id(8)})
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
    if matches!(cut, Cut::Refused) {
        eof(stream);
        return;
    }
    let request = read_view(stream);
    assert!(matches!(request.action, ClientViewAction::Get { .. }));
    if matches!(cut, Cut::ObservationLost) {
        return;
    }
    view_reply(
        stream,
        &request,
        json!({"kind":"found","client":client("13", if matches!(cut,Cut::SelectionChanged) {6} else {8})}),
    );
    if matches!(cut, Cut::SelectionChanged) {
        eof(stream);
        return;
    }
    let request = read_view(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("one focus");
    };
    assert_eq!(
        command.minimum_workbench_revision,
        WorkbenchRevision::new(13)
    );
    assert_eq!(json!(command.expected_view_revision), "3");
    assert_eq!(
        json!(command.intent),
        json!({"kind":"panel","terminalContextId":id(4),"tabId":id(5),"panelId":id(6)})
    );
    if matches!(cut, Cut::FocusLost) {
        return;
    }
    let status = match cut {
        Cut::FocusRejected => json!({"kind":"rejected","reason":"stale-view"}),
        Cut::WaitLost | Cut::WaitTimedOut => json!({"kind":"queued"}),
        _ => json!({"kind":"applied","view":client("13",6)["view"]}),
    };
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":status}}),
    );
    if matches!(cut, Cut::WaitLost | Cut::WaitTimedOut) {
        let next = read_view(stream);
        assert!(matches!(next.action, ClientViewAction::Read { .. }));
        if matches!(cut, Cut::WaitTimedOut) {
            std::thread::sleep(std::time::Duration::from_millis(1800));
            eof(stream);
        }
        return;
    }
    eof(stream);
}

#[test]
fn every_exchange_fault_retains_prior_effects_and_never_replays() {
    for cut in [
        Cut::Complete,
        Cut::CanonicalLost,
        Cut::Refused,
        Cut::ObservationLost,
        Cut::SelectionChanged,
        Cut::FocusLost,
        Cut::FocusRejected,
        Cut::WaitLost,
        Cut::WaitTimedOut,
    ] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            workflow(&mut stream, cut)
        });
        let output = fixture.run(&[
            "panel",
            "swap",
            "@panel:06",
            "--with",
            "@panel:08",
            "--client",
            &id(7),
            if matches!(cut, Cut::WaitTimedOut) {
                "--timeout=1500"
            } else {
                "--timeout=30000"
            },
            "--format=json",
        ]);
        assert!(
            handler.join().is_ok(),
            "{cut:?}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.panel.swap.json"),
        );
        assert!(
            output.stderr.is_empty(),
            "{cut:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let (status, outcome, canonical) = match cut {
            Cut::Complete => (0, "swapped", "committed"),
            Cut::CanonicalLost => (8, "unknown", "unknown"),
            Cut::Refused => (3, "not-swapped", "refused"),
            Cut::ObservationLost | Cut::WaitLost => (5, "incomplete", "committed"),
            Cut::SelectionChanged | Cut::FocusRejected => (3, "incomplete", "committed"),
            Cut::FocusLost => (8, "incomplete", "committed"),
            Cut::WaitTimedOut => (7, "incomplete", "committed"),
        };
        assert_eq!(output.status.code(), Some(status), "{cut:?}: {document}");
        assert_eq!(document["outcome"], outcome, "{cut:?}");
        assert_eq!(document["canonical"]["kind"], canonical, "{cut:?}");
        if canonical == "committed" {
            assert_eq!(document["canonical"]["revision"], "13");
        }
        if matches!(cut, Cut::WaitLost) {
            assert_eq!(document["focus"]["observation"]["result"]["kind"], "queued");
            assert_eq!(document["focus"]["wait"], "transport-lost");
        }
    }
}

#[test]
fn same_panel_and_cross_tab_are_effect_free_receipts() {
    for (target, reason) in [("@panel:06", "same-panel"), ("@panel:0a", "cross-tab")] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |mut stream| {
            setup(&mut stream);
            eof(&mut stream);
        });
        let output = fixture.run(&[
            "panel",
            "swap",
            "@panel:06",
            "--with",
            target,
            "--client",
            &id(7),
            "--format=json",
        ]);
        assert!(
            handler.join().is_ok(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(output.status.success());
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.panel.swap.json"),
        );
        assert_eq!(document["reason"], reason);
        assert_eq!(
            document["canonical"],
            json!({"kind":"unchanged","revision":"12"})
        );
        assert!(document["requestId"].is_null());
        assert!(document["focus"].is_null());
    }
}

#[test]
fn directional_swap_uses_one_layout_and_refuses_to_retarget_after_revision_change() {
    for mode in ["swap", "no-neighbor", "changed", "hidden", "lost"] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            setup(&mut stream);
            let request = read_view(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("layout");
            };
            assert_eq!(
                command.minimum_workbench_revision,
                WorkbenchRevision::new(12)
            );
            assert!(matches!(command.intent, ClientViewIntent::Layout { .. }));
            if mode == "lost" {
                return;
            }
            let corpus: Value = serde_json::from_str(include_str!(
                "../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
            ))
            .unwrap();
            let name = if mode == "hidden" {
                "hidden-source-layout-read"
            } else {
                "layout-read"
            };
            let mut status = corpus["valid"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"] == name)
                .unwrap()["document"]["outcome"]["operation"]["status"]
                .clone();
            if mode == "hidden" {
                status["layout"]["sourcePanelId"] = json!(id(6));
                status["layout"]["panels"][0]["panelId"] = json!(id(8));
                status["layout"]["panels"][1]["panelId"] = json!(id(6));
            }
            status["view"] = client(if mode == "changed" { "13" } else { "12" }, 8)["view"].clone();
            status["layout"]["focusedPanelId"] = json!(id(8));
            if mode == "changed" {
                status["layout"]["workbenchRevision"] = json!("13");
            }
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"operation","operation":{"command":command,"status":status}}),
            );
            if mode == "swap" {
                exchange_and_focus(&mut stream, Cut::Complete);
            } else {
                eof(&mut stream);
            }
        });
        let output = fixture.run(&[
            "panel",
            "swap",
            "@panel:06",
            "--direction",
            if mode == "no-neighbor" {
                "left"
            } else {
                "right"
            },
            "--client",
            &id(7),
            "--format=json",
        ]);
        assert!(
            handler.join().is_ok(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.panel.swap.json"),
        );
        let (exit, kind, reason) = match mode {
            "swap" => (0, "committed", Value::Null),
            "no-neighbor" => (0, "unchanged", json!("no-neighbor")),
            "changed" => (3, "not-submitted", json!("layout-revision-changed")),
            "hidden" => (3, "not-submitted", json!("source-unavailable")),
            _ => (8, "not-submitted", json!("layout-unconfirmed")),
        };
        assert_eq!(output.status.code(), Some(exit), "{mode}: {document}");
        assert_eq!(document["canonical"]["kind"], kind, "{mode}: {document}");
        assert_eq!(document["reason"], reason);
        assert!(document["layoutOperationId"].is_string());
        if mode != "swap" {
            assert!(document["requestId"].is_null());
            assert!(document["focus"].is_null());
        }
    }
}
