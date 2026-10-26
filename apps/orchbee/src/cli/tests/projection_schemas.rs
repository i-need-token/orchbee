use crate::agent_projection::tests::{rows, state, summary};
use crate::cli::{
    agent_projection::{Item, List},
    results::Projection,
};
use crate::invocation::{OutputFormat, QueryCommand};
use crate::localization::Locale;
use orchbee_agent_observation_protocol::{AgentAttachmentGeneration, AgentSafeInteger};
use orchbee_workbench_protocol::{WorkbenchClientId, WorkbenchRevision};
use serde_json::{Map, Value, json};

fn listed(rows: Vec<crate::agent_projection::Row>, with_client: bool) -> Value {
    let snapshot = crate::machine_agents::tests::fixture();
    let checkpoint = crate::agent_projection::read::Checkpoint {
        canonical_revision: AgentSafeInteger::new(2),
        report_revision: WorkbenchRevision::new(7),
        rows: rows.clone(),
    };
    let client_id =
        with_client.then(|| WorkbenchClientId::parse("00000000000000000000000000000001").unwrap());
    let items = rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| Item {
            ordinal: index + 1,
            row,
            panel_handle: None,
            cwd: None,
            client: with_client.then(|| json!({"canonical":null,"terminal":null})),
        })
        .collect();
    let list = List {
        items,
        revision: AgentSafeInteger::new(2),
        next: None,
        client: client_id.clone(),
    };
    let command = QueryCommand::AgentList {
        client: client_id,
        include_ended: false,
        limit: 50,
        cursor: None,
    };
    let output = crate::cli::success_output(
        Projection::UnifiedAgentList(Box::new(list)),
        crate::cli::agent_projection::source(&snapshot, &checkpoint),
        &command,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 0);
    serde_json::from_slice(&output.stdout).expect("unified Agent list envelope")
}

fn row_document(row: &crate::agent_projection::Row) -> Value {
    let mut document = row.document(1, None);
    document
        .as_object_mut()
        .expect("Agent row object")
        .remove("ordinal");
    document
}

fn definition_accepts(schema_document: &str, definition: &str, value: &Value) -> bool {
    let schema: Value = serde_json::from_str(schema_document).expect("schema parses");
    let focused = json!({
        "$schema": schema["$schema"],
        "$defs": schema["$defs"],
        "$ref": format!("#/$defs/{definition}"),
    });
    jsonschema::draft202012::new(&focused)
        .expect("focused schema compiles")
        .is_valid(value)
}

