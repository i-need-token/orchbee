use super::*;
use crate::cli::terminal_stream::{
    STEP,
    tests::{COHORT, SNAPSHOT, handshake, request},
};
use crate::local_client::tests::server::FakeServer;
use orchbee_terminal_protocol::decode_terminal_text_frame;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[cfg(windows)]
mod windows;

#[test]
fn absent_history_does_not_pause_live_projection_or_acquire_history_demand() -> TestResult {
    for (alternate, retained) in [(true, 100), (false, 0)] {
        let server = FakeServer::start(|mut stream| {
            handshake(&mut stream)?;
            assert!(
                request(&mut stream).is_err(),
                "no history operation may be sent"
            );
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let mut frame = decode_terminal_text_frame(SNAPSHOT)?;
        frame.alternate_screen = alternate;
        frame.scrollback_rows = retained;
        let viewer = ViewerId::parse(&frame.route.attachment_id.to_text())?;
        let gate = Gate {
            cancelled: &|| false,
            wake: None,
            total: Some(Instant::now() + STEP),
        };
        let mut history = History::default();
        history
            .scroll(3, &frame, &mut client, &viewer, &gate)
            .unwrap();
        assert!(!history.active());
        drop(client);
        server.finish()?;
    }
    Ok(())
}
