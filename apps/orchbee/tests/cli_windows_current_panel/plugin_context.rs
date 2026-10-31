//! Plugin dispatch must follow the same exact-member observation as current-Panel reads.
use super::*;

#[test]
fn plugin_context_preflight_uses_exact_caller_or_client_and_refuses_selection_races() {
    for case in [
        "client",
        "caller-child",
        "explicit",
        "busy",
        "final-moved",
        "ambiguous",
        "wrong-tab",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let submitted = matches!(case, "client" | "caller-child" | "explicit" | "busy");
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let mut listed = None;
            if case != "caller-child" {
                let mut selected = record(10, 6);
                if case == "busy" {
                    selected["view"]["phase"] = json!("busy");
                }
                if case == "wrong-tab" {
                    selected["view"]["selection"]["tabId"] = json!(id(99));
                }
                listed = Some(selected.clone());
                let mut clients = vec![selected];
                if case == "ambiguous" {
                    clients.push(record(11, 8));
                }
                list_reply(&mut stream, clients);
            }
            if !matches!(case, "ambiguous" | "wrong-tab") {
                enrichment::read_source(
                    &mut stream,
                    6,
                    (case != "caller-child").then_some(10),
                    case,
                    listed.as_ref(),
                );
            }
            if submitted {
                let WorkbenchRequest::Plugin(request) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("one plugin submission after stable reads")
                };
                let PluginAction::Invoke {
                    context,
                    operation_id,
                    ..
                } = &request.action
                else {
                    panic!()
                };
                assert_eq!(context.tab_id.as_deref(), Some(id(5).as_str()));
                assert_eq!(context.tab_label.as_deref(), Some("two"));
                assert_eq!(context.focused_pane_id.as_deref(), Some(id(6).as_str()));
                assert_eq!(context.workspace_id.as_deref(), Some("display-only"));
                assert_eq!(context.selected_text.as_deref(), Some("选区"));
                assert_eq!(
                    context.correlation_id.as_deref(),
                    Some(operation_id.as_str())
                );
                assert!(
                    context.focused_pane_cwd.is_none(),
                    "unavailable Host cwd is not invented"
                );
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Plugin(PluginResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        session_id: request.session_id,
                        outcome: PluginOutcome::Failed {
                            reason: PluginFailure::Disabled,
                        },
                    }),
                )
                .unwrap();
            }
            assert_closed(&mut stream);
        });
        let client_id = id(10);
        let mut args = vec![
            "plugin",
            "action",
            "invoke",
            "tools.action",
            "--format=json",
            "--context",
            r#"{"workspaceId":"display-only","selectedText":"选区"}"#,
        ];
        if case == "explicit" {
            args.extend(["--client", &client_id]);
        }
        let caller = id(406);
        let output = fixture.run_with_caller(
            &args,
            match case {
                "caller-child" => Some(&caller),
                "explicit" => Some("invalid-inherited"),
                _ => None,
            },
            (case == "caller-child").then_some("6f7263686265652d6c6f63616c2d7631"),
        );
        server.join().unwrap();
        assert!(!output.status.success(), "{case}: {output:?}");
        let document = document(&output);
        if submitted {
            validate(
                &document,
                include_str!("../../src/cli/schemas/orchbee.plugin.runtime.json"),
            );
            assert_eq!(
                document["result"]["replies"][0]["outcome"]["reason"], "disabled",
                "{case}: {document}"
            );
        } else {
            assert!(document.get("result").is_none(), "{case}: {document}");
            assert!(document["failure"].is_object(), "{case}: {document}");
        }
    }
}
