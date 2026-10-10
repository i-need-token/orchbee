use serde_json::{Value, json};

#[test]
fn organization_receipts_require_explicit_nullable_grouping_and_closed_action_fields() {
    let schema: Value =
        serde_json::from_str(super::super::document("orchbee.mutation.result").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let id = "00000000000000000000000000000001";
    for project in [Value::Null, json!(id)] {
        let doc = json!({"schema":"orchbee.mutation.result","version":6,
            "source":{"kind":"workbench-command","siteId":id,"requestId":id,"expectedRevision":"1"},
            "result":{"action":"workspace.set-project","outcome":"committed","revision":"2","workspaceId":id,"projectId":project}});
        validator.validate(&doc).unwrap();
        for field in ["workspaceId", "projectId", "revision"] {
            let mut bad = doc.clone();
            bad["result"].as_object_mut().unwrap().remove(field);
            assert!(!validator.is_valid(&bad), "missing {field}");
        }
        let mut bad = doc.clone();
        bad["result"]["terminalContextId"] = json!(id);
        assert!(!validator.is_valid(&bad));
        for version in [4, 5] {
            let mut old = doc.clone();
            old["version"] = json!(version);
            assert!(!validator.is_valid(&old));
        }
        let mut created = doc.clone();
        created["result"]["action"] = json!("workspace.create-project");
        assert_eq!(validator.is_valid(&created), !project.is_null());
    }
}
