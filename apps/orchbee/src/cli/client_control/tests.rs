use super::*;
use crate::invocation::{self, ClientNavigationTarget, Request, Target, ZoomMode};
use orchbee_workbench_protocol::{
    ClientViewAvailability, ClientViewIntent, ClientViewRecord, Snapshot, WorkbenchClientId,
    WorkbenchResponse,
};
use serde_json::Value;
use std::ffi::OsString;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap()
}
fn sample(name: &str) -> Value {
    corpus()["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap()["document"]
        .clone()
}
fn record() -> ClientViewRecord {
    let WorkbenchResponse::ClientView(result) = serde_json::from_value(sample("found")).unwrap()
    else {
        panic!("result");
    };
    let ClientViewOutcome::Found { client } = result.outcome else {
        panic!("client");
    };
    *client
}
fn snapshot() -> Snapshot {
    let id = |number| format!("{number:032x}");
    serde_json::from_value(json!({"protocolVersion":1, "siteId":id(2), "revision":"12",
        "navigation":{"projects":[],"standaloneTerminalContextId":id(4)},
        "terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[{"tabId":id(5),"label":"shell","pinned":false,"groupId":null,
        "root":{"kind":"panel","panelId":id(6),"primaryPresentation":{"presentationId":id(8),"label":"shell","binding":null},"childPresentations":[]}}]}]})).unwrap()
}

#[test]
fn current_panel_read_refuses_stale_location_instead_of_following_a_moved_panel() {
    let mut client = record();
    let snapshot = snapshot();
    let target = Target::Id("00000000000000000000000000000006".into());
    assert_eq!(
        selection::current_panel(&client, &snapshot).unwrap(),
        target
    );
    client.view.selection.as_mut().unwrap().tab_id = Some(
        orchbee_workbench_protocol::WorkbenchTabId::parse("00000000000000000000000000000009")
            .unwrap(),
    );
    let error = selection::current_panel(&client, &snapshot).unwrap_err();
    assert_eq!(error.details.unwrap()["reason"], "client-panel-unavailable");
    // Navigation may deliberately follow an explicit Panel; observation cannot.
    assert!(
        selection::intent(
            &ClientNavigationTarget::Panel(Some(target)),
            &client,
            None,
            &snapshot
        )
        .is_ok()
    );
}

