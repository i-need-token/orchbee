use crate::{
    ClientMessage, FRAME_HEADER_BYTES, HistoryDemand, ProjectionDemand, RequestId, ServerMessage,
    SiteId, TerminalId, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
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
            site_id: SiteId::parse(SHARED_IDENTITY)?,
            worker_id: WorkerId::parse(SHARED_IDENTITY)?,
            terminal_id: TerminalId::parse(SHARED_IDENTITY)?,
        },
    ));

    let frame = encode_client_frame(&message)?;
    assert_eq!(
        frame.get(FRAME_HEADER_BYTES..),
        Some(
            br#"{"kind":"terminal.viewer","protocolVersion":2,"requestId":"11111111111111111111111111111111","action":{"kind":"attach-binding","siteId":"11111111111111111111111111111111","workerId":"11111111111111111111111111111111","terminalId":"11111111111111111111111111111111"}}"#
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
            br#"{"kind":"terminal.viewer.result","protocolVersion":2,"requestId":"11111111111111111111111111111111","outcome":{"kind":"unavailable","reason":"workbench-unavailable"}}"#
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
        site_id: SiteId::parse(SHARED_IDENTITY)?,
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
            r#"{{"kind":"terminal.viewer","protocolVersion":2,"requestId":"{SHARED_IDENTITY}","action":{payload}}}"#
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
            r#"{{"kind":"terminal.viewer","protocolVersion":2,"requestId":"{SHARED_IDENTITY}","action":{action}}}"#
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
            br#"{"kind":"terminal.viewer.result","protocolVersion":2,"requestId":"11111111111111111111111111111111","outcome":{"kind":"rejected","reason":"host-rejected"}}"#
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
