use super::*;

#[test]
fn poll_unavailable_retirement_during_geometry_requests_one_redraw() -> TestResult {
    for (refreshing, draw_before_tick) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let geometry = fixture_geometry()?;
        let (poll_sender, poll_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let server = FakeServer::start(move |stream| {
            let mut stream = UnixStream::new(stream);
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected the ready viewer's Poll"));
            };
            assert_eq!(
                poll.action(),
                &TerminalViewerAction::Poll {
                    attachment_id: binding_viewer_id()?,
                }
            );
            poll_sender.send(()).map_err(io::Error::other)?;
            release_receiver
                .recv_timeout(Duration::from_secs(2))
                .map_err(io::Error::other)?;
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    poll.request_id().clone(),
                    TerminalViewerOutcome::Unavailable {
                        reason: orchbee_client_server_protocol::ViewerUnavailableReason::WorkbenchUnavailable,
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
        let (mut workbench_client, mailbox) =
            workbench_client_with_identity(native_client(&server)?, geometry)?;
        assert!(spin_with_draw(
            &mut workbench_client,
            |workbench_client| workbench_client.status() == TerminalStatus::Controlling
        ));
        poll_receiver.recv_timeout(Duration::from_secs(2))?;
        assert!(!workbench_client.tick(), "unchanged Ready needs no redraw");
        let mut view = ClientView::new(120);
        if refreshing {
            dispatch_input(
                &mut InputDispatcher::default(),
                &mut view,
                b"\x02w",
                &mut workbench_client,
                Instant::now(),
                |_, _| Ok::<(), io::Error>(()),
            )?;
        }
        let before = render_workbench_client(&workbench_client, &view)?;
        if refreshing {
            assert!(before.contains("Refreshing"), "{before}");
        }
        workbench_client.after_draw();
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
        workbench_client.observe_geometry(geometry);
        assert!(
            workbench_client.task.is_none(),
            "geometry observation joins the failed worker"
        );
        assert_eq!(workbench_client.status(), TerminalStatus::Unavailable);
        if draw_before_tick {
            render_workbench_client(&workbench_client, &view)?;
            workbench_client.after_draw();
        }
        assert_eq!(
            workbench_client.tick(),
            !draw_before_tick,
            "retirement before tick needs one redraw unless already drawn"
        );
        let after = render_workbench_client(&workbench_client, &view)?;
        assert_ne!(after, before);
        assert!(!after.contains("Refreshing"));
        assert!(after.contains("initial_workspace"), "{after}");
        assert!(after.contains("Terminal disconnected"), "{after}");
        if refreshing {
            assert!(after.contains("› * 1 initial_workspace"), "{after}");
            assert!(
                after.contains("Navigator · Esc return · Enter open"),
                "{after}"
            );
            assert!(
                after.contains(&crate::terminal_events::keys::tests::host_label(
                    "Esc / Ctrl+b w return  │  Enter open"
                )),
                "{after}"
            );
        } else {
            assert!(
                after.contains(&crate::terminal_events::keys::tests::host_label(
                    "Ctrl+b q / Ctrl+b d exit"
                )),
                "{after}"
            );
        }
        workbench_client.after_draw();
        assert!(
            !workbench_client.tick(),
            "the retirement redraw is consumed once"
        );
        drop(workbench_client);
        server.finish()?;
    }
    Ok(())
}
