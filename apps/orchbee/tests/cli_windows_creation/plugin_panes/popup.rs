use super::*;
use orchbee_workbench_protocol::ClientPresentationSelection;

#[test]
fn popup_cli_uses_exact_source_sizes_and_one_client_intent_without_canonical_creation() {
    for case in ["default", "override", "stale", "lost"] {
        let fixture = Fixture::new();
        let manifest = fixture.0.join("orchbee-plugin.toml");
        std::fs::write(&manifest,"id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\nplacement='popup'\nwidth='75%'\nheight=7\ncommand=['cmd.exe']\n").unwrap();
        assert!(
            fixture
                .run(&[
                    "plugin",
                    "link",
                    manifest.to_str().unwrap(),
                    "--format=json"
                ])
                .status
                .success()
        );
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let before = snapshot(false, true);
            snapshot_reply(&mut stream, before.clone());
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::List {}));
            let original = record(false, true, false);
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"listed","clients":[original.clone()]}),
            );
            source::read(&mut stream, &before, Some(&original), 106, false, false);
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::Get { .. }));
            let mut current = original.clone();
            if case == "stale" {
                current["viewRevision"] = json!("3");
                current["view"]["selection"]["panelId"] = json!(id(20));
            }
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"found","client":current}),
            );
            if case == "stale" {
                assert_closed(&mut stream);
                return;
            }
            let request = view_request(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("one popup intent");
            };
            let ClientViewIntent::PopupOpen { spec } = &command.intent else {
                panic!("popup must not create a canonical Pane");
            };
            assert_eq!(spec.source.panel_id.as_str(), id(6));
            assert!(
                matches!(&spec.source.member,ClientPresentationSelection::Unbound { presentation_id,selection_generation }
                if presentation_id.as_str()==id(106) && selection_generation.value()==1)
            );
            assert_eq!(
                json!(spec.width),
                if case == "override" {
                    json!({"kind":"cells","value":0})
                } else {
                    json!({"kind":"percent","value":75})
                }
            );
            assert_eq!(json!(spec.height), json!({"kind":"cells","value":7}));
            assert_eq!(
                spec.context.focused_pane_id.as_deref(),
                Some(id(6).as_str())
            );
            if case == "lost" {
                return;
            }
            let mut view = original["view"].clone();
            view["phase"] = json!("busy");
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"operation","operation":{"command":command,"status":{"kind":"popup-opened","popupId":command.operation_id,"view":view}}}),
            );
            assert_closed(&mut stream);
        });
        let mut args = vec![
            "plugin",
            "pane",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--format=json",
        ];
        if case == "override" {
            args.extend(["--width", "0"]);
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        if case == "stale" {
            assert!(!output.status.success());
        } else {
            validate(
                &document,
                include_str!("../../../src/cli/schemas/orchbee.popup.result.json"),
            );
            assert_eq!(document["schema"], "orchbee.popup.result");
            assert_eq!(
                output.status.code(),
                Some(if case == "lost" { 8 } else { 0 }),
                "{document}"
            );
        }
    }
}

#[test]
fn popup_close_cli_addresses_client_even_with_noncanonical_caller_and_reports_no_popup() {
    for had_popup in [false, true] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream, snapshot(false, true));
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::List {}));
            let mut original = record(false, true, false);
            original["view"]["phase"] = json!("busy");
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"listed","clients":[original.clone()]}),
            );
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::Get { .. }));
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"found","client":original.clone()}),
            );
            let request = view_request(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("one popup close");
            };
            assert!(matches!(command.intent, ClientViewIntent::PopupClose {}));
            let mut view = original["view"].clone();
            if had_popup {
                view["phase"] = json!("ready");
            }
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"operation","operation":{"command":command,"status":{"kind":"popup-closed","hadPopup":had_popup,"view":view}}}),
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run_with_caller(
            &["popup", "close", "--format=json"],
            Some(&id(9000)),
            Some(&id(9001)),
        );
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../../../src/cli/schemas/orchbee.popup.result.json"),
        );
        assert_eq!(output.status.code(), Some(0), "{document}");
        assert_eq!(
            document["observation"]["reply"]["outcome"]["operation"]["status"]["hadPopup"],
            had_popup
        );
    }
}

