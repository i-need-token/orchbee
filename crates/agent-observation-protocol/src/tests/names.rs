use std::io::Cursor;

use serde_json::json;

use crate::*;

const ID: &str = "11111111111111111111111111111111";

#[test]
fn all_names_requests_and_outcomes_round_trip_through_the_public_codec() {
    let requests = [
        json!({"kind":"agent.session.resolve.request","requestId":ID,"target":{"kind":"auto","target":"s1"}}),
        json!({"kind":"agent.session.names.list.request","requestId":ID}),
        json!({"kind":"agent.session.name.assign.request","requestId":ID,"target":{"kind":"session","sessionId":"s1"},"name":"alpha"}),
        json!({"kind":"agent.session.name.release.request","requestId":ID,"target":{"kind":"name","name":"alpha"}}),
    ];
    for (index, document) in requests.iter().enumerate() {
        let request = parse_agent_request(&serde_json::to_vec(document).unwrap()).unwrap();
        let mut bytes = Vec::new();
        write_agent_request(&mut bytes, &request).unwrap();
        assert_eq!(bytes[7], 7 + u8::try_from(index).unwrap() * 2);
        assert_eq!(
            read_agent_request(&mut Cursor::new(bytes)).unwrap(),
            request
        );
    }
    let outcomes = [
        (
            "resolve",
            vec![
                json!({"kind":"resolved","sessionId":"s1","resolvedBy":"name"}),
                json!({"kind":"rejected","reason":"not-found"}),
                json!({"kind":"rejected","reason":"read-denied"}),
            ],
        ),
        (
            "names.list",
            vec![
                json!({"kind":"listed","bindings":[{"name":"alpha","sessionId":"s1","lifecycle":"live"}]}),
                json!({"kind":"rejected","reason":"read-denied"}),
            ],
        ),
        (
            "name.assign",
            vec![
                json!({"kind":"assigned","sessionId":"s1","name":"alpha","previousName":null}),
                json!({"kind":"assigned","sessionId":"s1","name":"alpha","previousName":"previous"}),
                json!({"kind":"unchanged","sessionId":"s1","name":"alpha"}),
                json!({"kind":"rejected","reason":"name-in-use","conflictSessionId":"s2"}),
                json!({"kind":"rejected","reason":"session-ended","conflictSessionId":null}),
            ],
        ),
        (
            "name.release",
            vec![
                json!({"kind":"released","sessionId":"s1","name":"alpha"}),
                json!({"kind":"unchanged","sessionId":"s1"}),
                json!({"kind":"rejected","reason":"name-not-found"}),
            ],
        ),
    ];
    for (index, (operation, mut cases)) in outcomes.into_iter().enumerate() {
        cases.push(json!({"kind":"unavailable","reason":"runtime-unavailable"}));
        for outcome in cases {
            let document = json!({"kind":format!("agent.session.{operation}.result"),"requestId":ID,"result":{"outcome":outcome}});
            let result = parse_agent_result(&serde_json::to_vec(&document).unwrap()).unwrap();
            let mut bytes = Vec::new();
            write_agent_result(&mut bytes, &result).unwrap();
            assert_eq!(bytes[7], 8 + u8::try_from(index).unwrap() * 2);
            assert_eq!(read_agent_result(&mut Cursor::new(bytes)).unwrap(), result);
        }
    }
}

#[test]
fn small_names_results_reject_hostile_lengths_before_reading_the_body() {
    for kind in [8, 12, 14] {
        let mut header = Vec::from(&b"OBAP\0\x01\0\0"[..]);
        header[7] = kind;
        header.extend_from_slice(&1025_u32.to_be_bytes());
        assert!(matches!(
            read_agent_result(&mut Cursor::new(header)),
            Err(AgentWireError::Contract(
                AgentContractError::EncodedValueTooLarge {
                    maximum: 1024,
                    actual: 1025
                }
            ))
        ));
    }
}

#[test]
fn names_invariants_reject_ghost_duplicate_and_mismatched_conflict_rows() {
    let row = json!({"name":"alpha","sessionId":"s1","lifecycle":"live"});
    for rows in [
        json!([row.clone(), row]),
        json!([{"name":"alpha","sessionId":"s1","lifecycle":"ended"}]),
        json!([{"name":"alpha","sessionId":"s1","lifecycle":"live"},{"name":"beta","sessionId":"s1","lifecycle":"live"}]),
    ] {
        let document = json!({"kind":"agent.session.names.list.result","requestId":ID,"result":{"outcome":{"kind":"listed","bindings":rows}}});
        assert!(parse_agent_result(&serde_json::to_vec(&document).unwrap()).is_err());
    }
    for (reason, conflict) in [("name-in-use", json!(null)), ("session-ended", json!("s1"))] {
        let document = json!({"kind":"agent.session.name.assign.result","requestId":ID,"result":{"outcome":{"kind":"rejected","reason":reason,"conflictSessionId":conflict}}});
        assert!(parse_agent_result(&serde_json::to_vec(&document).unwrap()).is_err());
    }
}
