use super::*;

#[test]
fn wrong_id_transport_loss_and_other_unavailability_cannot_open_peer_recovery() -> TestResult {
    for kind in 0..3 {
        let geometry = fixture_geometry()?;
        let server = FakeServer::start(move |stream| {
            let mut stream = UnixStream::new(stream);
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("ordinary Poll"));
            };
            if kind == 2 {
                return Ok(());
            }
            let id = if kind == 0 {
                RequestId::parse(ID).unwrap()
            } else {
                poll.request_id().clone()
            };
            let reason = if kind == 0 {
                orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable
            } else {
                orchbee_client_server_protocol::ViewerUnavailableReason::WorkbenchUnavailable
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    id,
                    TerminalViewerOutcome::Unavailable { reason },
                )),
            )
            .map_err(io::Error::other)?;
            let mut byte = [0; 1];
            assert_eq!(stream.read(&mut byte)?, 0);
            Ok(())
        })?;
        let (mut workbench_client, _) =
            workbench_client_with_identity(native_client(&server)?, geometry)?;
        assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
            workbench_client.task.is_none()
        }));
        workbench_client.apply_client_action((ClientAction::OpenWorkbenchNavigator).into());
        assert!(
            workbench_client.task.is_none(),
            "failure kind {kind} does not establish an origin"
        );
        drop(workbench_client);
        server.finish()?;
    }
    Ok(())
}
