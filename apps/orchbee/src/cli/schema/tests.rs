//! The published catalog must describe the exact embedded schema dictionary.
use super::DOCUMENTS;
use crate::cli_text::CliText;
use serde_json::Value;
use std::collections::BTreeSet;

#[cfg(windows)]
#[test]
fn json_option_inputs_have_named_definitions_and_executable_examples() {
    use serde_json::json;
    for (schema_id, definition, input, arguments) in [
        (
            "orchbee.events.stream",
            "filter",
            json!({"kind":"event","event":"workspace.created","scope":{"kind":"all"},"condition":{"kind":"any"}}),
            vec!["events", "wait", "--filter"],
        ),
        (
            "orchbee.plugin.runtime",
            "context",
            json!({"selectedText":"example"}),
            vec![
                "plugin",
                "action",
                "invoke",
                "example.plugin.action",
                "--context",
            ],
        ),
    ] {
        let schema: Value = serde_json::from_str(super::document(schema_id).unwrap()).unwrap();
        let input_schema = json!({"$ref":format!("#/$defs/{definition}"),"$defs":schema["$defs"]});
        let validator = jsonschema::validator_for(&input_schema).unwrap();
        validator.validate(&input).unwrap();
        let encoded = serde_json::to_string(&input).unwrap();
        let args = arguments.into_iter().chain([encoded.as_str()]);
        assert!(crate::invocation::parse_request(args.map(std::ffi::OsString::from)).is_ok());
        let mut invalid = input;
        invalid["unknown"] = true.into();
        assert!(!validator.is_valid(&invalid));
    }
}

#[test]
fn notification_configuration_failure_admits_current_closed_details() {
    let schema: Value =
        serde_json::from_str(super::document("orchbee.cli.failure.configuration").unwrap())
            .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for (field, reason) in [
        ("ui.sound", "wrong-type"),
        ("ui.toast", "invalid-delay"),
        ("ui.sound", "too-many-agents"),
    ] {
        let mut document = serde_json::json!({
            "schema": "orchbee.cli.failure.configuration",
            "version": 1,
            "failure": {
                "kind": "check-failed",
                "details": { "field": field, "reason": reason }
            }
        });
        assert!(validator.is_valid(&document), "{field}: {reason}");
        document["failure"]["details"]["extra"] = true.into();
        assert!(!validator.is_valid(&document));
    }
}

#[test]
fn configured_catalog_and_receipts_publish_closed_native_schema() {
    let schema: Value = serde_json::from_str(super::document("orchbee.command").unwrap()).unwrap();
    let command_schema: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-command.schema.json"
    ))
    .unwrap();
    assert_eq!(
        schema["$defs"]["wc_rejectionReason"], command_schema["$defs"]["rejectionReason"],
        "the embedded configured receipt keeps the complete canonical rejection vocabulary"
    );
    let validator = jsonschema::validator_for(&schema).unwrap();
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-configured-command-conformance-v1.json"
    ))
    .unwrap();
    for case in corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["direction"] == "response")
    {
        let mut reply = case["document"].clone();
        reply.as_object_mut().unwrap().remove("kind");
        let mut document = serde_json::json!({"schema":"orchbee.command","version":1,"source":{"kind":"local-runtime"},"operation":"invoke","result":{"reply":reply,"followup":null,"complete":true}});
        validator
            .validate(&document)
            .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        document["result"]["reply"]["program"] = "untrusted".into();
        assert!(!validator.is_valid(&document));
    }
}

