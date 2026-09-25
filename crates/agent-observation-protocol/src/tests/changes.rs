use crate::*;
use serde_json::{Value, json};
use std::io::Cursor;

const ID: &str = "11111111111111111111111111111111";

fn cursor() -> String {
    format!("v1.{}.{}", "A".repeat(59), "A".repeat(43))
}

fn document(outcome: Value) -> Value {
    json!({"kind":"agent.session.changes.result","requestId":ID,"result":{"outcome":outcome}})
}

fn change(revision: u64) -> Value {
    json!({"revision":revision,"transition":"session-observed","session":{
        "lifecycle":"live","observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":1,"source":"native"},
        "parentSessionId":null,"pendingRequestCount":0,"providerId":"test","sessionId":"s1","status":{"kind":"idle"}
    }})
}

fn page() -> Value {
    document(
        json!({"kind":"changes","changes":[change(1),change(2)],"cursor":cursor(),"observedThroughRevision":2,"runtimeRevision":3,"hasMore":true}),
    )
}

#[test]
fn changes_framing_defaults_and_all_outcomes_round_trip() {
    let request = parse_agent_request(
        &serde_json::to_vec(&json!({"kind":"agent.session.changes.request","requestId":ID}))
            .unwrap(),
    )
    .unwrap();
    assert!(
        matches!(&request, AgentRequest::Changes(AgentChangesRequest::Read { cursor: None, limit, .. }) if limit.value() == 50)
    );
    let mut bytes = Vec::new();
    write_agent_request(&mut bytes, &request).unwrap();
    assert_eq!(bytes[7], 23);
    assert_eq!(
        read_agent_request(&mut Cursor::new(bytes)).unwrap(),
        request
    );
    for value in [
        page(),
        document(json!({"kind":"current","cursor":cursor(),"revision":0})),
        document(json!({"kind":"reset-required","cursor":cursor(),"revision":1000})),
        document(json!({"kind":"rejected","reason":"cursor-invalid"})),
        document(json!({"kind":"unavailable","reason":"runtime-unavailable"})),
    ] {
        let result = parse_agent_result(&serde_json::to_vec(&value).unwrap()).unwrap();
        let mut bytes = Vec::new();
        write_agent_result(&mut bytes, &result).unwrap();
        assert_eq!(bytes[7], 24);
        assert_eq!(read_agent_result(&mut Cursor::new(bytes)).unwrap(), result);
    }
}

#[test]
fn changes_reject_invalid_inputs_and_dishonest_progress() {
    for patch in [
        json!({"cursor":null}),
        json!({"limit":0}),
        json!({"limit":101}),
        json!({"limit":1.5}),
        json!({"extra":true}),
    ] {
        let mut value = json!({"kind":"agent.session.changes.request","requestId":ID});
        value
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(parse_agent_request(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    for (pointer, invalid) in [
        ("/result/outcome/changes", json!([])),
        ("/result/outcome/changes/0/revision", json!(0)),
        ("/result/outcome/changes/1/revision", json!(3)),
        ("/result/outcome/changes/0/transition", json!("invented")),
        (
            "/result/outcome/changes/0/session/pendingRequestCount",
            json!(129),
        ),
        ("/result/outcome/hasMore", json!(false)),
        ("/result/outcome/runtimeRevision", json!(1)),
        ("/result/outcome/observedThroughRevision", json!(1)),
    ] {
        let mut value = page();
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            parse_agent_result(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn changes_bound_declared_length_before_reading_body_and_reject_wrong_kind() {
    let mut header = b"OBAP\x00\x01\x00\x18".to_vec();
    header.extend(
        u32::try_from(MAXIMUM_AGENT_CHANGES_RESULT_BYTES + 1)
            .unwrap()
            .to_be_bytes(),
    );
    assert!(matches!(
        read_agent_result(&mut Cursor::new(header)),
        Err(AgentWireError::Contract(
            AgentContractError::EncodedValueTooLarge { .. }
        ))
    ));
    let result = parse_agent_result(&serde_json::to_vec(&page()).unwrap()).unwrap();
    let mut bytes = Vec::new();
    write_agent_result(&mut bytes, &result).unwrap();
    bytes[7] = 2;
    assert!(matches!(
        read_agent_result(&mut Cursor::new(bytes)),
        Err(AgentWireError::InvalidHeader)
    ));
}

#[test]
fn changes_full_page_worst_case_payload_stays_inside_the_declared_ceiling() {
    let mut entries = Vec::new();
    for revision in (MAXIMUM_SAFE_INTEGER - 99)..=MAXIMUM_SAFE_INTEGER {
        let mut entry = change(revision);
        entry["transition"] = json!("presentation-visibility-changed");
        entry["session"]["sessionId"] = json!("s".repeat(128));
        entry["session"]["parentSessionId"] = json!("p".repeat(128));
        entry["session"]["providerId"] = json!("a".repeat(64));
        entry["session"]["pendingRequestCount"] = json!(128);
        entry["session"]["observation"]["source"] = json!("heuristic");
        entry["session"]["observation"]["fidelity"] = json!("heuristic");
        entry["session"]["observation"]["freshness"] = json!("disconnected");
        entry["session"]["observation"]["generation"] = json!(MAXIMUM_SAFE_INTEGER);
        entry["session"]["observation"]["sequence"] = json!(MAXIMUM_SAFE_INTEGER);
        entry["session"]["status"] =
            json!({"kind":"working","activity":"thinking","phase":"waiting-external"});
        entries.push(entry);
    }
    let value = document(
        json!({"kind":"changes","changes":entries,"cursor":cursor(),"observedThroughRevision":MAXIMUM_SAFE_INTEGER,"runtimeRevision":MAXIMUM_SAFE_INTEGER,"hasMore":false}),
    );
    let encoded = serde_json::to_vec(&value).unwrap();
    let result = parse_agent_result(&encoded).unwrap();
    assert!(encoded.len() < MAXIMUM_AGENT_CHANGES_RESULT_BYTES);
    let mut bytes = Vec::new();
    write_agent_result(&mut bytes, &result).unwrap();
    assert_eq!(read_agent_result(&mut Cursor::new(bytes)).unwrap(), result);
    let mut overflow = value;
    let entries = overflow["result"]["outcome"]["changes"]
        .as_array_mut()
        .unwrap();
    entries.push(change(1));
    assert!(parse_agent_result(&serde_json::to_vec(&overflow).unwrap()).is_err());
}
