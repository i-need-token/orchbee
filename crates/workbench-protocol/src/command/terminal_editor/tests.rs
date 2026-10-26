use super::*;
use crate::{
    WorkbenchRequest, WorkbenchResponse, parse_workbench_request, parse_workbench_response,
    serialize_workbench_request, serialize_workbench_response,
};

fn request() -> serde_json::Value {
    serde_json::json!({"kind":"workbench.terminal-editor", "protocolVersion":1,
        "requestId":"00000000000000000000000000000001","sessionId":"00000000000000000000000000000002",
        "action":{"kind":"open","popupId":"00000000000000000000000000000003",
        "spec":{"source":{"terminalContextId":"00000000000000000000000000000004","tabId":"00000000000000000000000000000005","panelId":"00000000000000000000000000000006",
            "member":{"presentationId":"00000000000000000000000000000007","binding":{"workerId":"00000000000000000000000000000008","terminalId":"00000000000000000000000000000009"},"attachmentGeneration":"1","incarnation":0,"viewport":{"state":"unavailable","reason":"frame-unavailable"}}}},
        "capture":{"attachmentId":"0000000000000000000000000000000a","generation":"18446744073709551615","revision":"0"},
        "geometry":{"columns":80,"rows":24,"cellPixelWidth":0,"cellPixelHeight":0}}})
}

#[test]
fn editor_intent_and_action_correlated_results_are_closed_and_preserve_full_u64() {
    let json = request();
    let parsed = parse_workbench_request(&serde_json::to_vec(&json).unwrap()).unwrap();
    assert_eq!(
        parse_workbench_request(&serialize_workbench_request(&parsed).unwrap()).unwrap(),
        parsed
    );
    let WorkbenchRequest::TerminalEditor(request) = parsed else {
        panic!()
    };
    let mut result = TerminalEditorResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        outcome: TerminalEditorOutcome::Opened {
            popup_id: request.action.popup_id().clone(),
            title: "editor.exe".into(),
        },
    };
    assert!(result.answers(&request));
    let wire = WorkbenchResponse::TerminalEditor(result.clone());
    assert_eq!(
        parse_workbench_response(&serialize_workbench_response(&wire).unwrap()).unwrap(),
        wire
    );
    let mut frame = Vec::new();
    crate::write_workbench_request(
        &mut frame,
        &WorkbenchRequest::TerminalEditor(request.clone()),
    )
    .unwrap();
    assert_eq!(&frame[6..8], &51u16.to_be_bytes());
    assert_eq!(
        crate::read_workbench_request(&mut frame.as_slice()).unwrap(),
        WorkbenchRequest::TerminalEditor(request.clone())
    );
    frame.clear();
    crate::write_workbench_response(&mut frame, &wire).unwrap();
    assert_eq!(&frame[6..8], &52u16.to_be_bytes());
    assert_eq!(
        crate::read_workbench_response(&mut frame.as_slice()).unwrap(),
        wire
    );
    result.session_id = SessionId::parse("000000000000000000000000000000ff").unwrap();
    assert!(!result.answers(&request));
    result.session_id = request.session_id.clone();
    result.outcome = TerminalEditorOutcome::Closed {
        popup_id: request.action.popup_id().clone(),
    };
    assert!(!result.answers(&request));
    let read = TerminalEditorRequest {
        action: TerminalEditorAction::Read {
            popup_id: request.action.popup_id().clone(),
        },
        ..request
    };
    assert!(result.answers(&read));
    result.outcome = TerminalEditorOutcome::Unknown {
        popup_id: WorkbenchRequestId::parse("000000000000000000000000000000ff").unwrap(),
    };
    assert!(!result.answers(&read));
    for (pointer, value) in [
        ("/action/capture/generation", serde_json::json!("0")),
        ("/action/capture/revision", serde_json::json!("01")),
        (
            "/action/capture/revision",
            serde_json::json!("18446744073709551616"),
        ),
        ("/action/capture/revision", serde_json::json!(1)),
        (
            "/action/capture/attachmentId",
            serde_json::json!("00000000000000000000000000000000"),
        ),
        ("/action/geometry/columns", serde_json::json!(0)),
        (
            "/action/spec/source/member",
            serde_json::json!({"presentationId":"00000000000000000000000000000007","selectionGeneration":"1"}),
        ),
    ] {
        let mut bad = json.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            parse_workbench_request(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{pointer}"
        );
    }
    for key in ["program", "environment", "path", "text"] {
        let mut bad = json.clone();
        bad["action"][key] = serde_json::json!("untrusted");
        assert!(parse_workbench_request(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut bad = json;
    bad["action"]["spec"]["source"]["member"]["viewport"] =
        serde_json::json!({"state":"unavailable","reason":"not-attached"});
    let encoded = serde_json::to_vec(&bad).unwrap();
    let mut padded = encoded.clone();
    padded.extend(std::iter::repeat_n(b' ', MAXIMUM_TERMINAL_EDITOR_BYTES));
    assert!(parse_workbench_request(&padded).is_err());
}
