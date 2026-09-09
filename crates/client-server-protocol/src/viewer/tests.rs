use crate::{
    ClientMessage, FRAME_HEADER_BYTES, RequestId, ServerMessage, SiteId, TerminalId,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, TerminalViewerResult,
    ViewerId, ViewerUnavailableReason, WorkerId, decode_client_frame, decode_server_frame,
    encode_client_frame, encode_server_frame,
};

const SHARED_IDENTITY: &str = "11111111111111111111111111111111";

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
            br#"{"kind":"terminal.viewer","protocolVersion":1,"requestId":"11111111111111111111111111111111","action":{"kind":"attach-binding","siteId":"11111111111111111111111111111111","workerId":"11111111111111111111111111111111","terminalId":"11111111111111111111111111111111"}}"#
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
