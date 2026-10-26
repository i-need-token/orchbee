use crate::{
    ClientMessage, FRAME_HEADER_BYTES, HistoryDemand, ProjectionDemand, RequestId, ServerMessage,
    SessionId, TerminalId, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    TerminalViewerResult, ViewerId, ViewerRejectedReason, ViewerUnavailableReason,
    ViewportRowOrigin, WorkerId, decode_client_frame, decode_server_frame, encode_client_frame,
    encode_server_frame,
};
use std::num::NonZeroU64;

const SHARED_IDENTITY: &str = "11111111111111111111111111111111";

#[test]
fn attached_viewer_requires_the_physical_terminal_identity() {
    let value = serde_json::json!({"kind":"attached","attachmentId":SHARED_IDENTITY,"hostEpoch":SHARED_IDENTITY,"hostTerminalId":"22222222222222222222222222222222"});
    let outcome: TerminalViewerOutcome = serde_json::from_value(value.clone()).unwrap();
    let message = ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
        RequestId::parse(SHARED_IDENTITY).unwrap(),
        outcome,
    ));
    assert_eq!(
        decode_server_frame(&encode_server_frame(&message).unwrap()).unwrap(),
        message
    );
    let mut missing = value.clone();
    missing.as_object_mut().unwrap().remove("hostTerminalId");
    assert!(serde_json::from_value::<TerminalViewerOutcome>(missing).is_err());
    let mut zero = value;
    zero["hostTerminalId"] = serde_json::json!("00000000000000000000000000000000");
    assert!(serde_json::from_value::<TerminalViewerOutcome>(zero).is_err());
}

#[test]
fn attach_binding_round_trips_equal_bytes_through_distinct_identity_domains()
-> Result<(), Box<dyn std::error::Error>> {
    let message = ClientMessage::TerminalViewer(TerminalViewerRequest::new(
        RequestId::parse(SHARED_IDENTITY)?,
        TerminalViewerAction::AttachBinding {
            session_id: SessionId::parse(SHARED_IDENTITY)?,
            worker_id: WorkerId::parse(SHARED_IDENTITY)?,
            terminal_id: TerminalId::parse(SHARED_IDENTITY)?,
        },
    ));

    let frame = encode_client_frame(&message)?;
    assert_eq!(
        frame.get(FRAME_HEADER_BYTES..),
        Some(
            br#"{"kind":"terminal.viewer","protocolVersion":1,"requestId":"11111111111111111111111111111111","action":{"kind":"attach-binding","sessionId":"11111111111111111111111111111111","workerId":"11111111111111111111111111111111","terminalId":"11111111111111111111111111111111"}}"#
                .as_slice()
        )
    );
    assert_eq!(decode_client_frame(&frame)?, message);
    Ok(())
}

#[test]
fn workbench_unavailable_round_trips_only_as_a_viewer_outcome()
-> Result<(), Box<dyn std::error::Error>> {
    let message = ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
        RequestId::parse(SHARED_IDENTITY)?,
        TerminalViewerOutcome::Unavailable {
            reason: ViewerUnavailableReason::WorkbenchUnavailable,
        },
    ));

    let frame = encode_server_frame(&message)?;
    assert_eq!(
        frame.get(FRAME_HEADER_BYTES..),
        Some(
            br#"{"kind":"terminal.viewer.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"unavailable","reason":"workbench-unavailable"}}"#
                .as_slice()
        )
    );
    assert_eq!(decode_server_frame(&frame)?, message);
    Ok(())
}

#[test]
fn attachment_id_accessor_distinguishes_server_issued_viewer_actions()
-> Result<(), Box<dyn std::error::Error>> {
    let attachment_id = ViewerId::parse("33333333333333333333333333333333")?;
    let attach = TerminalViewerAction::Attach {
        terminal_create_request_id: RequestId::parse(SHARED_IDENTITY)?,
    };
    let attach_binding = TerminalViewerAction::AttachBinding {
        session_id: SessionId::parse(SHARED_IDENTITY)?,
        worker_id: WorkerId::parse(SHARED_IDENTITY)?,
        terminal_id: TerminalId::parse(SHARED_IDENTITY)?,
    };
    let poll = TerminalViewerAction::Poll {
        attachment_id: attachment_id.clone(),
    };

    assert_eq!(attach.attachment_id(), None);
    assert_eq!(attach_binding.attachment_id(), None);
    assert_eq!(poll.attachment_id(), Some(&attachment_id));
    Ok(())
}

const ATTACHMENT: &str = "33333333333333333333333333333333";

fn attachment() -> Result<ViewerId, Box<dyn std::error::Error>> {
    Ok(ViewerId::parse(ATTACHMENT)?)
}

