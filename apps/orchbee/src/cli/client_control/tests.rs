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
