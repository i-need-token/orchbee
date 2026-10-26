//! Embedded JSON Schema dictionary. Every document ships inside the binary so
//! dictionary reads never touch the filesystem, the network, or a Session.
//! Explicit file publication belongs to the separate schema exporter.

use crate::cli_text::CliText;
use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, Output, failure_output, output, target_details};
use crate::invocation::{OutputFormat, Resource};
use crate::localization::Locale;

use crate::invocation::grammar::schemas::DOCUMENTS;
pub(crate) use crate::invocation::grammar::schemas::{
    AGENT_FAILURE_ID, CLOSE_FAILURE_ID, FAILURE_ID, LAUNCH_PROFILE_FAILURE_ID, MUTATION_RESULT_ID,
    PANEL_INPUT_RESULT_ID, PANEL_RENAME_RESULT_ID, PROJECT_RENAME_RESULT_ID, SOURCE_FAILURE_ID,
    TAB_MOVE_FAILURE_ID, TAB_MOVE_RESULT_ID, WORKSPACE_MOVE_FAILURE_ID, WORKSPACE_MOVE_RESULT_ID,
    WORKTREE_FAILURE_ID,
};

pub(crate) fn embedded_source() -> Value {
    json!({ "kind": "embedded" })
}

fn document(schema_id: &str) -> Option<&'static str> {
    DOCUMENTS
        .iter()
        .find(|(id, _document)| *id == schema_id)
        .map(|(_id, document)| *document)
}

/// Text `api schema` without an ID: one row per schema with its version and
/// title, then where the documents themselves are.
fn catalog(locale: Locale) -> Result<String, Failure> {
    let mut rows = Vec::with_capacity(DOCUMENTS.len());
    for (schema_id, document) in DOCUMENTS {
        let document = serde_json::from_str::<Value>(document)
            .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
        rows.push(vec![
            (*schema_id).to_owned(),
            document["properties"]["version"]["const"]
                .as_u64()
                .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?
                .to_string(),
            document["title"].as_str().unwrap_or("-").to_owned(),
        ]);
    }
    let mut text =
        super::present::columns(&CliText::new(locale).cli_schema_catalog_headers(), &rows);
    text.push_str(CliText::new(locale).cli_schema_catalog_footer());
    Ok(text)
}

/// The `api schema` read: text without an ID lists the catalog instead of
/// printing every document; anything else is the machine document.
pub(crate) fn read(schema_id: Option<&str>, format: OutputFormat, locale: Locale) -> Output {
    if schema_id.is_some() || format == OutputFormat::Json {
        return answer(schema_id, format, locale);
    }
    match catalog(locale).and_then(|text| {
        output::encode_text(&text).map_err(|_| Failure::new(FailureKind::OutputTooLarge))
    }) {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(failure) => failure_output(failure.source(embedded_source()), Some(format), locale),
    }
}

/// The machine document, in either format: every schema, or the one named.
/// File export always uses it. Failures keep the caller's selected format
/// like every other command: text/default keeps stdout empty, json emits the
/// failure envelope.
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
            finalization: None,
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

#[cfg(test)]
mod tests;