#[test]
fn plugin_catalog_requires_current_provenance() {
    use serde_json::json;
    let schema: Value =
        serde_json::from_str(super::document("orchbee.plugin.registry").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut receipt = json!({"schema":"orchbee.plugin.registry","version":1,"source":{"kind":"local-configuration"},"operation":"list","result":{"kind":"catalog","catalog":{"revision":1,"changed":false,"plugins":[{"manifestPath":"C:/source/orchbee-plugin.toml","pluginRoot":"C:/source","enabled":true,"warning":null,"manifest":{"id":"tools","name":"Tools","version":"1","min_orchbee_version":null,"description":null,"platforms":null,"build":[],"startup":[],"actions":[],"events":[],"panes":[],"link_handlers":[]}}]}}});
    assert!(!validator.is_valid(&receipt));
    receipt["result"]["catalog"]["plugins"][0]["source"] = Value::Null;
    assert!(validator.is_valid(&receipt));
    receipt["result"]["catalog"]["plugins"][0]
        .as_object_mut()
        .unwrap()
        .remove("source");
    assert!(!validator.is_valid(&receipt));
}

#[test]
fn catalog_requires_and_admits_every_embedded_schema_exactly_once() {
    let ids = DOCUMENTS.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), DOCUMENTS.len(), "duplicate embedded schema ID");
    for (id, body) in DOCUMENTS {
        let document: Value = serde_json::from_str(body).expect("embedded JSON schema");
        assert_eq!(document["$id"].as_str(), Some(*id));
        jsonschema::validator_for(&document).expect("embedded schema compiles");
    }
    let catalog: Value =
        serde_json::from_str(super::document("orchbee.api.schema").expect("catalog"))
            .expect("JSON");
    for names in [
        &catalog["properties"]["result"]["properties"]["schemas"]["required"],
        &catalog["$defs"]["schemaId"]["enum"],
    ] {
        let names = names.as_array().expect("schema identity array");
        let declared = names
            .iter()
            .map(|value| value.as_str().expect("schema ID"))
            .collect::<BTreeSet<_>>();
        assert_eq!(declared.len(), names.len(), "duplicate catalog identity");
        assert_eq!(declared, ids, "catalog and embedded dictionary diverged");
    }
    let output = super::answer(
        None,
        crate::invocation::OutputFormat::Json,
        crate::localization::Locale::EnUs,
    );
    let published: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(published["version"], 1, "runtime catalog emitter version");
    let validator = jsonschema::validator_for(&catalog).unwrap();
    validator.validate(&published).unwrap();
    let mut stale = published;
    stale["version"] = 0.into();
    assert!(!validator.is_valid(&stale));
}

