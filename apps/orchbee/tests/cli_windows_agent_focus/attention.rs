//! Selected Client attention observations and their boot authentication, shared by the Client reads.
use super::*;
use orchbee_agent_observation_protocol::{AgentResult, write_agent_result};

pub(super) fn token() -> String {
    format!("v1.{}.{}", "A".repeat(59), "A".repeat(43))
}

pub(super) fn observation(state: Value) -> Value {
    json!({"cursor":token(),"revision":3,"outerFocused":false,"entries":[{"sessionId":"sess-alpha","state":state}]})
}

pub(super) fn authenticate(stream: &mut LocalStream, allowed: bool) {
    let AgentRequest::Changes(request) = read_agent_request(stream).unwrap() else {
        panic!("cursor authentication before exposure")
    };
    assert_eq!(json!(request)["cursor"], token());
    assert_eq!(json!(request)["limit"], 1);
    let outcome = if allowed {
        json!({"kind":"current","cursor":token(),"revision":3})
    } else {
        json!({"kind":"rejected","reason":"cursor-invalid"})
    };
    write_agent_result(stream, &AgentResult::Changes(serde_json::from_value(json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":outcome}})).unwrap())).unwrap();
}
