//! Embedded JSON Schema dictionary. Every document ships inside the binary so
//! `api schema` never touches the filesystem, the network, or a Site.

use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, Output, failure_output, output, target_details};
use crate::invocation::{OutputFormat, Resource};
use crate::localization::Locale;

pub(crate) const FAILURE_ID: &str = "orchbee.cli.failure";

/// `schema` id → embedded document, in dictionary order.
const DOCUMENTS: &[(&str, &str)] = &[
    (
        "orchbee.api.schema",
        include_str!("schemas/orchbee.api.schema.json"),
    ),
    (
        "orchbee.api.snapshot",
        include_str!("schemas/orchbee.api.snapshot.json"),
    ),
    (FAILURE_ID, include_str!("schemas/orchbee.cli.failure.json")),
    (
        "orchbee.config.check",
        include_str!("schemas/orchbee.config.check.json"),
    ),
    (
        "orchbee.panel.get",
        include_str!("schemas/orchbee.panel.get.json"),
    ),
    (
        "orchbee.panel.list",
        include_str!("schemas/orchbee.panel.list.json"),
    ),
    (
        "orchbee.project.get",
        include_str!("schemas/orchbee.project.get.json"),
    ),
    (
        "orchbee.project.list",
        include_str!("schemas/orchbee.project.list.json"),
    ),
    (
        "orchbee.status",
        include_str!("schemas/orchbee.status.json"),
    ),
    (
        "orchbee.tab.get",
        include_str!("schemas/orchbee.tab.get.json"),
    ),
    (
        "orchbee.tab.list",
        include_str!("schemas/orchbee.tab.list.json"),
    ),
    (
        "orchbee.terminal.get",
        include_str!("schemas/orchbee.terminal.get.json"),
    ),
    (
        "orchbee.terminal.list",
        include_str!("schemas/orchbee.terminal.list.json"),
    ),
    (
        "orchbee.terminal.read",
        include_str!("schemas/orchbee.terminal.read.json"),
    ),
    (
        "orchbee.workspace.get",
        include_str!("schemas/orchbee.workspace.get.json"),
    ),
    (
        "orchbee.workspace.list",
        include_str!("schemas/orchbee.workspace.list.json"),
    ),
];

pub(crate) fn embedded_source() -> Value {
    json!({ "kind": "embedded" })
}

fn document(schema_id: &str) -> Option<&'static str> {
    DOCUMENTS
        .iter()
        .find(|(id, _document)| *id == schema_id)
        .map(|(_id, document)| *document)
}

/// `api schema` always answers a machine document on success, in either
/// format. Its failures keep the caller's selected format like every other
/// command: text/default keeps stdout empty, json emits the failure envelope.
pub(crate) fn answer(schema_id: Option<&str>, format: OutputFormat, locale: Locale) -> Output {
    let caller = Some(format);
    let result = match schema_id {
        Some(schema_id) => {
            let Some(document) = document(schema_id) else {
                return failure_output(
                    Failure::new(FailureKind::NotFound)
                        .details(target_details(Resource::Schema, schema_id))
                        .source(embedded_source()),
                    caller,
                    locale,
                );
            };
            let document = match serde_json::from_str::<Value>(document) {
                Ok(document) => document,
                Err(_) => {
                    return failure_output(
                        Failure::new(FailureKind::InternalFailure),
                        caller,
                        locale,
                    );
                }
            };
            json!({
                "schema": {
                    "schemaId": schema_id,
                    "document": document,
                }
            })
        }
        None => {
            let mut schemas = Map::new();
            for (schema_id, document) in DOCUMENTS {
                match serde_json::from_str::<Value>(document) {
                    Ok(document) => {
                        schemas.insert((*schema_id).to_owned(), document);
                    }
                    Err(_) => {
                        return failure_output(
                            Failure::new(FailureKind::InternalFailure),
                            caller,
                            locale,
                        );
                    }
                }
            }
            json!({ "schemas": Value::Object(schemas) })
        }
    };
    let document = json!({
        "schema": "orchbee.api.schema",
        "version": 1,
        "source": embedded_source(),
        "result": result,
    });
    match output::encode_document(&document) {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
        },
        Err(output::TooLarge) => failure_output(
            Failure::new(FailureKind::OutputTooLarge)
                .details(Map::from_iter([(
                    "limitBytes".to_owned(),
                    json!(output::LIMIT),
                )]))
                .source(embedded_source()),
            caller,
            locale,
        ),
    }
}
