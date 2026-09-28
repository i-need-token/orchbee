//! The published catalog must describe the exact embedded schema dictionary.
use super::DOCUMENTS;
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn catalog_requires_and_admits_every_embedded_schema_exactly_once() {
    let ids = DOCUMENTS.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), DOCUMENTS.len(), "duplicate embedded schema ID");
    for (id, body) in DOCUMENTS {
        let document: Value = serde_json::from_str(body).expect("embedded JSON schema");
        assert_eq!(document["$id"].as_str(), Some(*id));
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
    jsonschema::validator_for(&catalog)
        .unwrap()
        .validate(&published)
        .unwrap();
}

#[test]
fn configuration_check_versions_keep_closed_and_required_restore_fields() {
    use serde_json::json;
    let schema: Value =
        serde_json::from_str(super::document("orchbee.config.check").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let endpoint = json!({"presence":"absent","status":"not-applicable"});
    let budget = json!({"presence":"unchecked","status":"not-applicable","reason":"presence-not-inspected","addressBytes":12,"addressCapacityBytes":200,"path":"C:/endpoint"});
    let mut document = json!({
        "schema":"orchbee.config.check", "version":3, "source":{"kind":"local-runtime"},
        "result":{"ok":true,"checks":[
            {"name":"runtime-base","status":"pass","details":{"exists":true,"path":"C:/runtime"}},
            {"name":"runtime-directory","status":"pass","details":{"exists":true,"path":"C:/runtime/site"}},
            {"name":"cohort","status":"pass","details":{"verified":true}},
            {"name":"locale","status":"pass","details":{"locale":"en-US","source":"LC_ALL"}},
            {"name":"sockets","status":"not-applicable","details":{"client":endpoint,"worker":endpoint,"host":budget,"clientPending":budget}},
            {"name":"user-config","status":"pass","details":{"exists":true,"path":"C:/config.toml","newCwd":"follow","worktreeDirectory":null,"resumeAgentsOnRestore":false,"resumeAgentsOnRestoreSource":"file"}}
        ]}
    });
    validator.validate(&document).unwrap();
    for field in ["resumeAgentsOnRestore", "resumeAgentsOnRestoreSource"] {
        let mut invalid = document.clone();
        invalid["result"]["checks"][5]["details"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(!validator.is_valid(&invalid), "missing {field}");
    }
    for (field, value) in [
        ("resumeAgentsOnRestore", "false"),
        ("resumeAgentsOnRestoreSource", "unknown"),
    ] {
        let mut invalid = document.clone();
        invalid["result"]["checks"][5]["details"][field] = value.into();
        assert!(!validator.is_valid(&invalid), "invalid {field}");
    }
    document["version"] = 2.into();
    assert!(!validator.is_valid(&document));
    let details = document["result"]["checks"][5]["details"]
        .as_object_mut()
        .unwrap();
    details.remove("resumeAgentsOnRestore");
    details.remove("resumeAgentsOnRestoreSource");
    validator.validate(&document).unwrap();
    document["version"] = 1.into();
    assert!(!validator.is_valid(&document));
    document["result"]["checks"].as_array_mut().unwrap().pop();
    validator.validate(&document).unwrap();
}

#[test]
fn terminal_name_schemas_accept_new_receipts_without_widening_legacy_versions() {
    use serde_json::json;
    let id = "11111111111111111111111111111111";
    let target = json!({"kind":"terminal","binding":{"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id}});
    for (schema, version, result, old) in [
        (
            "orchbee.agent.resolve",
            3,
            json!({"kind":"resolved-target","target":target,"resolvedBy":"name"}),
            2,
        ),
        (
            "orchbee.agent.names",
            3,
            json!({"kind":"listed-targets","bindings":[{"name":"alpha","target":target}],"pending":[]}),
            2,
        ),
        (
            "orchbee.agent.rename",
            2,
            json!({"kind":"assigned-target","target":target,"name":"alpha","previousName":null}),
            1,
        ),
    ] {
        let definition: Value = serde_json::from_str(super::document(schema).unwrap()).unwrap();
        let validator = jsonschema::validator_for(&definition).unwrap();
        let mut document = json!({"schema":schema,"version":version,"source":{"kind":"agent-names","requestId":id},"result":result});
        validator.validate(&document).unwrap();
        document["version"] = json!(old);
        assert!(!validator.is_valid(&document));
    }
    let definition: Value =
        serde_json::from_str(super::document("orchbee.cli.failure.agent-names").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&definition).unwrap();
    let document = json!({"schema":"orchbee.cli.failure.agent-names","version":4,"failure":{"kind":"rejected","details":{"stage":"agent-names","reason":"name-in-use","conflictTarget":target}}});
    validator.validate(&document).unwrap();
    let mut invalid = document.clone();
    invalid["failure"]["details"]["conflictTarget"] = Value::Null;
    assert!(!validator.is_valid(&invalid));
}

#[test]
fn terminal_agent_inspection_schema_has_closed_target_and_preserves_canonical_versions() {
    use serde_json::json;
    let id = "11111111111111111111111111111111";
    let source = json!({"kind":"workbench-snapshot","siteId":id,"revision":"1"});
    let binding = json!({"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id});
    let definition: Value =
        serde_json::from_str(super::document("orchbee.agent.get").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&definition).unwrap();
    let document = json!({"schema":"orchbee.agent.get","version":13,"source":source,"result":{
        "kind":"terminal-found","target":{"kind":"terminal","binding":binding},"management":{"phase":"unmanaged"},
        "terminal":{"terminalId":id,"presentationId":id,"panelId":id,"source":source,"cwd":null,"cwdSource":null,"foregroundCwd":null,"terminalTitle":null,"terminalTitleStripped":null,
            "terminalObservation":{"state":"observed","frame":{"hostEpoch":id,"attachmentId":id,"sequence":"1","generation":"1","revision":"1","columns":80,"rows":24,"alternateScreen":false,"scrollbackRows":"0","title":null,"currentDirectory":null},"processes":{"state":"unavailable","reason":"process-unavailable"}}}
    }});
    validator.validate(&document).unwrap();
    for state in [
        json!({"phase":"unmanaged"}),
        json!({"phase":"active","providerId":"codex"}),
    ] {
        let mut current = document.clone();
        current["version"] = 14.into();
        current["result"]["management"] =
            json!({"source":{"kind":"agent-read","requestId":id},"state":state});
        validator.validate(&current).unwrap();
        let mut observed = current.clone();
        observed["version"] = 15.into();
        observed["result"]["semantic"] = json!({"source":{"siteId":id,"workbenchRevision":"1","reportRevision":"2","requestId":id},
            "observation":{"binding":binding,"sequence":"0","changes":[],"view":{"agent":null,"state":"unknown","authoritySource":null,"message":null,"session":null}}});
        validator.validate(&observed).unwrap();
        observed["version"] = 14.into();
        assert!(!validator.is_valid(&observed));
        current["result"]["management"]["state"]["reservationId"] = id.into();
        assert!(!validator.is_valid(&current));
    }
    let mut invalid = document.clone();
    invalid["version"] = 12.into();
    assert!(!validator.is_valid(&invalid));
    let mut invalid = document.clone();
    invalid["result"]["session"] = json!({});
    assert!(!validator.is_valid(&invalid));
    let mut invalid = document;
    invalid["result"]["target"]["binding"]
        .as_object_mut()
        .unwrap()
        .remove("generation");
    assert!(!validator.is_valid(&invalid));
}