#[test]
fn real_projection_rows_validate_with_each_identity_and_client_selection() {
    let list_schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.agent.list.json"))
            .expect("Agent list schema parses");
    let list_validator =
        jsonschema::draft202012::new(&list_schema).expect("Agent list schema compiles");

    let terminal_only = rows(Vec::new(), vec![state(12, Some("codex"))]);
    let both = rows(
        vec![summary("canonical-both", 12, "codex")],
        vec![state(12, Some("codex"))],
    );
    let canonical_placed = rows(
        vec![summary("canonical-placed", 13, "codex")],
        vec![state(13, None)],
    );
    let canonical_unplaced = rows(vec![summary("canonical-unplaced", 99, "codex")], vec![]);

    for (name, projection) in [
        ("terminal-only", terminal_only.clone()),
        ("both", both.clone()),
        ("canonical with witness placement", canonical_placed.clone()),
        ("canonical without placement", canonical_unplaced.clone()),
    ] {
        let document = listed(projection, false);
        assert!(
            list_validator.is_valid(&document),
            "{name} list output must validate: {document}"
        );
        assert!(document["result"]["clientId"].is_null());
        assert!(document["result"]["items"][0]["client"].is_null());
    }

    let client_document = listed(terminal_only.clone(), true);
    assert!(
        list_validator.is_valid(&client_document),
        "nullable independent Client facets must validate: {client_document}"
    );
    assert_eq!(
        client_document["result"]["items"][0]["client"],
        json!({"canonical":null,"terminal":null})
    );
    let mut missing_client_id = client_document.clone();
    missing_client_id["result"]
        .as_object_mut()
        .unwrap()
        .remove("clientId");
    assert!(!list_validator.is_valid(&missing_client_id));
    let mut missing_row_client = client_document;
    missing_row_client["result"]["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("client");
    assert!(!list_validator.is_valid(&missing_row_client));

    let mut terminal_with_canonical = listed(terminal_only.clone(), false);
    terminal_with_canonical["result"]["items"][0]["canonical"] =
        both[0].canonical().map_or(Value::Null, |row| json!(row));
    assert!(!list_validator.is_valid(&terminal_with_canonical));
    let mut both_without_terminal = listed(both, false);
    both_without_terminal["result"]["items"][0]["terminal"] = Value::Null;
    assert!(!list_validator.is_valid(&both_without_terminal));
    let mut terminal_without_placement =
        listed(rows(Vec::new(), vec![state(12, Some("codex"))]), false);
    terminal_without_placement["result"]["items"][0]["placement"] = Value::Null;
    assert!(!list_validator.is_valid(&terminal_without_placement));

    let get_schema = include_str!("../schemas/orchbee.agent.get.json");
    let both_for_get = rows(
        vec![summary("canonical-both", 12, "codex")],
        vec![state(12, Some("codex"))],
    );
    for (name, row) in [
        ("terminal-only", &terminal_only[0]),
        ("both", &both_for_get[0]),
        ("canonical with witness placement", &canonical_placed[0]),
        ("canonical without placement", &canonical_unplaced[0]),
    ] {
        let projected = row_document(row);
        assert!(
            definition_accepts(get_schema, "projectedAgent", &projected),
            "{name} get projection must validate: {projected}"
        );
    }
    let mut impossible_get = row_document(&canonical_placed[0]);
    impossible_get["terminal"] = row_document(&terminal_only[0])["terminal"].clone();
    assert!(!definition_accepts(
        get_schema,
        "projectedAgent",
        &impossible_get
    ));
}

#[test]
fn opaque_launch_generation_is_accepted_in_list_and_failure_bindings() {
    let mut physical = state(12, Some("codex"));
    physical.binding.generation =
        AgentAttachmentGeneration::parse("attachment-generation-12").unwrap();
    let terminal = rows(Vec::new(), vec![physical]);
    let document = listed(terminal.clone(), false);
    let list_schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.agent.list.json"))
            .expect("Agent list schema parses");
    let list_validator =
        jsonschema::draft202012::new(&list_schema).expect("Agent list schema compiles");
    assert!(list_validator.is_valid(&document), "{document}");

    let row = row_document(&terminal[0]);
    assert!(definition_accepts(
        include_str!("../schemas/orchbee.agent.get.json"),
        "projectedAgent",
        &row
    ));
    assert!(definition_accepts(
        include_str!("../schemas/orchbee.agent.get.json"),
        "physical_terminalBinding",
        &row["terminal"]["binding"]
    ));

    let mut details = Map::new();
    details.insert("stage".into(), json!("agent-projection"));
    details.insert("reason".into(), json!("target-ambiguous"));
    details.insert("selector".into(), json!("@agent:codex"));
    details.insert(
        "candidates".into(),
        json!([{
            "selector":"@agent:1",
            "sessionId":null,
            "panelId":row["placement"]["panelId"],
            "binding":row["terminal"]["binding"],
        }]),
    );
    let failure = crate::cli::Failure::new(crate::cli::FailureKind::Ambiguous)
        .failure_schema("orchbee.cli.failure.agent-projection")
        .details(details);
    let output = crate::cli::failure_output(failure, Some(OutputFormat::Json), Locale::EnUs);
    let document: Value = serde_json::from_slice(&output.stdout).expect("Agent failure envelope");
    let failure_schema: Value = serde_json::from_str(include_str!(
        "../schemas/orchbee.cli.failure.agent-projection.json"
    ))
    .expect("Agent projection failure schema parses");
    let failure_validator = jsonschema::draft202012::new(&failure_schema)
        .expect("Agent projection failure schema compiles");
    assert!(
        failure_validator.is_valid(&document),
        "opaque candidate binding must validate: {document}"
    );
}