/// The five history-viewport actions keep their exact canonical JSON shape:
/// kebab-case kinds, camelCase fields, and decimal-string u64 fields.
#[test]
fn history_viewport_actions_encode_their_canonical_json_shape()
-> Result<(), Box<dyn std::error::Error>> {
    let attachment_id = attachment()?;
    for (action, payload) in [
        (
            TerminalViewerAction::SetProjectionDemand {
                attachment_id: attachment_id.clone(),
                demand: ProjectionDemand::Paused,
            },
            r#"{"kind":"set-projection-demand","attachmentId":"33333333333333333333333333333333","demand":"paused"}"#,
        ),
        (
            TerminalViewerAction::SetHistoryDemand {
                attachment_id: attachment_id.clone(),
                demand: HistoryDemand::ViewingHistory,
            },
            r#"{"kind":"set-history-demand","attachmentId":"33333333333333333333333333333333","demand":"viewing-history"}"#,
        ),
        (
            TerminalViewerAction::ReadViewportRows {
                detail: super::ViewerRowDetail::Text,
                attachment_id: attachment_id.clone(),
                origin: ViewportRowOrigin::Tail { rows_before: 30 },
                row_count: 24,
                revision: 7,
                generation: NonZeroU64::MIN,
                request_sequence: NonZeroU64::MIN,
                maximum_bytes: 8 * 1024 * 1024,
            },
            r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"tail","rowsBefore":30},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":8388608}"#,
        ),
        (
            TerminalViewerAction::ReadViewportRows {
                detail: super::ViewerRowDetail::Text,
                attachment_id: attachment_id.clone(),
                origin: ViewportRowOrigin::Committed { delta: -12 },
                row_count: 24,
                revision: 7,
                generation: NonZeroU64::MIN,
                request_sequence: NonZeroU64::MIN,
                maximum_bytes: 112,
            },
            r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"committed","delta":-12},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":112}"#,
        ),
        (
            TerminalViewerAction::RowsApplied {
                attachment_id: attachment_id.clone(),
                request_sequence: NonZeroU64::MIN,
                generation: NonZeroU64::MIN,
                revision: 7,
            },
            r#"{"kind":"rows-applied","attachmentId":"33333333333333333333333333333333","requestSequence":"1","generation":"1","revision":"7"}"#,
        ),
        (
            TerminalViewerAction::HistoryObserved {
                attachment_id: attachment_id.clone(),
                sequence: NonZeroU64::MIN,
                generation: NonZeroU64::MIN,
                revision: 7,
            },
            r#"{"kind":"history-observed","attachmentId":"33333333333333333333333333333333","sequence":"1","generation":"1","revision":"7"}"#,
        ),
    ] {
        let message = ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            RequestId::parse(SHARED_IDENTITY)?,
            action,
        ));
        let frame = encode_client_frame(&message)?;
        let expected = format!(
            r#"{{"kind":"terminal.viewer","protocolVersion":1,"requestId":"{SHARED_IDENTITY}","action":{payload}}}"#
        );
        assert_eq!(frame.get(FRAME_HEADER_BYTES..), Some(expected.as_bytes()));
        assert_eq!(decode_client_frame(&frame)?, message);
    }
    Ok(())
}

