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
