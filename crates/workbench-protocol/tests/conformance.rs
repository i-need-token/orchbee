use std::{collections::BTreeMap, error::Error, io};

use orchbee_workbench_protocol::{
    ContractError, DeltaRejectionReason, MAXIMUM_ENCODED_INTENT_BYTES, SiteId,
    WorkbenchProtocolDocument, WorkbenchRevision, apply_workbench_delta, parse_delta,
    parse_document, parse_snapshot, serialize_document,
};
use serde::Deserialize;
use serde_json::{Value, json};

const CORPUS: &str = include_str!("../../../schemas/workbench/v1/workbench-conformance-v1.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Corpus {
    contract_version: u8,
    documents: BTreeMap<String, Value>,
    invalid_documents: Vec<InvalidDocument>,
    transitions: Vec<Transition>,
    valid_documents: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidDocument {
    #[serde(rename = "boundary")]
    _boundary: String,
    name: String,
    value: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    delta: String,
    initial: String,
    name: String,
    outcome: TransitionOutcome,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum TransitionOutcome {
    Applied { snapshot: String },
    Rejected { reason: DeltaRejectionReason },
}

#[test]
fn consumes_every_canonical_document() -> Result<(), Box<dyn Error>> {
    let corpus = read_corpus()?;
    assert_eq!(corpus.contract_version, 1);
    for name in &corpus.valid_documents {
        let encoded = serde_json::to_vec(document(&corpus, name)?)?;
        let parsed = parse_document(&encoded).map_err(boxed_contract_error)?;
        let serialized = serialize_document(&parsed).map_err(boxed_contract_error)?;
        let reparsed = parse_document(&serialized).map_err(boxed_contract_error)?;
        assert_eq!(parsed, reparsed, "valid document {name}");
    }
    for fixture in &corpus.invalid_documents {
        let encoded = serde_json::to_vec(&fixture.value)?;
        assert!(
            parse_document(&encoded).is_err(),
            "invalid document {} was accepted",
            fixture.name
        );
    }
    Ok(())
}

#[test]
fn applies_every_canonical_delta_transition_atomically() -> Result<(), Box<dyn Error>> {
    let corpus = read_corpus()?;
    for fixture in &corpus.transitions {
        let initial = parse_snapshot(&serde_json::to_vec(document(&corpus, &fixture.initial)?)?)
            .map_err(boxed_contract_error)?;
        let delta = parse_delta(&serde_json::to_vec(document(&corpus, &fixture.delta)?)?)
            .map_err(boxed_contract_error)?;
        let original = initial.clone();
        let result = apply_workbench_delta(&initial, &delta);
        match &fixture.outcome {
            TransitionOutcome::Applied { snapshot } => {
                let expected = parse_snapshot(&serde_json::to_vec(document(&corpus, snapshot)?)?)
                    .map_err(boxed_contract_error)?;
                assert_eq!(result, Ok(expected), "transition {}", fixture.name);
            }
            TransitionOutcome::Rejected { reason } => {
                assert_eq!(result, Err(*reason), "transition {}", fixture.name);
            }
        }
        assert_eq!(
            initial, original,
            "transition {} mutated input",
            fixture.name
        );
    }
    Ok(())
}

#[test]
fn enforces_identity_revision_and_encoded_intent_limits() -> Result<(), Box<dyn Error>> {
    assert!(SiteId::parse("11111111111111111111111111111111").is_ok());
    assert!(SiteId::parse("00000000000000000000000000000000").is_err());
    assert_eq!(
        WorkbenchRevision::parse("18446744073709551615")?.value(),
        u64::MAX
    );
    assert!(WorkbenchRevision::parse("18446744073709551616").is_err());
    assert!(WorkbenchRevision::parse("01").is_err());

    let corpus = read_corpus()?;
    let read_intent = document(&corpus, "snapshot-read")?;
    let encoded = serde_json::to_vec(read_intent)?;
    let padding_length = MAXIMUM_ENCODED_INTENT_BYTES - encoded.len() + 1;
    let mut oversized = vec![b' '; padding_length];
    oversized.extend_from_slice(&encoded);
    assert!(matches!(
        parse_document(&oversized),
        Err(ContractError::EncodedValueTooLarge { .. })
    ));
    Ok(())
}

fn read_corpus() -> Result<Corpus, serde_json::Error> {
    serde_json::from_str(CORPUS)
}

fn document<'corpus>(corpus: &'corpus Corpus, name: &str) -> Result<&'corpus Value, io::Error> {
    corpus
        .documents
        .get(name)
        .ok_or_else(|| io::Error::other(format!("missing conformance document {name}")))
}

fn boxed_contract_error(error: ContractError) -> Box<dyn Error> {
    Box::new(error)
}

#[test]
fn public_document_enum_keeps_kind_in_serialized_json() -> Result<(), Box<dyn Error>> {
    let corpus = read_corpus()?;
    let encoded = serde_json::to_vec(document(&corpus, "snapshot-full-r2")?)?;
    let parsed = parse_document(&encoded).map_err(boxed_contract_error)?;
    assert!(matches!(parsed, WorkbenchProtocolDocument::Snapshot(_)));
    let serialized = serialize_document(&parsed).map_err(boxed_contract_error)?;
    let value = serde_json::from_slice::<Value>(&serialized)?;
    assert_eq!(
        value.get("kind"),
        Some(&Value::String("workbench.snapshot".to_owned()))
    );
    Ok(())
}