fn link_popup(fixture: &Fixture) {
    let manifest = fixture.0.join("orchbee-plugin.toml");
    std::fs::write(&manifest,"id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\nplacement='popup'\nwidth='50%'\nheight=10\ncommand=['cmd.exe']\n").unwrap();
    assert!(
        fixture
            .run(&[
                "plugin",
                "link",
                manifest.to_str().unwrap(),
                "--format=json"
            ])
            .status
            .success()
    );
}

const OPEN: [&str; 8] = [
    "plugin",
    "pane",
    "open",
    "--plugin",
    "tools",
    "--entrypoint",
    "dashboard",
    "--format=json",
];

/// Replays the request sequence of a real Session with two TUI Clients: the popup source's own
/// terminal-metadata read moves every Client's view revision by two without changing its
/// selection, and output in another Workspace keeps stepping it. The popup addresses the Client
/// at its current revision and repeats an open the Server refused as stale, instead of failing;
/// a selection change still fails before anything opens.
#[test]
fn popup_cli_readdresses_a_client_whose_view_moved_without_a_selection_change() {
    for case in ["stepped", "selection-changed"] {
        let fixture = Fixture::new();
        link_popup(&fixture);
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let before = snapshot(false, true);
            snapshot_reply(&mut stream, before.clone());
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::List {}));
            let original = record(false, true, false);
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"listed","clients":[original.clone()]}),
            );
            let stepped = source::read_stepped(&mut stream, &before, &original, 106);
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::Get { .. }));
            let mut current = stepped.clone();
            if case == "selection-changed" {
                current["view"]["selection"]["panelId"] = json!(id(20));
            }
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"found","client":current}),
            );
            if case == "selection-changed" {
                assert_closed(&mut stream);
                return;
            }
            source::refuse_stale(&mut stream, &stepped);
            let stepped = source::stepped(&stepped);
            source::found(&mut stream, &stepped);
            let request = view_request(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("one popup intent");
            };
            let ClientViewIntent::PopupOpen { spec } = &command.intent else {
                panic!("one popup intent");
            };
            assert_eq!(
                json!(command.expected_view_revision),
                stepped["viewRevision"],
                "the popup addresses the Client's current view"
            );
            assert_eq!(
                spec.context.correlation_id.as_deref(),
                Some(command.operation_id.as_str()),
                "a repeated open correlates with its own operation"
            );
            let mut view = stepped["view"].clone();
            view["phase"] = json!("busy");
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"operation","operation":{"command":command,"status":{"kind":"popup-opened","popupId":command.operation_id,"view":view}}}),
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&OPEN);
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        if case == "stepped" {
            assert_eq!(output.status.code(), Some(0), "{document}");
            assert_eq!(document["schema"], "orchbee.popup.result");
        } else {
            assert_eq!(output.status.code(), Some(3), "{document}");
            assert_eq!(document["failure"]["details"]["reason"], "client-changed");
        }
    }
}

/// A busy Client names why it refuses a popup. The CLI no longer stops at the listed phase: the
/// passive source reads still answer while the Client is busy, and the Client's typed refusal of
/// the open reaches the caller as `client-refused` with that reason.
#[test]
fn popup_cli_reports_why_a_busy_client_refuses_the_popup() {
    for reason in ["modal-open", "insufficient-space"] {
        let fixture = Fixture::new();
        link_popup(&fixture);
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let before = snapshot(false, true);
            snapshot_reply(&mut stream, before.clone());
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::List {}));
            let mut original = record(false, true, false);
            original["view"]["phase"] = json!("busy");
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"listed","clients":[original.clone()]}),
            );
            source::read(&mut stream, &before, Some(&original), 106, false, false);
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::Get { .. }));
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"found","client":original.clone()}),
            );
            let request = view_request(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("one popup intent");
            };
            assert!(matches!(command.intent, ClientViewIntent::PopupOpen { .. }));
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"operation","operation":{"command":command,"status":{"kind":"rejected","reason":reason}}}),
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&OPEN);
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.code(), Some(3), "{document}");
        assert_eq!(
            document["failure"]["details"],
            json!({"stage":"plugin","reason":"client-refused","clientReason":reason})
        );
    }
}
