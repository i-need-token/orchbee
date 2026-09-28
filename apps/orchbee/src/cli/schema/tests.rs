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
