use std::{error::Error, io};

use orchbee_server::WorkbenchProjectionStore;
use orchbee_workbench_protocol::{
    ContractError, MAXIMUM_ENCODED_INTENT_BYTES, MAXIMUM_ENCODED_PROJECTION_BYTES, parse_snapshot,
    parse_snapshot_read_intent,
};
use serde_json::Value;

const CORPUS: &str = include_str!("../../../schemas/workbench/v1/workbench-conformance-v1.json");

#[test]
fn returns_the_exact_request_and_immutable_snapshot() -> Result<(), Box<dyn Error>> {
    let snapshot_bytes = document_bytes("snapshot-full-r2")?;
    let intent_bytes = document_bytes("snapshot-read")?;
    let expected_snapshot = parse_snapshot(&snapshot_bytes)?;
    let expected_intent = parse_snapshot_read_intent(&intent_bytes)?;
    let reader = WorkbenchProjectionStore::load(&snapshot_bytes)?;

    let first = reader.read_snapshot(&intent_bytes)?;
    let second = reader.read_snapshot(&intent_bytes)?;

    assert_eq!(first.request_id, expected_intent.request_id);
    assert_eq!(first.snapshot, expected_snapshot);
    assert_eq!(second, first);
    Ok(())
}

#[test]
fn rejects_wrong_document_kinds_and_malformed_values() -> Result<(), Box<dyn Error>> {
    let snapshot_bytes = document_bytes("snapshot-empty-r1")?;
    let intent_bytes = document_bytes("snapshot-read")?;
    let delta_bytes = document_bytes("delta-empty-r1-r2")?;
    let reader = WorkbenchProjectionStore::load(&snapshot_bytes)?;

    assert!(matches!(
        WorkbenchProjectionStore::load(&intent_bytes),
        Err(ContractError::WrongDocumentKind)
    ));
    assert!(matches!(
        reader.read_snapshot(&snapshot_bytes),
        Err(ContractError::WrongDocumentKind)
    ));
    assert!(matches!(
        reader.read_snapshot(&delta_bytes),
        Err(ContractError::WrongDocumentKind)
    ));
    assert!(matches!(
        WorkbenchProjectionStore::load(b"{"),
        Err(ContractError::InvalidDocument)
    ));
    assert!(matches!(
        reader.read_snapshot(b"not-json"),
        Err(ContractError::InvalidDocument)
    ));
    Ok(())
}

#[test]
fn rejects_semantically_invalid_and_oversized_inputs() -> Result<(), Box<dyn Error>> {
    let invalid_snapshot = invalid_document_bytes("missing-referenced-context")?;
    assert!(matches!(
        WorkbenchProjectionStore::load(&invalid_snapshot),
        Err(ContractError::InvalidDocument)
    ));

    let snapshot_bytes = document_bytes("snapshot-empty-r1")?;
    let snapshot_padding = MAXIMUM_ENCODED_PROJECTION_BYTES
        .checked_sub(snapshot_bytes.len())
        .ok_or_else(|| io::Error::other("snapshot fixture exceeds the projection limit"))?;
    let mut bounded_snapshot = vec![b' '; snapshot_padding];
    bounded_snapshot.extend_from_slice(&snapshot_bytes);
    assert!(WorkbenchProjectionStore::load(&bounded_snapshot).is_ok());
    let mut oversized_snapshot = bounded_snapshot;
    oversized_snapshot.push(b' ');
    assert!(matches!(
        WorkbenchProjectionStore::load(&oversized_snapshot),
        Err(ContractError::EncodedValueTooLarge {
            actual,
            maximum: MAXIMUM_ENCODED_PROJECTION_BYTES
        }) if actual == MAXIMUM_ENCODED_PROJECTION_BYTES + 1
    ));

    let reader = WorkbenchProjectionStore::load(&snapshot_bytes)?;
    let intent_bytes = document_bytes("snapshot-read")?;
    let intent_padding = MAXIMUM_ENCODED_INTENT_BYTES
        .checked_sub(intent_bytes.len())
        .ok_or_else(|| io::Error::other("intent fixture exceeds the intent limit"))?;
    let mut bounded_intent = vec![b' '; intent_padding];
    bounded_intent.extend_from_slice(&intent_bytes);
    assert!(reader.read_snapshot(&bounded_intent).is_ok());
    let mut oversized_intent = bounded_intent;
    oversized_intent.push(b' ');
    assert!(matches!(
        reader.read_snapshot(&oversized_intent),
        Err(ContractError::EncodedValueTooLarge {
            actual,
            maximum: MAXIMUM_ENCODED_INTENT_BYTES
        }) if actual == MAXIMUM_ENCODED_INTENT_BYTES + 1
    ));
    Ok(())
}

fn document_bytes(name: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let corpus = serde_json::from_str::<Value>(CORPUS)?;
    let document = corpus
        .get("documents")
        .and_then(Value::as_object)
        .and_then(|documents| documents.get(name))
        .ok_or_else(|| io::Error::other(format!("missing conformance document {name}")))?;
    Ok(serde_json::to_vec(document)?)
}

fn invalid_document_bytes(name: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let corpus = serde_json::from_str::<Value>(CORPUS)?;
    let fixture = corpus
        .get("invalidDocuments")
        .and_then(Value::as_array)
        .and_then(|fixtures| {
            fixtures
                .iter()
                .find(|fixture| fixture.get("name").and_then(Value::as_str) == Some(name))
        })
        .and_then(|fixture| fixture.get("value"))
        .ok_or_else(|| io::Error::other(format!("missing invalid conformance document {name}")))?;
    Ok(serde_json::to_vec(fixture)?)
}
