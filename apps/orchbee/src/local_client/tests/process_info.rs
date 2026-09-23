use super::*;
use orchbee_client_server_protocol::{
    TerminalViewerAction as Action, TerminalViewerOutcome as Outcome, TerminalViewerRequest,
    TerminalViewerResult, ViewerId,
};

#[test]
fn process_info_accepts_only_a_record_and_retires_on_empty_without_retry() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 9)?;
        for outcome in [Outcome::Record(vec![0xa5; 64]), Outcome::Empty {}] {
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected process read"));
            };
            assert!(matches!(request.action(), Action::ReadProcessInfo { .. }));
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
        }
        assert_eq!(
            stream.read(&mut [0])?,
            0,
            "invalid response retires without retry"
        );
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let request = |id| {
        Ok::<_, Box<dyn std::error::Error>>(TerminalViewerRequest::new(
            RequestId::parse(id)?,
            Action::ReadProcessInfo {
                attachment_id: ViewerId::parse(FIRST_REQUEST_ID)?,
            },
        ))
    };
    assert_eq!(
        client
            .submit_terminal_viewer(request(FIRST_REQUEST_ID)?)?
            .outcome(),
        &Outcome::Record(vec![0xa5; 64])
    );
    assert!(matches!(
        client.submit_terminal_viewer(request(SECOND_REQUEST_ID)?),
        Err(SubmissionError::OutcomeUnknown {
            reason: OutcomeUnknownReason::InvalidResult(_),
            ..
        })
    ));
    assert!(matches!(
        client.submit_terminal_viewer(request(FIRST_REQUEST_ID)?),
        Err(SubmissionError::SessionUnavailable)
    ));
    server.finish()?;
    Ok(())
}
