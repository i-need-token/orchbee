use super::*;

#[test]
fn panic_after_close_handoff_preserves_uncertainty_after_exact_join() -> TestResult {
    let geometry = fixture_geometry()?;
    let saw_close = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&saw_close);
    let (wire_id_sender, wire_id_receiver) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;
        loop {
            let mut magic = [0; 4];
            stream.read_exact(&mut magic)?;
            let mut reader = io::Cursor::new(magic).chain(&mut stream);
            if magic == *b"OBWP" {
                let request = read_workbench_request(&mut reader).map_err(io::Error::other)?;
                if let WorkbenchRequest::PublicationRead(intent) = request {
                    // Publication probes may interleave with ordinary polls on the ready session.
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                            protocol_version: ProtocolVersion,
                            request_id: intent.request_id,
                            outcome: WorkbenchPublicationOutcome::Current {
                                revision: intent.known_revision,
                            },
                        }),
                    )
                    .map_err(io::Error::other)?;
                    continue;
                }
                let WorkbenchRequest::Command(command) = request else {
                    return Err(io::Error::other("expected Close"));
                };
                assert!(matches!(command.action, WorkbenchAction::TabClose { .. }));
                observed.store(true, Ordering::SeqCst);
                wire_id_sender
                    .send(command.request_id)
                    .map_err(io::Error::other)?;
                return Ok(());
            }
            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut reader).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected ordinary Poll"));
            };
            assert_eq!(
                poll.action(),
                &TerminalViewerAction::Poll {
                    attachment_id: binding_viewer_id()?,
                }
            );
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    poll.request_id().clone(),
                    TerminalViewerOutcome::Empty {},
                )),
            )
            .map_err(io::Error::other)?;
        }
    })?;
    let client = native_client(&server)?;
    let shutdown = client.shutdown_handle()?;
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    let origin = client.origin();
    let clock = client.operation_clock();
    let mailbox = Arc::new(mailbox::CommandMailbox::new(
        NATIVE_TEST_IDENTITY_SEED,
        geometry,
    ));
    let worker_mailbox = Arc::clone(&mailbox);
    let task = thread::spawn(move || -> connection::NativeWorkerExit {
        let _lost_return = connection::run_native(client, geometry, worker_mailbox, None);
        panic!("injected worker panic after transport returned");
    });
    let mut session = TerminalSession {
        #[cfg(windows)]
        position_writer: None,
        #[cfg(windows)]
        position_startup: None,
        #[cfg(windows)]
        configuration: None,
        #[cfg(windows)]
        notifications: None,
        #[cfg(windows)]
        window_title: None,
        #[cfg(windows)]
        client_control: None,
        backend: SessionBackend::Native(Box::new(ui::NativeUi::new(mailbox, clock))),
        task: Some(SessionTask::Native(task)),
        shutdown,
        retirement_redraw_pending: false,
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        origin: Some(origin),
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        recovery_shutdown: None,
    };
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    session.apply_client_action(ClientAction::RequestCloseTab);
    let TerminalSessionView::Native(view) = session.view();
    let token = view.close_confirmation().unwrap().token();
    session.after_close_draw(NativeDrawReceipt::new(
        Some(ModalPresentation::Presented(token)),
        None,
    ));
    session.apply_client_action(ClientAction::ConfirmClose);
    assert!(spin_with_draw(&mut session, |session| session
        .task
        .is_none()));
    assert!(saw_close.load(Ordering::SeqCst));
    let TerminalSessionView::Native(view) = session.view();
    let status = view
        .close_status()
        .expect("missing worker return cannot erase the possibly submitted Close");
    assert_eq!(status.status(), NativeCloseStatusKind::AwaitingStatus);
    assert!(status.unavailable());
    assert!(!status.resumable());
    let submitted = wire_id_receiver.recv_timeout(Duration::from_secs(1))?;
    let SessionBackend::Native(native) = &session.backend;
    let (retained, canonical) = native
        .uncertain_close_identity()
        .expect("retain original uncertain exchange identity");
    assert_eq!(retained.request_id(), &submitted);
    assert!(
        canonical.is_none(),
        "submitted identity is not proof of canonical operation identity"
    );
    server.finish()?;
    Ok(())
}
