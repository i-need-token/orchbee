use super::*;

#[test]
fn poll_unavailable_retirement_during_geometry_requests_one_redraw() -> TestResult {
    for (refreshing, draw_before_tick) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let geometry = fixture_geometry()?;
        let (poll_sender, poll_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let server = FakeServer::start(move |mut stream| {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected the ready viewer's Poll"));
            };
            assert_eq!(poll.action(), &TerminalViewerAction::Poll {});
            poll_sender.send(()).map_err(io::Error::other)?;
            release_receiver
                .recv_timeout(Duration::from_secs(2))
                .map_err(io::Error::other)?;
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    poll.request_id().clone(),
                    TerminalViewerOutcome::Unavailable {
                        reason: orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable,
                    },
                )),
            )
            .map_err(io::Error::other)?;
            let mut unexpected = [0; 1];
            assert_eq!(
                stream.read(&mut unexpected)?,
                0,
                "retirement sends no Refresh or replacement Attach"
            );
            Ok(())
        })?;
        let (mut session, mailbox) =
            native_session_with_identity(native_client(&server)?, geometry)?;
        assert!(spin_with_draw(&mut session, |session| session.status()
            == TerminalStatus::Controlling));
        poll_receiver.recv_timeout(Duration::from_secs(2))?;
        assert!(!session.tick(), "unchanged Ready needs no redraw");
        let mut view = ClientView::new(120);
        if refreshing {
            dispatch_input(&mut view, b"\x02w", &mut session, |_, _| {
                Ok::<(), io::Error>(())
            })?;
        }
        let before = render_native_session(&session, &view)?;
        if refreshing {
            assert!(before.contains("Refreshing"), "{before}");
        }
        session.after_draw();
        release_sender.send(())?;
        let cutoff = Instant::now() + Duration::from_secs(2);
        while mailbox.is_closed().is_none() {
            assert!(
                Instant::now() < cutoff,
                "Poll failure must close the worker mailbox"
            );
            thread::yield_now();
        }

        // The real shell observes unchanged geometry before polling the session for redraw.
        session.observe_geometry(geometry);
        assert!(
            session.task.is_none(),
            "geometry observation joins the failed worker"
        );
        assert_eq!(session.status(), TerminalStatus::Unavailable);
        if draw_before_tick {
            render_native_session(&session, &view)?;
            session.after_draw();
        }
        assert_eq!(
            session.tick(),
            !draw_before_tick,
            "retirement before tick needs one redraw unless already drawn"
        );
        let after = render_native_session(&session, &view)?;
        assert_ne!(after, before);
        assert!(!after.contains("Refreshing"));
        assert!(
            after.contains("Terminal disconnected · Ctrl+B then q to exit"),
            "{after}"
        );
        session.after_draw();
        assert!(!session.tick(), "the retirement redraw is consumed once");
        drop(session);
        server.finish()?;
    }
    Ok(())
}
