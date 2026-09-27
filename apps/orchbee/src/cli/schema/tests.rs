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
}
