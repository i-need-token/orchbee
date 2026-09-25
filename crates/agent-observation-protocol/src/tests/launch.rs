use std::io::Cursor;

use serde_json::{Value, json};

use crate::*;

const ID: &str = "11111111111111111111111111111111";
const OTHER: &str = "22222222222222222222222222222222";

fn reservation() -> Value {
    json!({
        "reservationId": ID, "name": "alpha", "providerId": "codex",
        "binding": {"presentationId": OTHER, "generation": OTHER,
            "hostEpoch": OTHER, "hostTerminalId": OTHER},
        "remainingMs": 30000, "settleRemainingMs": 3000
    })
}

#[test]
fn launch_wire_correlates_each_operation_and_bounds_its_semantic_fields() {
    let requests = [
        json!({"kind":"agent.launch.reserve.request", "requestId":ID,
            "name":"alpha", "providerId":"codex", "presentationId":OTHER,
            "hostEpoch":OTHER, "hostTerminalId":OTHER, "ttlMs":30000}),
        json!({"kind":"agent.launch.inspect.request", "requestId":OTHER, "reservationId":ID}),
        json!({"kind":"agent.launch.complete.request", "requestId":OTHER,
            "reservationId":ID, "sessionId":"s1"}),
        json!({"kind":"agent.launch.cancel.request", "requestId":OTHER, "reservationId":ID}),
    ];
    let outcomes = [
        json!({"kind":"reserved", "reservation":reservation()}),
        json!({"kind":"pending", "reservation":reservation(), "candidateSessionId":null}),
        json!({"kind":"completed", "reservationId":ID, "name":"alpha", "sessionId":"s1"}),
        json!({"kind":"released", "reservationId":ID}),
    ];
    for (index, document) in requests.iter().enumerate() {
        let request = parse_agent_request(&serde_json::to_vec(document).unwrap()).unwrap();
        let mut bytes = Vec::new();
        write_agent_request(&mut bytes, &request).unwrap();
        assert_eq!(bytes[7], 15 + u8::try_from(index).unwrap() * 2);
        assert_eq!(
            read_agent_request(&mut Cursor::new(&bytes)).unwrap(),
            request
        );
        bytes[7] = if index == 0 { 17 } else { 15 };
        assert!(read_agent_request(&mut Cursor::new(bytes)).is_err());

        let result = json!({"kind":document["kind"].as_str().unwrap().replace("request","result"),
            "requestId":document["requestId"], "result":{"outcome":outcomes[index]}});
        let typed = parse_agent_result(&serde_json::to_vec(&result).unwrap()).unwrap();
        let mut bytes = Vec::new();
        write_agent_result(&mut bytes, &typed).unwrap();
        assert_eq!(bytes[7], 16 + u8::try_from(index).unwrap() * 2);
        assert_eq!(read_agent_result(&mut Cursor::new(bytes)).unwrap(), typed);

        let mut wrong = result.clone();
        wrong["result"]["outcome"] = outcomes[(index + 1) % outcomes.len()].clone();
        assert!(parse_agent_result(&serde_json::to_vec(&wrong).unwrap()).is_err());
    }
    for ttl in [0, MAXIMUM_LAUNCH_TTL_MS + 1] {
        let mut invalid = requests[0].clone();
        invalid["ttlMs"] = ttl.into();
        assert!(parse_agent_request(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    for field in ["remainingMs", "settleRemainingMs"] {
        let mut invalid = reservation();
        invalid[field] = (MAXIMUM_LAUNCH_TTL_MS + 1).into();
        let result = json!({"kind":"agent.launch.reserve.result", "requestId":ID,
            "result":{"outcome":{"kind":"reserved", "reservation":invalid}}});
        assert!(parse_agent_result(&serde_json::to_vec(&result).unwrap()).is_err());
    }
    let mut missing = requests[0].clone();
    missing.as_object_mut().unwrap().remove("hostEpoch");
    assert!(parse_agent_request(&serde_json::to_vec(&missing).unwrap()).is_err());
}

#[test]
fn pending_names_share_capacity_and_cannot_duplicate_committed_names_or_presentations() {
    let make = |bindings, pending| {
        json!({"kind":"agent.session.names.list.result", "requestId":ID,
        "result":{"outcome":{"kind":"listed", "bindings":bindings, "pending":pending}}})
    };
    let valid = make(json!([]), json!([reservation()]));
    assert!(parse_agent_result(&serde_json::to_vec(&valid).unwrap()).is_ok());
    let overlap = make(
        json!([{"name":"alpha","sessionId":"s1","lifecycle":"live"}]),
        json!([reservation()]),
    );
    assert!(parse_agent_result(&serde_json::to_vec(&overlap).unwrap()).is_err());
    let mut second = reservation();
    second["name"] = "beta".into();
    second["reservationId"] = OTHER.into();
    let duplicate_presentation = make(json!([]), json!([reservation(), second]));
    assert!(parse_agent_result(&serde_json::to_vec(&duplicate_presentation).unwrap()).is_err());
}

#[test]
fn maximum_pending_list_fits_its_frame_and_shares_the_committed_name_capacity() {
    let pending: Vec<_> = (0..512)
        .map(|index| {
            let mut entry = reservation();
            entry["name"] = json!(format!("a{index:031}"));
            entry["providerId"] = json!("p".repeat(64));
            entry["reservationId"] = json!(format!("{:032x}", index + 2));
            entry["binding"]["presentationId"] = json!(format!("{:032x}", index + 1));
            entry["remainingMs"] = json!(MAXIMUM_LAUNCH_TTL_MS);
            entry
        })
        .collect();
    let mut document = json!({"kind":"agent.session.names.list.result","requestId":ID,"result":{"outcome":{"kind":"listed","bindings":[],"pending":pending}}});
    let bytes = serde_json::to_vec(&document).unwrap();
    assert!(bytes.len() <= MAXIMUM_AGENT_NAMES_RESULT_BYTES);
    let result = parse_agent_result(&bytes).unwrap();
    let mut frame = Vec::new();
    write_agent_result(&mut frame, &result).unwrap();
    assert_eq!(read_agent_result(&mut Cursor::new(frame)).unwrap(), result);
    document["result"]["outcome"]["bindings"] =
        json!([{"name":"committed","sessionId":"s1","lifecycle":"live"}]);
    assert!(parse_agent_result(&serde_json::to_vec(&document).unwrap()).is_err());
}

#[test]
fn management_status_has_closed_phases_correlated_frames_and_a_small_worst_case() {
    let request = parse_agent_request(
        &serde_json::to_vec(
            &json!({"kind":"agent.launch.status.request","requestId":ID,"sessionId":"s1"}),
        )
        .unwrap(),
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_agent_request(&mut bytes, &request).unwrap();
    assert_eq!(bytes[7], 25);
    assert_eq!(
        read_agent_request(&mut Cursor::new(bytes)).unwrap(),
        request
    );
    let mut maximum = reservation();
    maximum["name"] = json!("a".repeat(32));
    maximum["providerId"] = json!("p".repeat(64));
    for state in [
        json!({"phase":"unmanaged"}),
        json!({"phase":"pending","reservation":maximum,"blocked":false}),
        json!({"phase":"active","reservationId":ID,"providerId":"codex","binding":reservation()["binding"]}),
    ] {
        let document = json!({"kind":"agent.launch.status.result","requestId":ID,"result":{"outcome":{
            "kind":"status","sessionId":"s".repeat(128),"revision":9007199254740991_u64,"management":state}}});
        let encoded = serde_json::to_vec(&document).unwrap();
        assert!(encoded.len() <= MAXIMUM_AGENT_SMALL_OUTCOME_BYTES);
        let result = parse_agent_result(&encoded).unwrap();
        let mut bytes = Vec::new();
        write_agent_result(&mut bytes, &result).unwrap();
        assert_eq!(bytes[7], 26);
        assert_eq!(read_agent_result(&mut Cursor::new(bytes)).unwrap(), result);
        let mut wrong = document.clone();
        wrong["kind"] = json!("agent.launch.inspect.result");
        assert!(parse_agent_result(&serde_json::to_vec(&wrong).unwrap()).is_err());
        let mut extra = document;
        extra["result"]["outcome"]["management"]["interactiveReady"] = json!(true);
        assert!(parse_agent_result(&serde_json::to_vec(&extra).unwrap()).is_err());
    }
    let mut header = Vec::from(AGENT_MAGIC);
    header.extend_from_slice(&1_u16.to_be_bytes());
    header.extend_from_slice(&26_u16.to_be_bytes());
    header.extend_from_slice(&1025_u32.to_be_bytes());
    assert!(read_agent_result(&mut Cursor::new(header)).is_err());
}
