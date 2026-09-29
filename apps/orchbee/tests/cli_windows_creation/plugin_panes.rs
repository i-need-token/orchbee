use super::*;

#[test]
fn plugin_pane_open_submits_catalog_identity_once_and_preserves_canonical_receipts() {
    for (split, focus, lost) in [
        (false, false, false),
        (true, true, false),
        (true, false, true),
    ] {
        let fixture = Fixture::new();
        let manifest = fixture.0.join("orchbee-plugin.toml");
        std::fs::write(&manifest, "id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\nplacement='tab'\ncommand=['cmd.exe','/d','/c','echo dashboard']\n").unwrap();
        let linked = fixture.run(&[
            "plugin",
            "link",
            manifest.to_str().unwrap(),
            "--format=json",
        ]);
        assert!(linked.status.success(), "{linked:?}");
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream, snapshot(false, split));
            if !split || focus {
                let request = view_request(&mut stream);
                assert!(matches!(request.action, ClientViewAction::List {}));
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"listed","clients":[record(false,split,false)]}),
                );
            }
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!()
            };
            let action = json!(command.action);
            assert_eq!(
                action["kind"],
                if split { "panel.split" } else { "tab.create" }
            );
            assert_eq!(action["label"], "Dashboard");
            assert_eq!(
                action["options"]["pluginPane"],
                json!({"pluginId":"tools","entrypointId":"dashboard"})
            );
            assert_eq!(
                action["options"]["environment"],
                json!([["LITERAL", "hello world"]])
            );
            assert!(action.get("launch").is_none() && action.get("profile").is_none());
            assert!(action["options"].get("cwd").is_none());
            if lost {
                return;
            }
            let outcome = if split {
                json!({"kind":"panel-split-pending","revision":"13","terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"newPanelId":id(20),"splitId":id(30),"firstSlotId":id(31),"secondSlotId":id(32),"presentationId":id(120)})
            } else {
                json!({"kind":"tab-create-pending","revision":"13","terminalContextId":id(4),"tabId":id(19),"panelId":id(20),"presentationId":id(120)})
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: command.request_id,
                    outcome: serde_json::from_value(outcome).unwrap(),
                }),
            )
            .unwrap();
            if focus {
                assert!(observe(&mut stream, split, Cut::Complete));
                assert!(submit(&mut stream, split, false, Cut::Complete));
            }
            assert_closed(&mut stream);
        });
        let panel = id(6);
        let mut args = vec![
            "plugin",
            "pane",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--env",
            "LITERAL=hello world",
            "--format=json",
        ];
        if split {
            args.extend(["--placement", "split", "--target-pane", &panel]);
        }
        if !focus {
            args.push("--no-focus");
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        if lost {
            assert_eq!(output.status.code(), Some(8), "{document}");
        } else {
            assert!(output.status.success(), "{document}");
            validate(
                &document,
                include_str!("../../src/cli/schemas/orchbee.creation.result.json"),
            );
            assert_eq!(document["directory"]["policy"], "plugin-root");
            let mut legacy = document.clone();
            legacy["version"] = json!(2);
            let schema: Value = serde_json::from_str(include_str!(
                "../../src/cli/schemas/orchbee.creation.result.json"
            ))
            .unwrap();
            assert!(
                !jsonschema::validator_for(&schema)
                    .unwrap()
                    .is_valid(&legacy)
            );
        }
    }
}
