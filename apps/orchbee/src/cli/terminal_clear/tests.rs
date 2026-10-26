use super::*;
#[test]
fn clear_receipt_rejects_impossible_outcomes_zero_revision_and_missing_identity() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.terminal.clear.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let id = "11111111111111111111111111111111";
    let base = json!({"schema":"orchbee.terminal.clear","version":1,"source":{"kind":"workbench-snapshot","sessionId":id,"revision":"1"},"result":{"terminal":{"terminalId":id,"workerId":id,"boundTo":{"presentationId":id,"panelId":id,"tabId":id,"terminalContextId":id}},"operationId":id,"outcome":{"kind":"cleared","generation":"2","revision":"1","cleanupConfirmed":true}}});
    validator.validate(&base).unwrap();
    for outcome in [
        json!({"kind":"outcome-unknown"}),
        json!({"kind":"cleared","generation":"2","revision":"1","cleanupConfirmed":false}),
        json!({"kind":"rejected","reason":"host-clear-alternate-screen"}),
    ] {
        let mut receipt = base.clone();
        receipt["result"]["outcome"] = outcome;
        validator.validate(&receipt).unwrap();
    }
    for outcome in [
        json!({"kind":"acquired","controllerId":id}),
        json!({"kind":"cleared","generation":"2","revision":"0","cleanupConfirmed":true}),
        json!({"kind":"cleared","generation":"2","revision":"1","cleanupConfirmed":"true"}),
    ] {
        let mut receipt = base.clone();
        receipt["result"]["outcome"] = outcome;
        assert!(!validator.is_valid(&receipt));
    }
    let mut receipt = base.clone();
    receipt["result"]
        .as_object_mut()
        .unwrap()
        .remove("operationId");
    assert!(!validator.is_valid(&receipt));
}

#[test]
fn clear_grammar_rejects_read_and_controller_flags_before_discovery() {
    use crate::invocation::{self, Request};
    let id = "11111111111111111111111111111111";
    let args = |tail: &[&str]| {
        let mut v = vec!["terminal", "clear", id];
        v.extend_from_slice(tail);
        v.into_iter()
            .map(Into::into)
            .collect::<Vec<std::ffi::OsString>>()
    };
    assert!(matches!(
        invocation::parse_request(args(&[]).into_iter()),
        Ok(Request::TerminalClear { .. })
    ));
    for extra in [
        &["--takeover"][..],
        &["--content-revision", "1"],
        &["--cursor", "0:0"],
        &["extra"],
    ] {
        assert!(invocation::parse_request(args(extra).into_iter()).is_err());
    }
}