#[test]
fn terminal_name_schemas_accept_current_receipts() {
    use serde_json::json;
    let id = "11111111111111111111111111111111";
    let target = json!({"kind":"terminal","binding":{"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id}});
    for (schema, result) in [
        (
            "orchbee.agent.resolve",
            json!({"kind":"resolved-target","target":target,"resolvedBy":"name"}),
        ),
        (
            "orchbee.agent.names",
            json!({"kind":"listed-targets","bindings":[{"name":"alpha","target":target}],"pending":[]}),
        ),
        (
            "orchbee.agent.rename",
            json!({"kind":"assigned-target","target":target,"name":"alpha","previousName":null}),
        ),
    ] {
        let definition: Value = serde_json::from_str(super::document(schema).unwrap()).unwrap();
        let validator = jsonschema::validator_for(&definition).unwrap();
        let document = json!({"schema":schema,"version":1,"source":{"kind":"agent-names","requestId":id},"result":result});
        validator.validate(&document).unwrap();
    }
    let definition: Value =
        serde_json::from_str(super::document("orchbee.cli.failure.agent-names").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&definition).unwrap();
    let document = json!({"schema":"orchbee.cli.failure.agent-names","version":1,"failure":{"kind":"rejected","details":{"stage":"agent-names","reason":"name-in-use","conflictTarget":target}}});
    validator.validate(&document).unwrap();
    let mut invalid = document.clone();
    invalid["failure"]["details"]["conflictTarget"] = Value::Null;
    assert!(!validator.is_valid(&invalid));
}

fn canonical_agent_inspection(with_client: bool) -> Box<crate::cli::results::AgentInspectionDoc> {
    use crate::cli::results::{
        AgentClientVersionMatch, AgentClientViewDoc, AgentInspectionDoc, AgentManagementDoc,
        AgentRuntimeDoc,
    };
    use orchbee_agent_observation_protocol::{
        AgentManagement, AgentSafeInteger, AgentSessionDetail,
    };
    use orchbee_workbench_protocol::{ClientViewPhase, WorkbenchClientId, WorkbenchRequestId};
    use serde_json::json;

    let id = "11111111111111111111111111111111";
    let session: AgentSessionDetail = serde_json::from_value(json!({
        "snapshot": {
            "activityHint": null,
            "capabilities": ["sessionLifecycle"],
            "executionLocation": {"executionLocationId": "local"},
            "lastOutcome": null,
            "lifecycle": "live",
            "observation": {"fidelity":"rich","freshness":"fresh","generation":1,"sequence":2,"source":"native"},
            "parentSessionId": null,
            "pendingRequests": [],
            "presentationAttachments": [{"generation":"00000000000000000000000000000001","navigable":true,"presentationId":"0000000000000000000000000000000c","visibility":"visible"}],
            "providerId": "codex",
            "sessionId": "session-schema",
            "turn": {"kind":"idle"}
        },
        "status": {"kind":"idle"}
    }))
    .expect("current Agent session detail fixture");
    let client = with_client.then(|| {
        Box::new(AgentClientViewDoc {
            client_id: WorkbenchClientId::parse(id).unwrap(),
            operation_id: WorkbenchRequestId::parse("22222222222222222222222222222222").unwrap(),
            phase: ClientViewPhase::Ready,
            outer_focused: Some(true),
            focused: true,
            attention_revision: AgentSafeInteger::new(3),
            state: AgentClientVersionMatch::Current,
            state_change_revision: Some(AgentSafeInteger::new(2)),
            seen: Some(true),
            status: Some("idle"),
        })
    });
    Box::new(AgentInspectionDoc {
        revision: AgentSafeInteger::new(2),
        session: Box::new(session),
        client,
        runtime: AgentRuntimeDoc::Unavailable("unbound"),
        terminal: None,
        management: AgentManagementDoc {
            source: json!({"kind":"agent-read","requestId":id}),
            revision: AgentSafeInteger::new(2),
            management: AgentManagement::Unmanaged {},
        },
    })
}

#[cfg(windows)]
fn current_canonical_terminal() -> Value {
    use serde_json::json;

    let id = "11111111111111111111111111111111";
    json!({
        "terminalId": id,
        "source": {"kind":"workbench-snapshot","sessionId":id,"revision":"7"},
        "cwd": null,
        "cwdSource": null,
        "foregroundCwd": null,
        "terminalTitle": null,
        "terminalTitleStripped": null,
        "terminalObservation": {"state":"unavailable","reason":"terminal-unavailable"},
        "metadata": {
            "source":{"sessionId":id,"workbenchRevision":"7","metadataRevision":"2","requestIds":["22222222222222222222222222222222","33333333333333333333333333333333"]},
            "binding":null,
            "tokens":{},
            "presentations":[],
            "context":{"agent":null,"authoritySource":null},
            "presentation":{"title":null,"displayAgent":null,"stateLabels":{}}
        },
        "reportedAgent": {
            "source":{"sessionId":id,"workbenchRevision":"7","reportRevision":"8","requestIds":["44444444444444444444444444444444","55555555555555555555555555555555"]},
            "binding":null,
            "view":null
        }
    })
}

#[test]
fn canonical_agent_get_schema_has_fixed_members() {
    use crate::cli::results::Projection;
    use serde_json::json;

    let definition: Value =
        serde_json::from_str(super::document("orchbee.agent.get").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&definition).unwrap();
    for client in [false, true] {
        let result = crate::cli::results::complete(
            &crate::invocation::QueryCommand::AgentGet(crate::invocation::AgentTarget::Session {
                session_id: orchbee_agent_observation_protocol::AgentSessionId::parse(
                    "11111111111111111111111111111111",
                )
                .unwrap(),
            }),
            Projection::AgentInspection(canonical_agent_inspection(client)).result(),
        );
        let document = json!({
            "schema":"orchbee.agent.get",
            "version":1,
            "source":{"kind":"agent-read","requestId":"11111111111111111111111111111111"},
            "result":result
        });
        validator.validate(&document).unwrap();
        let mut minimal = document;
        for field in ["runtime", "terminal", "management"] {
            minimal["result"]["inspection"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        assert!(!validator.is_valid(&minimal));
    }
}

#[cfg(windows)]
#[test]
fn unified_agent_get_schema_is_closed_around_real_projection_and_inspection() {
    use crate::{
        agent_projection::{
            read::Checkpoint,
            tests::{rows, state, summary},
        },
        cli::{
            agent_projection::Inspection,
            results::{AgentRuntimeDoc, Projection},
        },
    };
    use orchbee_agent_observation_protocol::{AgentSafeInteger, AgentSemanticStatus};
    use orchbee_workbench_protocol::WorkbenchRevision;
    use serde_json::json;

    let definition: Value =
        serde_json::from_str(super::document("orchbee.agent.get").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&definition).unwrap();
    let mut canonical = summary("session-schema", 12, "codex");
    canonical.status = AgentSemanticStatus::Idle {};
    let checkpoint = Checkpoint {
        canonical_revision: AgentSafeInteger::new(2),
        report_revision: WorkbenchRevision::new(7),
        rows: rows(vec![canonical], vec![state(12, None)]),
    };
    let snapshot = crate::machine_agents::tests::fixture();
    let source = crate::cli::agent_projection::source(&snapshot, &checkpoint);
    let document = |with_client| {
        let result = Projection::UnifiedAgentInspection(Box::new(Inspection {
            row: checkpoint.rows[0].clone(),
            detail: Box::new(Projection::AgentInspection(canonical_agent_inspection(
                with_client,
            ))),
        }))
        .result();
        json!({
            "schema":"orchbee.agent.get",
            "version":1,
            "source":source.clone(),
            "result":result
        })
    };
    for with_client in [false, true] {
        let current = document(with_client);
        validator.validate(&current).unwrap();
        assert_eq!(current["result"]["agent"]["source"], "canonical");
        assert!(!current["result"]["agent"]["placement"].is_null());
        assert_eq!(
            !current["result"]["inspection"]["client"].is_null(),
            with_client
        );
    }

    let mut inspection = canonical_agent_inspection(false);
    inspection.runtime = AgentRuntimeDoc::Unavailable("terminal-unavailable");
    inspection.terminal = Some(current_canonical_terminal());
    let with_terminal = Projection::UnifiedAgentInspection(Box::new(Inspection {
        row: checkpoint.rows[0].clone(),
        detail: Box::new(Projection::AgentInspection(inspection)),
    }))
    .result();
    let with_terminal = json!({
        "schema":"orchbee.agent.get",
        "version":1,
        "source":source.clone(),
        "result":with_terminal
    });
    validator.validate(&with_terminal).unwrap();
    for field in ["metadata", "reportedAgent"] {
        let mut invalid_terminal = with_terminal.clone();
        invalid_terminal["result"]["inspection"]["terminal"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            !validator.is_valid(&invalid_terminal),
            "non-null Windows canonical terminal requires {field}"
        );
    }

    for reason in [
        "session-ended",
        "unbound",
        "ambiguous-primary",
        "binding-changed",
    ] {
        let mut null_terminal = document(false);
        null_terminal["result"]["inspection"]["runtime"]["reason"] = reason.into();
        assert!(
            validator.is_valid(&null_terminal),
            "a null Windows canonical terminal is valid for {reason}"
        );
    }
    for reason in ["terminal-unavailable", "process-unavailable"] {
        let mut null_terminal = document(false);
        null_terminal["result"]["inspection"]["runtime"]["reason"] = reason.into();
        assert!(
            !validator.is_valid(&null_terminal),
            "a null Windows canonical terminal is invalid for {reason}"
        );
    }
    let mut placed_terminal = with_terminal.clone();
    placed_terminal["result"]["inspection"]["runtime"]["reason"] = "unbound".into();
    assert!(
        !validator.is_valid(&placed_terminal),
        "a non-null Windows canonical terminal is invalid for an early unavailable reason"
    );

    let document = document(false);
    let mut invalid = document.clone();
    invalid["result"]["agent"]["ordinal"] = 1.into();
    assert!(
        !validator.is_valid(&invalid),
        "the Get row omits list ordinal"
    );
    invalid = document.clone();
    invalid["result"]["target"] = json!({"kind":"terminal"});
    assert!(
        !validator.is_valid(&invalid),
        "the projection wrapper has no legacy target"
    );
    invalid = document.clone();
    invalid["result"]["inspection"]["extra"] = true.into();
    assert!(
        !validator.is_valid(&invalid),
        "current inspection stays closed"
    );
    for field in ["runtime", "terminal", "management"] {
        invalid = document.clone();
        invalid["result"]["inspection"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            !validator.is_valid(&invalid),
            "Windows projected canonical inspection requires {field}"
        );
    }
    invalid = document;
    invalid["source"]["revision"] = "1".into();
    assert!(
        !validator.is_valid(&invalid),
        "projection source has no legacy revision"
    );
}

#[cfg(windows)]
#[test]
fn unified_agent_get_physical_inspection_requires_current_semantics() {
    use crate::{
        agent_projection::{
            read::Checkpoint,
            tests::{rows, state},
        },
        cli::{agent_projection::Inspection, results::Projection},
    };
    use orchbee_agent_observation_protocol::{AgentLaunchBinding, AgentSafeInteger};
    use orchbee_workbench_protocol::{TerminalAgentObservation, WorkbenchRevision};
    use serde_json::json;

    let definition: Value =
        serde_json::from_str(super::document("orchbee.agent.get").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&definition).unwrap();
    let checkpoint = Checkpoint {
        canonical_revision: AgentSafeInteger::new(2),
        report_revision: WorkbenchRevision::new(7),
        rows: rows(Vec::new(), vec![state(12, Some("codex"))]),
    };
    let snapshot = crate::machine_agents::tests::fixture();
    let source = crate::cli::agent_projection::source(&snapshot, &checkpoint);
    let row = checkpoint.rows[0].clone();
    let row_document = row.document(1, None);
    let binding: AgentLaunchBinding =
        serde_json::from_value(row_document["terminal"]["binding"].clone()).unwrap();
    let placement = &row_document["placement"];
    let id = "11111111111111111111111111111111";
    let terminal = json!({
        "terminalId": placement["terminalId"],
        "presentationId": placement["presentationId"],
        "panelId": placement["panelId"],
        "source": crate::cli::results::snapshot_source(&snapshot),
        "cwd": null,
        "cwdSource": null,
        "foregroundCwd": null,
        "terminalTitle": null,
        "terminalTitleStripped": null,
        "terminalObservation": {"state":"unavailable","reason":"terminal-unavailable"}
    });
    let observation: TerminalAgentObservation = serde_json::from_value(json!({
        "binding": binding.clone(),
        "sequence":"4",
        "changes":[],
        "view":{"agent":"codex","state":"idle","authoritySource":null,"message":null}
    }))
    .unwrap();
    let document = Projection::UnifiedAgentInspection(Box::new(Inspection {
        row,
        detail: Box::new(Projection::AgentTerminalInspection {
            client_view: None,
            binding,
            terminal,
            semantic: crate::cli::query::terminal_agent::TerminalAgentRead {
                source: json!({"sessionId":id,"workbenchRevision":"12","reportRevision":"7","requestId":"22222222222222222222222222222222"}),
                report_revision: WorkbenchRevision::new(7),
                observation: Box::new(observation),
            },
        }),
    }))
    .result();
    let document = json!({
        "schema":"orchbee.agent.get",
        "version":1,
        "source":source,
        "result":document
    });
    validator.validate(&document).unwrap();
    let mut invalid = document.clone();
    invalid["result"]["inspection"]
        .as_object_mut()
        .unwrap()
        .remove("semantic");
    assert!(
        !validator.is_valid(&invalid),
        "current physical Get inspection requires semantic detail"
    );
    let mut obsolete = document;
    obsolete["result"]["inspection"]["management"] = json!({"phase":"unmanaged"});
    assert!(
        !validator.is_valid(&obsolete),
        "physical inspection does not retain a constant management field"
    );
}

#[test]
fn configuration_admits_bounded_warnings_and_requires_current_fields() {
    use serde_json::json;
    let schema: Value =
        serde_json::from_str(super::document("orchbee.config.check").unwrap()).unwrap();
    let full_validator = jsonschema::validator_for(&schema).unwrap();
    let mut reduced = schema.clone();
    reduced["$ref"] = json!("#/$defs/userConfigCheck");
    reduced.as_object_mut().unwrap().remove("required");
    reduced.as_object_mut().unwrap().remove("properties");
    reduced
        .as_object_mut()
        .unwrap()
        .remove("additionalProperties");
    reduced.as_object_mut().unwrap().remove("allOf");
    let validator = jsonschema::validator_for(&reduced).unwrap();
    let details = json!({
        "path": null,
        "exists": false,
        "newCwd": "follow",
        "worktreeDirectory": null,
        "manifestCheck": true,
        "manifestCheckSource": "default",
        "defaultShell": "",
        "shellMode": "auto",
        "themeName":"one-dark",
        "keyPrefix": "Ctrl+B",
        "keyOverrides": 0,
        "windowTitle": "{hostname}: {workspace}",
        "windowTitleDiagnostic": null,
        "notificationDelivery": "off",
        "notificationPosition": "bottom-right",
        "agentSidebarProfile": "{}",
        "spacesSidebarProfile": "{}",
        "notificationAgentDelivery": "orchbee",
        "notificationDelaySeconds": 1,
        "notificationSoundEnabled": false,
        "notificationAgentSounds": {},
        "diagnostics": [],
        "diagnosticsOmitted": 0
    });
    let pass = json!({"name":"user-config","status":"pass","details":details});
    assert!(validator.is_valid(&pass));
    let mut all_action_overrides = pass.clone();
    all_action_overrides["details"]["keyOverrides"] = json!(37);
    assert!(validator.is_valid(&all_action_overrides));
    all_action_overrides["details"]["keyOverrides"] = json!(38);
    assert!(!validator.is_valid(&all_action_overrides));
    let mut warning = pass.clone();
    warning["status"] = json!("warning");
    warning["details"]["diagnostics"] = json!([{
        "field":"theme.custom",
        "reason":"unknown-key",
        "disposition":"ignored"
    }]);
    assert!(validator.is_valid(&warning));
    let mut invalid_field = warning.clone();
    invalid_field["details"]["diagnostics"][0]["field"] = json!("theme\u{0085}name");
    assert!(!validator.is_valid(&invalid_field));
    let mut invalid_reason = warning.clone();
    invalid_reason["details"]["diagnostics"][0]["reason"] = json!("1unsupported");
    assert!(!validator.is_valid(&invalid_reason));
    let mut invalid_title_diagnostic = warning.clone();
    invalid_title_diagnostic["details"]["windowTitleDiagnostic"] = json!("unknown-token");
    assert!(!validator.is_valid(&invalid_title_diagnostic));
    let mut invalid_pass = warning.clone();
    invalid_pass["status"] = json!("pass");
    assert!(!validator.is_valid(&invalid_pass));
    let mut invalid_warning = pass.clone();
    invalid_warning["status"] = json!("warning");
    assert!(!validator.is_valid(&invalid_warning));
    let endpoint = json!({"presence":"absent","status":"not-applicable"});
    let budget = json!({"presence":"unchecked","status":"not-applicable","reason":"presence-not-inspected","addressBytes":12,"addressCapacityBytes":200,"path":"C:/endpoint"});
    let mut fatal = json!({
        "schema":"orchbee.config.check", "version":1, "source":{"kind":"local-runtime"},
        "result":{"ok":false,"checks":[
            {"name":"runtime-base","status":"pass","details":{"exists":true,"path":"C:/runtime"}},
            {"name":"runtime-directory","status":"pass","details":{"exists":true,"path":"C:/runtime/session"}},
            {"name":"cohort","status":"pass","details":{"verified":true}},
            {"name":"locale","status":"pass","details":{"locale":"en-US","source":"LC_ALL"}},
            {"name":"sockets","status":"not-applicable","details":{"client":endpoint,"worker":endpoint,"host":budget,"clientPending":budget}},
            {"name":"user-config","status":"fail","details":{"field":"file","reason":"invalid-toml"}}
        ]}
    });
    for (field, reason) in [
        ("file", "invalid-path"),
        ("file", "missing"),
        ("file", "io-error"),
        ("file", "not-file"),
        ("file", "too-large"),
        ("file", "invalid-utf8"),
        ("file", "invalid-toml"),
        ("ORCHBEE_CONFIG_FILE", "invalid-path"),
        ("APPDATA", "invalid-path"),
    ] {
        fatal["result"]["checks"][5]["details"] = json!({"field":field,"reason":reason});
        assert!(full_validator.is_valid(&fatal), "{field}: {reason}");
    }
    for (field, reason) in [
        ("keys", "invalid-binding"),
        ("ui.toast", "wrong-type"),
        ("update.manifest_check", "wrong-type"),
        ("file", "invalid-boolean"),
        ("terminal.new_cwd", "invalid-path"),
        ("worktrees.directory", "invalid-path"),
        ("ORCHBEE_CONFIG_FILE", "invalid-boolean"),
    ] {
        fatal["result"]["checks"][5]["details"] = json!({"field":field,"reason":reason});
        assert!(
            !full_validator.is_valid(&fatal),
            "field-local or mismatched fatal pair admitted: {field}: {reason}"
        );
    }
    for field in ["manifestCheck", "manifestCheckSource"] {
        let mut missing = pass.clone();
        missing["details"].as_object_mut().unwrap().remove(field);
        assert!(!validator.is_valid(&missing), "missing {field}");
    }
}

#[test]
fn popup_receipts_and_client_control_keep_closed_shapes() {
    use serde_json::json;
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let mut reply = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "popup-opened-actual-source")
        .unwrap()["document"]
        .clone();
    reply.as_object_mut().unwrap().remove("kind");
    let mut document = json!({"schema":"orchbee.client.control","version":1,
        "sessionId": reply["sessionId"],"operationId":reply["outcome"]["operation"]["command"]["operationId"],
        "observation":{"kind":"server","reply":reply},"wait":null});
    let schema: Value =
        serde_json::from_str(super::document("orchbee.client.control").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&document));
    document["schema"] = json!("orchbee.popup.result");
    document["version"] = json!(1);
    let schema: Value =
        serde_json::from_str(super::document("orchbee.popup.result").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&document));
    document["observation"]["reply"]["outcome"]["operation"]["command"]["intent"]["spec"]
        .as_object_mut()
        .unwrap()
        .remove("width");
    assert!(!validator.is_valid(&document));
}

mod organization;

/// Plain `api schema` lists the catalog; an ID, `--format json` or export still
/// answers the machine documents.
#[test]
fn text_catalog_lists_every_schema_and_explicit_requests_keep_documents() {
    use crate::{invocation::OutputFormat, localization::Locale};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let output = super::read(None, OutputFormat::Text, locale);
        assert_eq!(output.status, 0);
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(text.lines().count(), DOCUMENTS.len() + 2, "{text}");
        assert!(text.starts_with(CliText::new(locale).cli_schema_catalog_headers()[0]));
        assert!(text.ends_with(CliText::new(locale).cli_schema_catalog_footer()));
        for (id, _) in DOCUMENTS {
            assert!(
                text.lines()
                    .any(|line| line.split_whitespace().next() == Some(*id)),
                "{id}"
            );
        }
        assert!(
            text.lines()
                .any(|line| line.starts_with("orchbee.worktree.list ") && line.contains(" 1 ")),
            "{text}"
        );
    }
    let single = super::answer(
        Some("orchbee.worktree.list"),
        OutputFormat::Text,
        Locale::EnUs,
    );
    let single: Value = serde_json::from_slice(&single.stdout).unwrap();
    assert_eq!(
        single["result"]["schema"]["schemaId"],
        "orchbee.worktree.list"
    );
}
