use super::*;
use crate::invocation::Target;
use orchbee_workbench_protocol::{ClientViewSequence, WorkbenchResponse};

fn id(number: u64) -> String {
    format!("{number:032x}")
}

#[test]
fn read_failures_preserve_denial_missing_observation_and_interruption_classes() {
    assert_eq!(
        submission_failure(3, Some(&ClientViewOutcome::Denied {})),
        Some(FailureKind::Denied)
    );
    assert_eq!(
        submission_failure(3, Some(&ClientViewOutcome::NotFound {})),
        Some(FailureKind::NotFound)
    );
    for (code, kind) in [
        (1, FailureKind::ProtocolInvalid),
        (3, FailureKind::Rejected),
        (4, FailureKind::ServiceUnavailable),
        (5, FailureKind::TransportLoss),
        (7, FailureKind::Timeout),
        (8, FailureKind::TransportLoss),
        (130, FailureKind::Cancelled),
    ] {
        assert_eq!(submission_failure(code, None), Some(kind));
    }
    assert_eq!(submission_failure(0, None), None);
}
fn record() -> ClientViewRecord {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let sample = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "found")
        .unwrap()["document"]
        .clone();
    let WorkbenchResponse::ClientView(reply) = serde_json::from_value(sample).unwrap() else {
        panic!()
    };
    let ClientViewOutcome::Found { client } = reply.outcome else {
        panic!()
    };
    let mut client = *client;
    client.view.selection.as_mut().unwrap().panel_count = 1;
    client
}
fn snapshot() -> Snapshot {
    let panel = |p| json!({"kind":"panel","panelId":id(p),"primaryPresentation":{"presentationId":id(p+1),"label":"中文\u{1b}[31m","binding":null},"childPresentations":[]});
    let tab =
        |t, p| json!({"tabId":id(t),"label":"shell","pinned":false,"groupId":null,"root":panel(p)});
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":"12",
        "navigation":{"projects":[{"projectId":id(20),"label":"project","directories":[{"directoryId":id(23),"workerId":id(24),"path":"C:\\work"}],"workspaces":[{"workspaceId":id(21),"label":"work","primaryDirectoryId":id(23),"terminalContextId":id(9)},{"workspaceId":id(22),"label":"empty","primaryDirectoryId":id(23),"terminalContextId":id(19)}]}],"standaloneTerminalContextId":id(4)},
        "terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[tab(5,6)]},{"terminalContextId":id(9),"groups":[],"tabs":[tab(10,11),tab(13,14)]},{"terminalContextId":id(19),"groups":[],"tabs":[]}]})).unwrap()
}
fn document(command: &QueryCommand, state: &Snapshot) -> Document {
    Document {
        projection: Box::new(results::project(command, state).unwrap()),
        client: record(),
        operations: vec![WorkbenchRequestId::parse(&id(30)).unwrap()],
        contexts: [(4, Some(5)), (9, Some(13)), (19, None)]
            .into_iter()
            .map(|(context, tab)| ClientContextSelection {
                terminal_context_id: WorkbenchTerminalContextId::parse(&id(context)).unwrap(),
                active_tab_id: tab.map(|tab| {
                    orchbee_workbench_protocol::WorkbenchTabId::parse(&id(tab)).unwrap()
                }),
            })
            .collect(),
    }
}