/// Semantic bounds the JSON shape cannot express reject at decode, and the
/// demand, origin, and acknowledgement enums stay closed.
#[test]
fn history_viewport_actions_reject_out_of_bounds_and_unknown_values()
-> Result<(), Box<dyn std::error::Error>> {
    for action in [
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"tail","rowsBefore":30},"rowCount":0,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":8388608}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"tail","rowsBefore":30},"rowCount":4097,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":8388608}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"tail","rowsBefore":30},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":111}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"tail","rowsBefore":30},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":8388609}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"sideways"},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":8388608}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"tail","rowsBefore":30},"rowCount":24,"revision":"7","generation":"0","requestSequence":"1","maximumBytes":8388608}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"tail","rowsBefore":30},"rowCount":24,"revision":"7","generation":"1","requestSequence":"0","maximumBytes":8388608}"#,
        r#"{"kind":"set-projection-demand","attachmentId":"33333333333333333333333333333333","demand":"hibernating"}"#,
        r#"{"kind":"set-history-demand","attachmentId":"33333333333333333333333333333333","demand":"detached"}"#,
        r#"{"kind":"rows-applied","attachmentId":"33333333333333333333333333333333","requestSequence":"0","generation":"1","revision":"7"}"#,
        r#"{"kind":"history-observed","attachmentId":"33333333333333333333333333333333","sequence":"0","generation":"1","revision":"7"}"#,
        r#"{"kind":"rows-applied","attachmentId":"33333333333333333333333333333333","requestSequence":"1","generation":"0","revision":"7"}"#,
        r#"{"kind":"rows-applied","attachmentId":"33333333333333333333333333333333","requestSequence":"1.5","generation":"1","revision":"7"}"#,
        r#"{"kind":"rows-applied","attachmentId":"33333333333333333333333333333333","requestSequence":"18446744073709551616","generation":"1","revision":"7"}"#,
        r#"{"kind":"rows-applied","attachmentId":"33333333333333333333333333333333","requestSequence":1,"generation":"1","revision":"7"}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"committed","delta":1.5},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":112}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"committed","delta":2147483648},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":112}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"committed","delta":-2147483649},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":112}"#,
        r#"{"kind":"set-projection-demand","attachmentId":"33333333333333333333333333333333","demand":"paused","extra":true}"#,
        r#"{"kind":"read-viewport-rows","attachmentId":"33333333333333333333333333333333","origin":{"kind":"committed","delta":-12,"extra":1},"rowCount":24,"revision":"7","generation":"1","requestSequence":"1","maximumBytes":112}"#,
        r#"{"kind":"scroll-history","attachmentId":"33333333333333333333333333333333"}"#,
    ] {
        let payload = format!(
            r#"{{"kind":"terminal.viewer","protocolVersion":1,"requestId":"{SHARED_IDENTITY}","action":{action}}}"#
        );
        let mut frame = b"OBCS\x00\x01".to_vec();
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(payload.as_bytes());
        assert!(decode_client_frame(&frame).is_err(), "{payload}");
    }
    Ok(())
}

/// `host-rejected` is additive to the closed viewer rejection vocabulary and
/// stays a server outcome, never an action.
#[test]
fn host_rejected_round_trips_as_viewer_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let message = ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
        RequestId::parse(SHARED_IDENTITY)?,
        TerminalViewerOutcome::Rejected {
            reason: ViewerRejectedReason::HostRejected,
        },
    ));
    let frame = encode_server_frame(&message)?;
    assert_eq!(
        frame.get(FRAME_HEADER_BYTES..),
        Some(
            br#"{"kind":"terminal.viewer.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"rejected","reason":"host-rejected"}}"#
                .as_slice()
        )
    );
    assert_eq!(decode_server_frame(&frame)?, message);
    Ok(())
}

#[test]
fn viewport_render_detail_is_explicit_and_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut value = serde_json::json!({"kind":"read-viewport-rows", "attachmentId":ATTACHMENT,
        "origin":{"kind":"tail","rowsBefore":3}, "rowCount":24, "revision":"7",
        "generation":"1", "requestSequence":"1", "maximumBytes":8388608, "detail":"render"});
    let action: TerminalViewerAction = serde_json::from_value(value.clone())?;
    assert!(matches!(
        action,
        TerminalViewerAction::ReadViewportRows {
            detail: super::ViewerRowDetail::Render,
            ..
        }
    ));
    assert_eq!(serde_json::to_value(action)?, value);
    value["detail"] = serde_json::json!("unknown");
    assert!(serde_json::from_value::<TerminalViewerAction>(value).is_err());
    Ok(())
}

fn raw_viewer_action_frame(action: &str) -> Vec<u8> {
    let payload = format!(
        r#"{{"kind":"terminal.viewer","protocolVersion":1,"requestId":"{SHARED_IDENTITY}","action":{action}}}"#
    );
    let mut frame = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    frame.extend_from_slice(b"OBCS");
    frame.extend_from_slice(&1_u16.to_be_bytes());
    frame.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("bounded fixture")
            .to_be_bytes(),
    );
    frame.extend_from_slice(payload.as_bytes());
    frame
}