#[test]
fn agent_attention_grammar_selects_read_only_client_query() {
    let parse = |words: &[&str]| invocation::parse_request(words.iter().map(OsString::from));
    assert!(matches!(
        parse(&[
            "agent",
            "attention",
            "--session",
            "agent-1",
            "--client",
            "00000000000000000000000000000007",
            "--format=json"
        ]),
        Ok(Request::Client {
            command: ClientCommand::Navigate {
                target: ClientNavigationTarget::AgentAttention(_),
                ..
            },
            ..
        })
    ));
    for words in [
        vec!["agent", "attention"],
        vec!["agent", "attention", "one", "two"],
        vec!["agent", "attention", "--name", "one", "--session", "two"],
        vec!["agent", "attention", "one", "--until", "done"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn agent_focus_grammar_requires_one_agent_target_and_reuses_navigation_receipts() {
    let parse = |words: &[&str]| invocation::parse_request(words.iter().map(OsString::from));
    let id = "00000000000000000000000000000007";
    for selector in [
        vec!["alpha"],
        vec!["@panel:07"],
        vec!["--name", "alpha"],
        vec!["--session", "sess-alpha"],
        vec!["--panel", id],
    ] {
        let mut words = vec!["agent", "focus"];
        words.extend(selector);
        let Request::Client {
            command:
                ClientCommand::Navigate {
                    target: ClientNavigationTarget::Agent(_),
                    timeout_ms,
                    ..
                },
            ..
        } = parse(&words).unwrap()
        else {
            panic!("Agent navigation");
        };
        assert_eq!(timeout_ms, 30_000);
        words.extend([
            "--client",
            id,
            "--request-id",
            id,
            "--detach",
            "--timeout=1",
            "--format=json",
        ]);
        let Request::Client {
            command:
                ClientCommand::Navigate {
                    client,
                    operation_id,
                    detach,
                    timeout_ms,
                    ..
                },
            format,
        } = parse(&words).unwrap()
        else {
            panic!("navigation");
        };
        assert_eq!(client.unwrap().as_str(), id);
        assert_eq!(operation_id.unwrap().as_str(), id);
        assert!(detach);
        assert_eq!(timeout_ms, 1);
        assert_eq!(format, OutputFormat::Json);
    }
    for tail in [
        vec![],
        vec!["alpha", "beta"],
        vec!["alpha", "--name", "alpha"],
        vec!["--name", "alpha", "--panel", id],
        vec!["--session", "sess-alpha", "--name", "alpha"],
        vec!["alpha", "--current"],
        vec!["alpha", "--direction", "left"],
        vec!["alpha", "--wait"],
        vec!["alpha", "--timeout=0"],
        vec!["alpha", "--client=invalid"],
    ] {
        let mut words = vec!["agent", "focus"];
        words.extend(tail);
        assert!(parse(&words).is_err(), "{words:?}");
    }
    for words in [["agent", "focus", "--help"], ["help", "agent", "focus"]] {
        let Request::Help(topic) = parse(&words).unwrap() else {
            panic!("help");
        };
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let help = locale.cli_help(topic);
            assert!(help.contains("orchbee agent focus"));
            assert!(help.contains("30000"));
            assert!(help.contains(match locale {
                Locale::EnUs => "visible paint",
                Locale::ZhCn => "成功绘制",
            }));
        }
    }
}

#[test]
fn panel_swap_grammar_selects_one_source_and_one_destination_with_one_budget() {
    let parse = |words: &[&str]| invocation::parse_request(words.iter().map(OsString::from));
    let id = "00000000000000000000000000000007";
    for source in [
        vec![],
        vec![id],
        vec!["--panel", "@panel:07"],
        vec!["--current"],
    ] {
        for destination in [vec!["--with", "@panel:08"], vec!["--direction", "up"]] {
            let mut words = vec!["panel", "swap"];
            words.extend(source.clone());
            words.extend(destination);
            let Request::Client {
                command: ClientCommand::Swap(command),
                ..
            } = parse(&words).unwrap()
            else {
                panic!("swap");
            };
            assert_eq!(command.timeout_ms, 30_000);
            assert!(command.client.is_none());
            words.extend(["--client", id, "--timeout=3600000", "--format=json"]);
            let Request::Client {
                command: ClientCommand::Swap(command),
                format,
            } = parse(&words).unwrap()
            else {
                panic!("swap");
            };
            assert_eq!(command.timeout_ms, 3_600_000);
            assert_eq!(command.client.unwrap().as_str(), id);
            assert_eq!(format, OutputFormat::Json);
        }
    }
    for tail in [
        vec![],
        vec!["--direction", "above"],
        vec!["--with", "label:name"],
        vec!["--with", id, "--direction", "up"],
        vec![id, "--panel", id, "--with", id],
        vec![id, "--current", "--with", id],
        vec!["--panel", id, "--current", "--with", id],
        vec![id, id, "--with", id],
        vec!["--with", id, "--detach"],
        vec!["--with", id, "--request-id", id],
        vec!["--with", id, "--timeout=0"],
        vec!["--with", id, "--timeout=3600001"],
        vec!["--with", id, "--client=bad"],
        vec!["--with", id, "--with", id],
    ] {
        let mut words = vec!["panel", "swap"];
        words.extend(tail);
        assert!(parse(&words).is_err(), "{words:?}");
    }
    for words in [["panel", "swap", "--help"], ["help", "panel", "swap"]] {
        assert!(matches!(parse(&words).unwrap(), Request::Help(_)));
    }
    assert!(parse(&["panel", "rename", id, "--with", id]).is_err());
}

#[test]
fn client_control_grammar_validates_all_paths_and_conflicts_before_runtime() {
    let parse = |words: &[&str]| invocation::parse_request(words.iter().map(OsString::from));
    let id = "00000000000000000000000000000007";
    for words in [
        vec!["client", "list"],
        vec!["client", "get", id],
        vec!["client", "control-status", id],
        vec!["workspace", "focus", "label:demo"],
        vec!["tab", "focus", "label:shell"],
        vec!["panel", "focus"],
        vec!["panel", "zoom"],
    ] {
        assert!(
            matches!(parse(&words).unwrap(), Request::Client { .. }),
            "{words:?}"
        );
        let mut help = words[..2].to_vec();
        help.push("--help");
        assert!(matches!(parse(&help).unwrap(), Request::Help(_)));
    }
    let Request::Client {
        command:
            ClientCommand::Navigate {
                target,
                client,
                operation_id,
                detach,
                timeout_ms,
            },
        format,
    } = parse(&[
        "panel",
        "zoom",
        "--current",
        "--toggle",
        "--client",
        id,
        "--request-id",
        id,
        "--detach",
        "--timeout=1",
        "--format=json",
    ])
    .unwrap()
    else {
        panic!("navigate");
    };
    assert_eq!(
        target,
        ClientNavigationTarget::Zoom {
            target: None,
            mode: ZoomMode::Toggle
        }
    );
    assert_eq!(client.unwrap().as_str(), id);
    assert_eq!(operation_id.unwrap().as_str(), id);
    assert!(detach);
    assert_eq!(timeout_ms, 1);
    assert_eq!(format, OutputFormat::Json);
    for words in [
        vec!["client"],
        vec!["client", "get"],
        vec!["client", "list", "extra"],
        vec!["client", "get", id, "extra"],
        vec!["client", "control-status", "0"],
        vec!["workspace", "focus"],
        vec!["tab", "focus"],
        vec!["panel", "zoom", "--on", "--off"],
        vec!["panel", "zoom", "--toggle", "--on"],
        vec!["panel", "zoom", "--off", "--toggle"],
        vec!["panel", "focus", id, "--current"],
        vec!["panel", "focus", id, "--panel", id],
        vec!["panel", "focus", "--panel", id, "--current"],
        vec!["panel", "zoom", "--timeout=0"],
        vec!["panel", "focus", "--timeout=3600001"],
        vec!["tab", "focus", "label:a", "--on"],
        vec![
            "panel",
            "focus",
            "--client=00000000000000000000000000000000",
        ],
        vec!["panel", "zoom", "--client=ABC"],
        vec!["client", "list", "--client", id],
        vec!["panel", "zoom", "--request-id=invalid"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn client_control_selection_is_exact_fresh_and_never_guesses_between_clients() {
    let first = record();
    let mut second = first.clone();
    second.client_id = WorkbenchClientId::parse("00000000000000000000000000000009").unwrap();
    second.view.selection.as_mut().unwrap().panel_id = Some(
        orchbee_workbench_protocol::WorkbenchPanelId::parse("0000000000000000000000000000000a")
            .unwrap(),
    );
    let clients = vec![first.clone(), second.clone()];
    assert_eq!(
        selection::select(&[], None, None).unwrap_err().kind,
        FailureKind::NotFound
    );
    assert_eq!(
        selection::select(std::slice::from_ref(&first), None, None)
            .unwrap()
            .client_id,
        first.client_id
    );
    assert_eq!(
        selection::select(&clients, None, None).unwrap_err().kind,
        FailureKind::Ambiguous
    );
    let caller = Target::Id(
        first
            .view
            .selection
            .as_ref()
            .unwrap()
            .panel_id
            .as_ref()
            .unwrap()
            .to_string(),
    );
    assert_eq!(
        selection::select(&clients, None, Some(&caller))
            .unwrap()
            .client_id,
        first.client_id
    );
    assert_eq!(
        selection::select(&clients, Some(&second.client_id), Some(&caller))
            .unwrap()
            .client_id,
        second.client_id
    );
    second.availability = ClientViewAvailability::Stale;
    assert_eq!(
        selection::select(std::slice::from_ref(&second), Some(&second.client_id), None)
            .unwrap_err()
            .kind,
        FailureKind::ServiceUnavailable
    );
    assert_eq!(
        selection::select(&[first.clone(), second], None, None)
            .unwrap()
            .client_id,
        first.client_id
    );
    let mut inactive = first;
    inactive.view.selection = None;
    assert!(matches!(
        selection::intent(
            &ClientNavigationTarget::Zoom {
                target: Some(caller),
                mode: ZoomMode::Toggle
            },
            &inactive,
            None,
            &snapshot()
        )
        .unwrap(),
        ClientViewIntent::ToggleZoom { .. }
    ));
}

#[test]
fn client_control_documents_preserve_every_receipt_and_validate_exported_schemas() {
    let control_schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.client.control.json")).unwrap();
    let view_schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.client.view.json")).unwrap();
    let control_validator = jsonschema::draft202012::new(&control_schema).unwrap();
    let view_validator = jsonschema::draft202012::new(&view_schema).unwrap();
    for entry in corpus()["valid"].as_array().unwrap() {
        if entry["direction"] != "response" {
            continue;
        }
        let WorkbenchResponse::ClientView(reply) =
            serde_json::from_value(entry["document"].clone()).unwrap()
        else {
            panic!("reply");
        };
        let (operation_id, validator) = match &reply.outcome {
            ClientViewOutcome::Synchronized { .. } => continue,
            ClientViewOutcome::Operation { operation } => (
                Some(operation.command.operation_id.clone()),
                &control_validator,
            ),
            _ => (None, &view_validator),
        };
        if matches!(reply.outcome, ClientViewOutcome::Conflict {}) {
            continue;
        }
        let answer = Answer {
            status_only: false,
            operation_id,
            site_id: Some(reply.site_id.clone()),
            reply: Some(reply),
            unknown: false,
            wait: None,
        };
        let output = emit(&answer, OutputFormat::Json, Locale::EnUs);
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            validator.is_valid(&document),
            "{}: {document}",
            entry["name"]
        );
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let text = emit(&answer, OutputFormat::Text, locale);
            assert_eq!(text.status, output.status);
            assert!(!text.stdout.is_empty());
        }
    }
    for (wait, exit) in [
        (None, 8),
        (Some(Wait::Detached), 8),
        (Some(Wait::TimedOut), 7),
        (Some(Wait::Cancelled), 130),
        (Some(Wait::TransportLost), 5),
        (Some(Wait::ObservationFailed), 1),
    ] {
        let answer = Answer {
            status_only: false,
            operation_id: Some(
                WorkbenchRequestId::parse("00000000000000000000000000000003").unwrap(),
            ),
            site_id: None,
            reply: None,
            unknown: true,
            wait,
        };
        assert_eq!(answer.exit_code(), exit);
        assert!(!answer.pending());
        control_validator.validate(&answer.document()).unwrap();
        for status_only in [false, true] {
            let answer = Answer {
                unknown: false,
                status_only,
                ..answer_without_reply(wait)
            };
            control_validator.validate(&answer.document()).unwrap();
            assert_eq!(
                answer.document()["observation"]["kind"],
                if status_only {
                    "unobserved"
                } else {
                    "not-submitted"
                }
            );
        }
    }
}
fn answer_without_reply(wait: Option<Wait>) -> Answer {
    Answer {
        status_only: false,
        operation_id: Some(WorkbenchRequestId::parse("00000000000000000000000000000003").unwrap()),
        site_id: None,
        reply: None,
        unknown: false,
        wait,
    }
}

#[test]
fn client_control_routing_failures_are_closed_schema_documents() {
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/orchbee.cli.failure.client-control.json"
    ))
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for (kind, reason, candidates) in [
        (FailureKind::NotFound, "client-not-found", vec![]),
        (
            FailureKind::NotFound,
            "client-panel-unavailable",
            vec![record().client_id.to_string()],
        ),
        (
            FailureKind::ServiceUnavailable,
            "client-stale",
            vec![record().client_id.to_string()],
        ),
        (
            FailureKind::Ambiguous,
            "client-ambiguous",
            vec![
                record().client_id.to_string(),
                "00000000000000000000000000000009".into(),
            ],
        ),
    ] {
        let output = failure_output(
            selection::failure(kind, reason, candidates),
            Some(OutputFormat::Json),
            Locale::EnUs,
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validator.validate(&document).unwrap();
    }
}

#[test]
fn layout_grammar_accepts_current_targets_and_rejects_direction_or_target_ambiguity() {
    use crate::invocation::LayoutQuery;
    use orchbee_workbench_protocol::ClientPanelDirection;
    let parse = |words: &[&str]| invocation::parse_request(words.iter().map(OsString::from));
    for verb in ["layout", "neighbor", "edges"] {
        let mut words = vec!["panel", verb, "--current"];
        if verb == "neighbor" {
            words.extend(["--direction", "up"]);
        }
        let Request::Client {
            command: ClientCommand::Navigate { target, .. },
            ..
        } = parse(&words).unwrap()
        else {
            panic!("layout query");
        };
        assert!(matches!(target, ClientNavigationTarget::Layout { .. }));
        assert!(matches!(
            parse(&["help", "panel", verb]).unwrap(),
            Request::Help(_)
        ));
    }
    let Request::Client {
        command: ClientCommand::Navigate { target, .. },
        ..
    } = parse(&["--direction=left", "panel", "neighbor"]).unwrap()
    else {
        panic!("neighbor");
    };
    assert_eq!(
        target,
        ClientNavigationTarget::Layout {
            target: None,
            query: LayoutQuery::Neighbor(ClientPanelDirection::Left)
        }
    );
    for words in [
        vec!["panel", "neighbor"],
        vec!["panel", "neighbor", "--direction", "above"],
        vec!["panel", "layout", "--direction", "right"],
        vec!["panel", "edges", "--on"],
        vec![
            "panel",
            "layout",
            "00000000000000000000000000000006",
            "--current",
        ],
        vec![
            "panel",
            "neighbor",
            "--panel",
            "00000000000000000000000000000006",
            "--current",
            "--direction",
            "up",
        ],
        vec!["panel", "edges", "--timeout", "0"],
        vec!["workspace", "layout", "label:x"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn layout_queries_keep_exact_routing_and_emit_closed_locale_independent_documents() {
    use crate::invocation::LayoutQuery;
    use orchbee_workbench_protocol::ClientPanelDirection;
    let target = ClientNavigationTarget::Layout {
        target: None,
        query: LayoutQuery::Edges,
    };
    assert!(matches!(
        selection::intent(&target, &record(), None, &snapshot()).unwrap(),
        ClientViewIntent::Layout { .. }
    ));
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.panel.layout.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for name in [
        "layout-read",
        "inactive-layout-read",
        "hidden-source-layout-read",
    ] {
        let WorkbenchResponse::ClientView(reply) = serde_json::from_value(sample(name)).unwrap()
        else {
            panic!("view result");
        };
        let mut answer = answer_without_reply(None);
        answer.site_id = Some(reply.site_id.clone());
        answer.reply = Some(reply);
        for query in [
            LayoutQuery::Layout,
            LayoutQuery::Neighbor(ClientPanelDirection::Right),
            LayoutQuery::Edges,
        ] {
            let en = layout::emit(&answer, query, OutputFormat::Json, Locale::EnUs);
            let zh = layout::emit(&answer, query, OutputFormat::Json, Locale::ZhCn);
            assert_eq!(en.stdout, zh.stdout);
            let document: Value = serde_json::from_slice(&en.stdout).unwrap();
            validator.validate(&document).unwrap();
            match query {
                LayoutQuery::Layout => {
                    assert_eq!(document["projection"], serde_json::json!({"kind":"layout"}))
                }
                LayoutQuery::Neighbor(_) => {
                    assert_eq!(document["projection"]["direction"], "right");
                    assert!(document["projection"].get("neighborPanelId").is_some());
                }
                LayoutQuery::Edges => {
                    assert_eq!(
                        document["projection"]["sourceAvailable"],
                        name != "hidden-source-layout-read"
                    );
                    for direction in ["left", "right", "up", "down"] {
                        assert_eq!(
                            document["projection"][direction].is_null(),
                            name == "hidden-source-layout-read"
                        );
                    }
                }
            }
            let hidden_query =
                name == "hidden-source-layout-read" && !matches!(query, LayoutQuery::Layout);
            assert_eq!(en.status, if hidden_query { 3 } else { 0 });
            let text = layout::emit(&answer, query, OutputFormat::Text, Locale::ZhCn);
            assert!(!text.stdout.is_empty());
            assert_eq!(text.status, en.status);
        }
    }
    for wait in [
        Some(Wait::TimedOut),
        Some(Wait::Cancelled),
        Some(Wait::Detached),
        None,
    ] {
        let output = layout::emit(
            &answer_without_reply(wait),
            LayoutQuery::Layout,
            OutputFormat::Json,
            Locale::EnUs,
        );
        validator
            .validate(&serde_json::from_slice::<Value>(&output.stdout).unwrap())
            .unwrap();
    }
}

#[test]
fn directional_focus_grammar_resolves_source_and_explicit_success_variants() {
    use orchbee_workbench_protocol::ClientPanelDirection;
    let parse = |words: &[&str]| invocation::parse_request(words.iter().map(OsString::from));
    for (word, direction) in [
        ("left", ClientPanelDirection::Left),
        ("right", ClientPanelDirection::Right),
        ("up", ClientPanelDirection::Up),
        ("down", ClientPanelDirection::Down),
    ] {
        for args in [
            vec!["panel", "focus", "--direction", word],
            vec!["panel", "focus", "--current", "--direction", word],
            vec![
                "--direction",
                word,
                "panel",
                "focus",
                "--panel",
                "@panel:06",
            ],
        ] {
            let Request::Client {
                command: ClientCommand::Navigate { target, .. },
                ..
            } = parse(&args).unwrap()
            else {
                panic!("client");
            };
            assert!(
                matches!(&target,ClientNavigationTarget::FocusDirection{direction:d,..} if *d==direction)
            );
            let intent = selection::intent(&target, &record(), None, &snapshot()).unwrap();
            assert!(
                matches!(intent,ClientViewIntent::FocusDirection{direction:d,..} if d==direction)
            );
        }
    }
    for args in [
        vec!["panel", "focus", "--direction", "above"],
        vec!["panel", "focus", "--direction"],
        vec!["panel", "focus", "--direction", "up", "--direction", "down"],
        vec!["panel", "focus", "--direction", "up", "--on"],
        vec![
            "panel",
            "focus",
            "--current",
            "--panel",
            "@panel:06",
            "--direction",
            "up",
        ],
        vec!["tab", "focus", "shell", "--direction", "up"],
        vec!["workspace", "focus", "main", "--direction", "up"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.client.control.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for (name, en, zh) in [
        (
            "direction-focused",
            "Neighbor focused and drawn",
            "已聚焦相邻面板并绘制",
        ),
        (
            "direction-unchanged",
            "No neighbor; selection unchanged",
            "该方向没有相邻面板，选择未改变",
        ),
    ] {
        let WorkbenchResponse::ClientView(reply) = serde_json::from_value(sample(name)).unwrap()
        else {
            panic!("reply");
        };
        let ClientViewOutcome::Operation { operation } = &reply.outcome else {
            panic!("operation");
        };
        let answer = Answer {
            status_only: false,
            operation_id: Some(operation.command.operation_id.clone()),
            site_id: Some(reply.site_id.clone()),
            reply: Some(reply),
            unknown: false,
            wait: None,
        };
        assert_eq!(answer.exit_code(), 0);
        assert!(validator.is_valid(&answer.document()));
        assert!(
            String::from_utf8(emit(&answer, OutputFormat::Text, Locale::EnUs).stdout)
                .unwrap()
                .contains(en)
        );
        assert!(
            String::from_utf8(emit(&answer, OutputFormat::Text, Locale::ZhCn).stdout)
                .unwrap()
                .contains(zh)
        );
    }
}

#[test]
fn ratio_and_resize_grammar_normalizes_finite_values_and_resolves_exact_targets() {
    use crate::invocation::ClientLayoutTarget;
    let parse = |args: &[&str]| invocation::parse_request(args.iter().map(OsString::from));
    for (value, expected) in [("0", 0.0), ("-0.05", 0.05), ("2", 0.5)] {
        let Request::Client {
            command: ClientCommand::Navigate { target, .. },
            ..
        } = parse(&[
            "panel",
            "resize",
            "--current",
            "--direction",
            "right",
            "--amount",
            value,
        ])
        .unwrap()
        else {
            panic!()
        };
        let intent = selection::intent(&target, &record(), None, &snapshot()).unwrap();
        assert!(matches!(intent,ClientViewIntent::Resize {amount,..} if amount.value()==expected));
    }
    for (value, expected) in [("-1", 0.1), ("0.25", 0.25), ("2", 0.9)] {
        for selector in [
            vec![],
            vec!["--current"],
            vec!["--panel", "@panel:06"],
            vec!["--tab", "shell"],
        ] {
            let mut words = vec![
                "layout",
                "set-split-ratio",
                "--path",
                "0.1",
                "--ratio",
                value,
            ];
            words.extend(selector);
            let Request::Client {
                command: ClientCommand::Navigate { target, .. },
                ..
            } = parse(&words).unwrap()
            else {
                panic!()
            };
            let intent = selection::intent(&target, &record(), None, &snapshot()).unwrap();
            assert!(
                matches!(intent,ClientViewIntent::SetSplitRatio {path,ratio,tab_id,..} if path.indices()==[0,1] && ratio.value()==expected && tab_id.as_str()==format!("{:032x}",5))
            );
        }
    }
    let Request::Client {
        command: ClientCommand::Navigate { target, .. },
        ..
    } = parse(&[
        "layout",
        "set-split-ratio",
        "--path",
        "root",
        "--ratio",
        ".5",
        "--tab",
        "shell",
    ])
    .unwrap()
    else {
        panic!()
    };
    assert!(
        matches!(target,ClientNavigationTarget::SetSplitRatio {target:ClientLayoutTarget::Tab(_),path,..} if path.indices().is_empty())
    );
    for words in [
        vec!["panel", "resize"],
        vec!["panel", "resize", "--direction", "above"],
        vec!["panel", "resize", "--direction", "up", "--amount", "NaN"],
        vec!["panel", "resize", "--direction", "up", "--amount", "inf"],
        vec!["panel", "resize", "--direction", "up", "--ratio", ".5"],
        vec!["panel", "focus", "--amount", ".5"],
        vec!["layout"],
        vec!["layout", "set-split-ratio"],
        vec![
            "layout",
            "set-split-ratio",
            "--path",
            "root",
            "--ratio",
            "-inf",
        ],
        vec![
            "layout",
            "set-split-ratio",
            "--path",
            "0..1",
            "--ratio",
            ".5",
        ],
        vec!["layout", "set-split-ratio", "--path", "2", "--ratio", ".5"],
        vec![
            "layout",
            "set-split-ratio",
            "--path",
            "0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0",
            "--ratio",
            ".5",
        ],
        vec![
            "layout",
            "set-split-ratio",
            "shell",
            "--path",
            "root",
            "--ratio",
            ".5",
        ],
        vec![
            "layout",
            "set-split-ratio",
            "--tab",
            "shell",
            "--current",
            "--path",
            "root",
            "--ratio",
            ".5",
        ],
        vec![
            "layout",
            "set-split-ratio",
            "--tab",
            "shell",
            "--panel",
            "@panel:06",
            "--path",
            "root",
            "--ratio",
            ".5",
        ],
        vec![
            "layout",
            "set-split-ratio",
            "--path",
            "root",
            "--ratio",
            ".5",
            "--amount",
            ".1",
        ],
        vec!["status", "--ratio", ".5"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    for words in [
        vec!["layout", "--help"],
        vec!["layout", "set-split-ratio", "--help"],
        vec!["panel", "resize", "--help"],
    ] {
        assert!(matches!(parse(&words).unwrap(), Request::Help(_)));
    }
}

#[test]
fn layout_adjustment_receipts_have_closed_json_and_honest_bilingual_text() {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.client.control.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for (name, en, zh) in [
        (
            "resize-adjusted-selected",
            "Ratio changed and drawn",
            "比例已改变并绘制",
        ),
        (
            "resize-adjusted-inactive-old-selected-view",
            "Ratio stored for inactive Tab",
            "已保存非活动标签页的比例",
        ),
        (
            "resize-no-split",
            "No eligible split; unchanged",
            "没有可调整的分割",
        ),
        ("resize-clamped-unchanged", "Ratio unchanged", "比例未改变"),
        (
            "ratio-set-root",
            "Ratio changed and drawn",
            "比例已改变并绘制",
        ),
    ] {
        let WorkbenchResponse::ClientView(reply) = serde_json::from_value(sample(name)).unwrap()
        else {
            panic!()
        };
        let ClientViewOutcome::Operation { operation } = &reply.outcome else {
            panic!()
        };
        let answer = Answer {
            status_only: false,
            operation_id: Some(operation.command.operation_id.clone()),
            site_id: Some(reply.site_id.clone()),
            reply: Some(reply),
            unknown: false,
            wait: None,
        };
        assert_eq!(answer.exit_code(), 0);
        validator.validate(&answer.document()).unwrap();
        for (locale, expected) in [(Locale::EnUs, en), (Locale::ZhCn, zh)] {
            assert!(
                String::from_utf8(emit(&answer, OutputFormat::Text, locale).stdout)
                    .unwrap()
                    .contains(expected),
                "{name}"
            );
        }
    }
}