#[test]
fn enforces_unicode_collection_and_tree_boundaries() -> Result<(), Box<dyn Error>> {
    let corpus = read_corpus()?;
    let mut label_boundary = document(&corpus, "snapshot-full-r2")?.clone();
    replace_string(
        &mut label_boundary,
        "/terminalContexts/0/tabs/0/label",
        "😀".repeat(512),
    )?;
    assert!(parse_document(&serde_json::to_vec(&label_boundary)?).is_ok());
    replace_string(
        &mut label_boundary,
        "/terminalContexts/0/tabs/0/label",
        "😀".repeat(513),
    )?;
    assert!(parse_document(&serde_json::to_vec(&label_boundary)?).is_err());

    let projects = (1_u128..=256)
        .map(|value| {
            json!({
                "label": format!("Project {value}"),
                "projectId": identity(value),
                "workspaces": []
            })
        })
        .collect::<Vec<_>>();
    let mut project_boundary = empty_snapshot_value("1");
    set_projects(&mut project_boundary, projects.clone())?;
    assert!(parse_document(&serde_json::to_vec(&project_boundary)?).is_ok());
    let mut overflowing_projects = projects;
    overflowing_projects.push(json!({
        "label": "Overflow",
        "projectId": identity(257),
        "workspaces": []
    }));
    set_projects(&mut project_boundary, overflowing_projects)?;
    assert!(parse_document(&serde_json::to_vec(&project_boundary)?).is_err());

    let change = json!({
        "kind": "terminal-context-removed",
        "terminalContextId": "33333333333333333333333333333333"
    });
    let bounded_delta = json!({
        "baseRevision": "1",
        "changes": vec![change.clone(); 1024],
        "kind": "workbench.delta",
        "protocolVersion": 1,
        "revision": "2",
        "siteId": "11111111111111111111111111111111"
    });
    assert!(parse_document(&serde_json::to_vec(&bounded_delta)?).is_ok());
    let overflowing_delta = json!({
        "baseRevision": "1",
        "changes": vec![change; 1025],
        "kind": "workbench.delta",
        "protocolVersion": 1,
        "revision": "2",
        "siteId": "11111111111111111111111111111111"
    });
    assert!(parse_document(&serde_json::to_vec(&overflowing_delta)?).is_err());

    assert!(parse_document(&serde_json::to_vec(&snapshot_with_depth(16))?).is_ok());
    assert!(parse_document(&serde_json::to_vec(&snapshot_with_depth(17))?).is_err());
    Ok(())
}

fn empty_snapshot_value(revision: &str) -> Value {
    json!({
        "kind": "workbench.snapshot",
        "navigation": {
            "projects": [],
            "standaloneTerminalContextId": "33333333333333333333333333333333"
        },
        "protocolVersion": 1,
        "revision": revision,
        "siteId": "11111111111111111111111111111111",
        "terminalContexts": [{
            "groups": [],
            "tabs": [],
            "terminalContextId": "33333333333333333333333333333333"
        }]
    })
}

fn snapshot_with_depth(depth: usize) -> Value {
    let mut next_identity = 100_u128;
    json!({
        "kind": "workbench.snapshot",
        "navigation": {
            "projects": [],
            "standaloneTerminalContextId": "33333333333333333333333333333333"
        },
        "protocolVersion": 1,
        "revision": "1",
        "siteId": "11111111111111111111111111111111",
        "terminalContexts": [{
            "groups": [],
            "tabs": [{
                "groupId": null,
                "label": "Terminal",
                "pinned": true,
                "root": panel_value(depth, &mut next_identity),
                "tabId": "88888888888888888888888888888888"
            }],
            "terminalContextId": "33333333333333333333333333333333"
        }]
    })
}

fn panel_value(depth: usize, next_identity: &mut u128) -> Value {
    if depth == 1 {
        let panel_id = take_identity(next_identity);
        let presentation_id = take_identity(next_identity);
        return json!({
            "childPresentations": [],
            "kind": "panel",
            "panelId": panel_id,
            "primaryPresentation": {
                "binding": null,
                "label": "Terminal",
                "presentationId": presentation_id
            }
        });
    }
    let split_id = take_identity(next_identity);
    let first_slot_id = take_identity(next_identity);
    let second_slot_id = take_identity(next_identity);
    json!({
        "children": [
            { "node": panel_value(depth - 1, next_identity), "slotId": first_slot_id },
            { "node": panel_value(1, next_identity), "slotId": second_slot_id }
        ],
        "kind": "split",
        "orientation": "horizontal",
        "splitId": split_id
    })
}

fn take_identity(next_identity: &mut u128) -> String {
    let value = identity(*next_identity);
    *next_identity += 1;
    value
}

fn identity(value: u128) -> String {
    format!("{value:032x}")
}

fn replace_string(value: &mut Value, pointer: &str, replacement: String) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| io::Error::other(format!("missing JSON pointer {pointer}")))?;
    *target = Value::String(replacement);
    Ok(())
}

fn set_projects(value: &mut Value, projects: Vec<Value>) -> Result<(), io::Error> {
    let target = value
        .pointer_mut("/navigation/projects")
        .ok_or_else(|| io::Error::other("missing navigation projects"))?;
    *target = Value::Array(projects);
    Ok(())
}