/// Viewer selection uses the same flat action vocabulary as Terminal Protocol:
/// exact attachment identity is explicit on OBCS, while every positive counter
/// remains canonical decimal text.
#[test]
fn selection_actions_round_trip_their_exact_canonical_json()
-> Result<(), Box<dyn std::error::Error>> {
    let actions = [
        format!(
            r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":2,"point":{{"column":3,"row":4}},"position":{{"x":-5,"y":6}},"selectOutput":true}}"#
        ),
        format!(
            r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"update","geometry":{{"cellWidth":8,"columns":80,"paddingLeft":2,"screenHeight":480}},"point":{{"column":7,"row":8}},"position":{{"x":9,"y":-10}},"rectangle":false}}"#
        ),
        format!(
            r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"end","point":{{"column":11,"row":12}}}}"#
        ),
        format!(
            r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"cancel"}}"#
        ),
        format!(
            r#"{{"kind":"selection-export","attachmentId":"{SHARED_IDENTITY}","exportSequence":"10","format":"plain"}}"#
        ),
        format!(
            r#"{{"kind":"selection-export-chunk-applied","attachmentId":"{SHARED_IDENTITY}","exportSequence":"10","chunkSequence":42}}"#
        ),
        format!(
            r#"{{"kind":"selection-export-cancel","attachmentId":"{SHARED_IDENTITY}","exportSequence":"10","reason":"clipboard-failed"}}"#
        ),
    ];
    let attachment_id = ViewerId::parse(SHARED_IDENTITY)?;
    for action in actions {
        let frame = raw_viewer_action_frame(&action);
        let decoded = decode_client_frame(&frame).map_err(|error| {
            std::io::Error::other(format!("selection action {action} rejected: {error}"))
        })?;
        assert_eq!(encode_client_frame(&decoded)?, frame);
        let ClientMessage::TerminalViewer(request) = decoded else {
            panic!("selection action decoded outside the viewer plane")
        };
        assert_eq!(request.action().attachment_id(), Some(&attachment_id));
    }

    for reason in ["clipboard-failed", "disposed", "replaced", "user"] {
        let action = format!(
            r#"{{"kind":"selection-export-cancel","attachmentId":"{SHARED_IDENTITY}","exportSequence":"10","reason":"{reason}"}}"#
        );
        let frame = raw_viewer_action_frame(&action);
        assert_eq!(encode_client_frame(&decode_client_frame(&frame)?)?, frame);
    }
    Ok(())
}

/// Shape-valid JSON still fails closed before Server routing when it exceeds a
/// Terminal Protocol gesture bound or names an open selection vocabulary.
#[test]
fn selection_actions_reject_semantic_bounds_unknown_fields_and_open_enums() {
    let invalid = [
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"0","action":"cancel"}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"future"}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":0,"point":{{"column":3,"row":4}},"position":{{"x":-5,"y":6}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":4,"point":{{"column":3,"row":4}},"position":{{"x":-5,"y":6}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":2,"point":{{"column":4096,"row":4}},"position":{{"x":-5,"y":6}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":2,"point":{{"column":3,"row":4096}},"position":{{"x":-5,"y":6}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":2,"point":{{"column":3,"row":4}},"position":{{"x":65536,"y":6}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":2,"point":{{"column":3,"row":4}},"position":{{"x":-5,"y":-65536}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"update","geometry":{{"cellWidth":0,"columns":80,"paddingLeft":2,"screenHeight":480}},"point":{{"column":7,"row":8}},"position":{{"x":9,"y":-10}},"rectangle":false}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"update","geometry":{{"cellWidth":8,"columns":4097,"paddingLeft":2,"screenHeight":480}},"point":{{"column":7,"row":8}},"position":{{"x":9,"y":-10}},"rectangle":false}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"update","geometry":{{"cellWidth":8,"columns":80,"paddingLeft":2,"screenHeight":0}},"point":{{"column":7,"row":8}},"position":{{"x":9,"y":-10}},"rectangle":false}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"cancel","extra":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"cancel","clickCount":2}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":null,"point":{{"column":3,"row":4}},"position":{{"x":-5,"y":6}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":2,"point":{{"column":3,"row":4}},"position":{{"x":-5,"y":6}},"selectOutput":true,"rectangle":false}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","action":"start","clickCount":2,"point":{{"column":3,"row":4}},"position":{{"x":-5,"y":6}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":2,"clickCount":2,"point":{{"column":3,"row":4}},"position":{{"x":-5,"y":6}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-gesture","attachmentId":"{SHARED_IDENTITY}","generation":"9","action":"start","clickCount":2,"point":{{"column":3,"row":4}},"selectOutput":true}}"#),
        format!(r#"{{"kind":"selection-export","attachmentId":"{SHARED_IDENTITY}","exportSequence":"0","format":"plain"}}"#),
        format!(r#"{{"kind":"selection-export","attachmentId":"{SHARED_IDENTITY}","exportSequence":"10","format":"html"}}"#),
        format!(r#"{{"kind":"selection-export-chunk-applied","attachmentId":"{SHARED_IDENTITY}","exportSequence":"10","chunkSequence":4294967296}}"#),
        format!(r#"{{"kind":"selection-export-cancel","attachmentId":"{SHARED_IDENTITY}","exportSequence":"10","reason":"future"}}"#),
        r#"{"kind":"selection-export","attachmentId":"00000000000000000000000000000000","exportSequence":"10","format":"plain"}"#.to_string(),
    ];
    for action in invalid {
        let frame = raw_viewer_action_frame(&action);
        assert!(decode_client_frame(&frame).is_err(), "accepted {action}");
    }
}