#[test]
fn selected_queries_publish_global_focus_and_client_memory_with_closed_schemas() {
    let state = snapshot();
    for (command, key) in [
        (QueryCommand::WorkspaceList { project: None }, "workspaces"),
        (QueryCommand::WorkspaceGet(Target::Id(id(21))), "workspace"),
        (
            QueryCommand::TabList {
                workspace: None,
                standalone: false,
            },
            "tabs",
        ),
        (QueryCommand::TabGet(Target::Id(id(5))), "tab"),
        (QueryCommand::PanelList { tab: None }, "panels"),
        (QueryCommand::PanelGet(Target::Id(id(6))), "panel"),
    ] {
        let published = crate::cli::schema::answer(
            Some(results::schema_id(&command)),
            OutputFormat::Json,
            Locale::EnUs,
        );
        let catalog: Value = serde_json::from_slice(&published.stdout).unwrap();
        let validator =
            jsonschema::validator_for(&catalog["result"]["schema"]["document"]).unwrap();
        for selected in [false, true] {
            let projection = if selected {
                Projection::ClientWorkbench(Box::new(document(&command, &state)))
            } else {
                results::project(&command, &state).unwrap()
            };
            let output = success_output(
                projection,
                results::snapshot_source(&state),
                &command,
                OutputFormat::Json,
                Locale::EnUs,
            );
            assert_eq!(output.status, 0);
            let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
            validator.validate(&doc).unwrap();
            assert_eq!(doc["result"].get("client").is_some(), selected);
            if !selected {
                continue;
            }
            let value = &doc["result"][key];
            if key == "workspace" {
                assert_eq!(value["focused"], false);
                assert_eq!(value["activeTabId"], id(13));
            }
            if key == "workspaces" {
                assert_eq!(value[0]["activeTabId"], id(13));
                assert_eq!(value[1]["activeTabId"], Value::Null);
            }
            if key == "tabs" || key == "panels" {
                assert_eq!(
                    value
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|row| row["focused"] == true)
                        .count(),
                    1
                );
            }
            if key == "tab" || key == "panel" {
                assert_eq!(value["focused"], true);
            }
            let mut missing_provenance = doc.clone();
            missing_provenance["result"]
                .as_object_mut()
                .unwrap()
                .remove("client");
            assert!(!validator.is_valid(&missing_provenance));
            let mut missing_focus = doc.clone();
            let row = &mut missing_focus["result"][key];
            if let Some(rows) = row.as_array_mut() {
                rows[0].as_object_mut().unwrap().remove("focused");
            } else {
                row.as_object_mut().unwrap().remove("focused");
            }
            assert!(!validator.is_valid(&missing_focus));
        }
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let output = success_output(
                Projection::ClientWorkbench(Box::new(document(&command, &state))),
                results::snapshot_source(&state),
                &command,
                OutputFormat::Text,
                locale,
            );
            assert_eq!(output.status, 0);
            assert!(!output.stdout.contains(&0x1b));
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains(locale.cli_query_client_labels()[0])
            );
        }
    }
}

#[test]
fn client_query_refuses_false_empty_stale_tab_and_wrong_panel_membership() {
    let state = snapshot();
    let command = QueryCommand::WorkspaceList { project: None };
    let mut doc = document(&command, &state);
    verify_membership(&state, &doc.client.view, &doc.contexts).unwrap();
    let requested = requested_contexts(&doc.projection, &doc.client).unwrap();
    assert_eq!(
        requested
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec![id(9), id(19), id(4)]
    );
    doc.contexts[1].active_tab_id = None;
    assert!(verify_membership(&state, &doc.client.view, &doc.contexts).is_err());
    let mut doc = document(&command, &state);
    doc.contexts[1].active_tab_id = doc.contexts[0].active_tab_id.clone();
    assert!(verify_membership(&state, &doc.client.view, &doc.contexts).is_err());
    let mut doc = document(&command, &state);
    doc.client.view.selection.as_mut().unwrap().panel_count = 2;
    assert!(verify_membership(&state, &doc.client.view, &doc.contexts).is_err());
    let mut doc = document(&command, &state);
    doc.client.view.selection.as_mut().unwrap().panel_id =
        Some(orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(14)).unwrap());
    assert!(verify_membership(&state, &doc.client.view, &doc.contexts).is_err());
    assert_eq!(
        doc.client.view_revision,
        ClientViewSequence::new(2).unwrap()
    );
}

#[test]
fn client_query_failure_is_schema_valid_and_localized() {
    let client = record().client_id;
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/orchbee.cli.failure.client-query.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for kind in [
        FailureKind::NotFound,
        FailureKind::ProtocolInvalid,
        FailureKind::Rejected,
        FailureKind::Timeout,
        FailureKind::Cancelled,
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let output = failure_output(failure(kind, &client), Some(OutputFormat::Json), locale);
            validator
                .validate(&serde_json::from_slice::<Value>(&output.stdout).unwrap())
                .unwrap();
            let output = failure_output(failure(kind, &client), Some(OutputFormat::Text), locale);
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
            assert!(!output.stderr.contains("()"));
            assert!(output.stderr.contains("Client"));
        }
    }
}
