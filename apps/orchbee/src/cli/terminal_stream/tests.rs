#[cfg(windows)]
mod windows;
use super::*;
use crate::local_client::test_support::server::FakeServer;
use base64::Engine;
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, TerminalViewerResult, read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::LocalStream;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding, TerminalTextCell,
    TerminalTextRow, decode_terminal_text_frame, encode_transport_envelope,
    inspect_terminal_frame_route,
};
use std::{
    io,
    num::NonZeroU64,
    sync::atomic::{AtomicBool, Ordering},
};
#[cfg(windows)]
pub(super) use windows::line;

pub(super) const SNAPSHOT: &[u8] =
    include_bytes!("../../../../../schemas/terminal/v1/fixtures/ghostty-surface.bin");
pub(super) const COHORT: &str = "terminal-stream-test";
type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn handshake(stream: &mut LocalStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::MIN,
        },
    )
    .map_err(io::Error::other)
}
pub(super) fn request(stream: &mut LocalStream) -> io::Result<TerminalViewerRequest> {
    let ClientMessage::TerminalViewer(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("observe must send Viewer requests only"));
    };
    assert_eq!(
        request
            .action()
            .attachment_id()
            .expect("retained Viewer action")
            .as_str(),
        inspect_terminal_frame_route(SNAPSHOT)
            .map_err(io::Error::other)?
            .attachment_id
            .to_text()
    );
    Ok(request)
}
pub(super) fn reply(
    stream: &mut LocalStream,
    request: TerminalViewerRequest,
    outcome: Outcome,
) -> io::Result<()> {
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .map_err(io::Error::other)
}
pub(super) fn record(sequence: u64, revision: u64, invalid_cells: bool) -> io::Result<Vec<u8>> {
    let mut bytes = SNAPSHOT.to_vec();
    bytes[48..56].copy_from_slice(&sequence.to_le_bytes());
    bytes[64..72].copy_from_slice(&revision.to_le_bytes());
    // Preserve routing but invalidate the text payload: the projection must request a snapshot.
    if invalid_cells {
        bytes.truncate(160);
        bytes[8..12].copy_from_slice(&160_u32.to_le_bytes());
    }
    let route = inspect_terminal_frame_route(&bytes).map_err(io::Error::other)?;
    encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: 41,
        payload_encoding: PayloadEncoding::TerminalFrame,
        host_epoch: Some(route.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: &bytes,
    })
    .map_err(io::Error::other)
}
#[test]
fn observe_eof_is_failure_and_deadline_or_cancel_prevents_polling() -> TestResult {
    let route = inspect_terminal_frame_route(SNAPSHOT)?;
    let viewer = ViewerId::parse(&route.attachment_id.to_text())?;
    let server = FakeServer::start(|mut stream| {
        handshake(&mut stream)?;
        assert!(matches!(
            request(&mut stream)?.action(),
            Action::Poll { .. }
        ));
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let cancelled = AtomicBool::new(false);
    let check = || cancelled.load(Ordering::Acquire);
    let expired = Gate {
        cancelled: &check,
        wake: None,
        total: Some(Instant::now()),
    };
    assert_eq!(
        exchange(
            &mut client,
            Action::Poll {
                attachment_id: viewer.clone()
            },
            &expired
        )
        .unwrap_err()
        .kind,
        FailureKind::Timeout
    );
    cancelled.store(true, Ordering::Release);
    assert_eq!(
        exchange(
            &mut client,
            Action::Poll {
                attachment_id: viewer.clone()
            },
            &expired
        )
        .unwrap_err()
        .kind,
        FailureKind::Cancelled
    );
    cancelled.store(false, Ordering::Release);
    let gate = Gate {
        cancelled: &check,
        wake: None,
        total: None,
    };
    assert_eq!(
        exchange(
            &mut client,
            Action::Poll {
                attachment_id: viewer
            },
            &gate
        )
        .unwrap_err()
        .kind,
        FailureKind::TransportLoss
    );
    server.finish()?;
    Ok(())
}

#[test]
fn ansi_viewport_clips_wide_cells_sanitizes_controls_and_preserves_styles() -> TestResult {
    let mut source = decode_terminal_text_frame(SNAPSHOT)?;
    source.cursor.visible = true;
    source.cursor.column = 7;
    source.cursor.row = 0;
    let style = source.rows[0].cells[0].style;
    source.rows = vec![TerminalTextRow {
        prompt_class: 0,
        index: 0,
        flags: 0,
        cells: vec![
            TerminalTextCell {
                text: "\x1b]52;c;payload\x07".into(),
                style,
                width: 0,
            },
            TerminalTextCell {
                text: "中".into(),
                style,
                width: 1,
            },
            TerminalTextCell {
                text: "".into(),
                style,
                width: 2,
            },
        ],
    }];
    let narrow = frame::record(&source, 2, 1).map_err(|_| "render")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(narrow["bytes"].as_str().unwrap())
        .expect("base64 frame");
    let text = String::from_utf8(bytes)?;
    assert!(!text.contains("中"));
    assert!(!text.contains("payload"));
    assert!(!text.contains("\x1b]"));
    assert!(!text.contains("\x1b[?25h"));
    let wide = frame::record(&source, 8, 3).map_err(|_| "render")?;
    let text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(wide["bytes"].as_str().unwrap())
            .expect("base64 frame"),
    )?;
    assert!(text.contains("中"));
    assert!(text.ends_with("\x1b[1;8H\x1b[?25h"));
    assert!(text.contains("\x1b[2J"));
    assert_eq!(wide["width"], 8);
    assert_eq!(wide["height"], 3);
    Ok(())
}
