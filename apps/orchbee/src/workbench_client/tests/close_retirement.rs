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
    let origin = client.origin();
    let clock = client.operation_clock();
    let mailbox = Arc::new(mailbox::CommandMailbox::new(
        NATIVE_TEST_IDENTITY_SEED,
        geometry,
    ));
    let worker_mailbox = Arc::clone(&mailbox);
    let task = thread::spawn(move || -> connection::ClientConnectionExit {
        let _lost_return = connection::run_native(client, geometry, worker_mailbox, None);
        panic!("injected worker panic after transport returned");
    });
    let mut workbench_client = WorkbenchClient {
        position_writer: None,
        position_startup: None,
        configuration: None,
        notifications: None,
        window_title: None,
        client_control: None,
        backend: ClientBackend::Native(Box::new(ui::WorkbenchClientUi::new(mailbox, clock))),
        task: Some(ClientTask::Native(task)),
        shutdown,
        retirement_redraw_pending: false,
        origin: Some(origin),
        recovery_shutdown: None,
    };
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    workbench_client.apply_client_action(ClientAction::RequestCloseTab);
    let view = workbench_client.view();
    let token = view.close_confirmation().unwrap().token();
    workbench_client.after_close_draw(DrawReceipt::new(
        Some(ModalPresentation::Presented(token)),
        None,
    ));
    workbench_client.apply_client_action(ClientAction::ConfirmClose);
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    assert!(saw_close.load(Ordering::SeqCst));
    let view = workbench_client.view();
    let status = view
        .close_status()
        .expect("missing worker return cannot erase the possibly submitted Close");
    assert_eq!(status.status(), CloseStatusKind::AwaitingStatus);
    assert!(status.unavailable());
    assert!(!status.resumable());
    let submitted = wire_id_receiver.recv_timeout(Duration::from_secs(1))?;
    let ClientBackend::Native(native) = &workbench_client.backend;
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
