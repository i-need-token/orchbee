use super::*;
mod close_membership;
use orchbee_workbench_protocol::{
    CloseOperationOutcome, CloseOperationResult, CloseOperationTarget, WorkbenchCommand,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseStatus,
};

fn receive_close_after_frame(stream: &mut UnixStream) -> io::Result<WorkbenchCommand> {
    loop {
        let mut magic = [0; 4];
        stream.read_exact(&mut magic)?;
        let mut reader = io::Cursor::new(magic).chain(&mut *stream);
        if magic == *b"OBWP" {
            let request = read_workbench_request(&mut reader).map_err(io::Error::other)?;
            if let WorkbenchRequest::PublicationRead(intent) = request {
                // Publication probes may interleave with ordinary polls on the ready session.
                write_workbench_response(
                    stream,
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
                return Err(io::Error::other("expected original Close"));
            };
            assert!(matches!(command.action, WorkbenchAction::TabClose { .. }));
            return Ok(command);
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
            stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            )),
        )
        .map_err(io::Error::other)?;
    }
}

fn draw_recovery(workbench_client: &mut WorkbenchClient) -> TestResult {
    let view = ClientView::new(120);
    let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
    let mut receipt = DrawReceipt::default();
    terminal.draw(|frame| {
        receipt = render_terminal_shell(
            frame,
            &view,
            Locale::EnUs,
            Some(workbench_client.view()),
            &crate::tui_theme::Theme::terminal_native(),
            &mut crate::tui_hit_map::HitMapBuilder::new(),
        );
    })?;
    workbench_client.after_close_draw(receipt);
    Ok(())
}

fn recovered_status(workbench_client: &WorkbenchClient, expected: CloseStatusKind) -> bool {
    {
        let view = workbench_client.view();
        view.close_status()
            .is_some_and(|status| status.status() == expected && !status.unavailable())
    }
}

#[test]
fn recovery_rejects_foreign_target_without_adopting_its_operation() -> TestResult {
    let geometry = fixture_geometry()?;
    let (wire_sender, wire_receiver) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            wire_sender
                .send(receive_close_after_frame(&mut stream)?.request_id)
                .map_err(io::Error::other)?;
            return Ok(());
        }
        admit(&mut stream)?;
        let WorkbenchRequest::TabCloseRead(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other(
                "recovery must read only the retained target",
            ));
        };
        assert_eq!(request.session_id.as_str(), SESSION_ID);
        assert_eq!(request.tab_id.as_str(), ID);
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: WorkbenchTabCloseReadOutcome::Available {
                    close_request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(
                        CREATED_TAB_ID,
                    )
                    .unwrap(),
                    terminal_context_id: request.terminal_context_id,
                    tab_id: WorkbenchTabId::parse(CREATED_TAB_ID).unwrap(),
                    attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                    status: WorkbenchTabCloseStatus::CleanupBlocked {
                        reason: WorkbenchTabCloseBlockedReason::HostRefused,
                        resumable: true,
                    },
                },
            }),
        )
        .map_err(io::Error::other)?;
        let mut unexpected = [0; 1];
        assert_eq!(
            stream.read(&mut unexpected)?,
            0,
            "foreign target cannot grant a control stage"
        );
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    workbench_client.apply_client_action((ClientAction::RequestCloseTab).into());
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action((ClientAction::ConfirmClose).into());
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    let wire_id = wire_receiver.recv_timeout(Duration::from_secs(1))?;
    workbench_client.apply_client_action((ClientAction::OpenWorkbenchNavigator).into());
    assert!(workbench_client.task.is_some());
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    let ClientBackend::Native(native) = &workbench_client.backend;
    let (retained, canonical) = native.uncertain_close_identity().unwrap();
    assert_eq!(retained.request_id(), &wire_id);
    assert_eq!(retained.target().tab_id().as_str(), ID);
    assert!(canonical.is_none());
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action((ClientAction::RequestCloseTab).into());
    assert!({
        let view = workbench_client.view();
        view.close_confirmation().is_none()
    });
    server.finish()?;
    Ok(())
}

#[test]
fn early_not_closing_preserves_uncertain_identity_until_explicit_read_resolves_it() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut original = None;
    let (wire_sender, wire_receiver) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let command = receive_close_after_frame(&mut stream)?;
            wire_sender
                .send(command.request_id.clone())
                .map_err(io::Error::other)?;
            original = Some(command);
            return Ok(());
        }
        admit(&mut stream)?;
        for observation in 0..2 {
            let WorkbenchRequest::TabCloseRead(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected only independent status Reads"));
            };
            if observation == 0 {
                wire_sender
                    .send(request.request_id.clone())
                    .map_err(io::Error::other)?;
            }
            let outcome = if observation == 0 {
                WorkbenchTabCloseReadOutcome::NotClosing {}
            } else {
                WorkbenchTabCloseReadOutcome::Available {
                    close_request_id: original.as_ref().unwrap().request_id.clone(),
                    terminal_context_id: request.terminal_context_id,
                    tab_id: request.tab_id,
                    attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                    status: WorkbenchTabCloseStatus::CleanupBlocked {
                        reason: WorkbenchTabCloseBlockedReason::HostRefused,
                        resumable: true,
                    },
                }
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome,
                }),
            )
            .map_err(io::Error::other)?;
        }
        let mut unexpected = [0; 1];
        assert_eq!(
            stream.read(&mut unexpected)?,
            0,
            "status read never replays the lost mutation"
        );
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    workbench_client.apply_client_action((ClientAction::RequestCloseTab).into());
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action((ClientAction::ConfirmClose).into());
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    let wire_id = wire_receiver.recv_timeout(Duration::from_secs(1))?;
    workbench_client.apply_client_action((ClientAction::OpenWorkbenchNavigator).into());
    workbench_client.tick();
    let read_id = wire_receiver.recv_timeout(Duration::from_secs(1))?;
    assert!(
        spin_with_draw(&mut workbench_client, |workbench_client| {
            matches!(&workbench_client.backend, ClientBackend::Native(native) if native.settled_close_identity().is_some_and(|key| key.request_id() == &read_id))
        }),
        "the exact NotClosing observation must semantically settle"
    );
    assert!(
        {
            let view = workbench_client.view();
            view.close_status()
                .is_some_and(|status| status.unavailable() && !status.resumable())
        },
        "early NotClosing remains inconclusive"
    );
    assert!(workbench_client.task.is_some());
    let ClientBackend::Native(native) = &workbench_client.backend;
    let (retained, canonical) = native
        .uncertain_close_identity()
        .expect("inconclusive Read cannot erase the submitted identity");
    assert_eq!(retained.request_id(), &wire_id);
    assert!(canonical.is_none());
    workbench_client.apply_client_action((ClientAction::OpenWorkbenchNavigator).into());
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        recovered_status(workbench_client, CloseStatusKind::CleanupBlocked)
    }));
    let ClientBackend::Native(native) = &workbench_client.backend;
    assert!(
        native.uncertain_close_identity().is_none(),
        "only validated canonical status resolves local uncertainty"
    );
    drop(workbench_client);
    server.finish()?;
    Ok(())
}

#[test]
fn explicit_recovery_reads_then_confirmed_resume_retires_and_recovers_closed() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut original: Option<WorkbenchCommand> = None;
    let mut previous_request = None;
    let server = FakeServer::start_connections(3, move |index, stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let command = receive_close_after_frame(&mut stream)?;
            previous_request = Some(command.request_id.clone());
            original = Some(command);
            return Ok(()); // The submitted Close has no reply.
        }
        admit(&mut stream)?;
        let command = original.as_ref().unwrap();
        let attempt_generation = WorkbenchTabCloseAttemptGeneration::new(index as u64).unwrap();
        let response = match (
            index,
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
        ) {
            // The submission's answer was lost, so the first Read finds the close by its Tab.
            (1, WorkbenchRequest::TabCloseRead(request)) => {
                assert_eq!(request.session_id, command.session_id);
                assert_eq!(request.terminal_context_id.as_str(), ID);
                assert_eq!(request.tab_id.as_str(), ID);
                assert_ne!(Some(&request.request_id), previous_request.as_ref());
                previous_request = Some(request.request_id.clone());
                WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: WorkbenchTabCloseReadOutcome::Available {
                        close_request_id: command.request_id.clone(),
                        terminal_context_id: request.terminal_context_id,
                        tab_id: request.tab_id,
                        attempt_generation,
                        status: WorkbenchTabCloseStatus::CleanupBlocked {
                            reason: WorkbenchTabCloseBlockedReason::HostRefused,
                            resumable: true,
                        },
                    },
                })
            }
            // That Read named the operation, so every later Read asks for it by identity.
            (2, WorkbenchRequest::CloseOperationRead(request)) => {
                assert_eq!(request.session_id, command.session_id);
                assert_eq!(request.operation_id, command.request_id);
                assert_ne!(Some(&request.request_id), previous_request.as_ref());
                previous_request = Some(request.request_id.clone());
                WorkbenchResponse::CloseOperation(CloseOperationResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    operation_id: request.operation_id,
                    outcome: CloseOperationOutcome::Available {
                        target: CloseOperationTarget::Tab {
                            terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
                            tab_id: WorkbenchTabId::parse(ID).unwrap(),
                        },
                        generation: attempt_generation,
                        status: WorkbenchTabCloseStatus::Closed {
                            revision: WorkbenchRevision::new(2),
                        },
                    },
                })
            }
            _ => return Err(io::Error::other("recovery must start with status Read")),
        };
        write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
        if index == 1 {
            let WorkbenchRequest::TabCloseResume(resume) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other(
                    "only newly confirmed Resume may follow Read",
                ));
            };
            assert_eq!(resume.close_request_id, command.request_id);
            assert_eq!(resume.expected_attempt_generation.value(), 1);
            assert_eq!(resume.session_id, command.session_id);
            assert_eq!(resume.terminal_context_id.as_str(), ID);
            assert_eq!(resume.tab_id.as_str(), ID);
            assert_ne!(Some(&resume.request_id), previous_request.as_ref());
            previous_request = Some(resume.request_id);
            return Ok(()); // The new Resume also loses its reply.
        }
        let mut unexpected = [0; 1];
        assert_eq!(
            stream.read(&mut unexpected)?,
            0,
            "Closed status cannot start attachment setup"
        );
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    workbench_client.apply_client_action((ClientAction::RequestCloseTab).into());
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action((ClientAction::ConfirmClose).into());
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));

    workbench_client.apply_client_action((ClientAction::OpenWorkbenchNavigator).into());
    assert!(
        workbench_client.task.is_some(),
        "explicit status recovery creates one fresh worker"
    );
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        recovered_status(workbench_client, CloseStatusKind::CleanupBlocked)
    }));
    workbench_client.apply_client_action((ClientAction::RequestCloseTab).into());
    assert!(
        {
            let view = workbench_client.view();
            view.close_confirmation().is_none()
        },
        "undrawn fresh status cannot authorize Resume"
    );
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action((ClientAction::RequestCloseTab).into());
    assert!({
        let view = workbench_client.view();
        view.close_confirmation()
            .is_some_and(|prompt| prompt.kind() == CloseConfirmationKind::Resume)
    });
    workbench_client.apply_client_action((ClientAction::ConfirmClose).into());
    assert!(
        {
            let view = workbench_client.view();
            view.close_confirmation().is_some()
        },
        "undrawn confirmation remains unsubmitted"
    );
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action((ClientAction::ConfirmClose).into());
    assert!(
        spin_with_draw(&mut workbench_client, |workbench_client| workbench_client
            .task
            .is_none()),
        "the second carrier also requires its own completed join"
    );

    workbench_client.apply_client_action((ClientAction::OpenWorkbenchNavigator).into());
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        recovered_status(workbench_client, CloseStatusKind::Closed)
    }));
    draw_recovery(&mut workbench_client)?;
    for _ in 0..4 {
        workbench_client.tick();
    }
    assert!({
        let view = workbench_client.view();
        view.projection().is_none()
    });
    drop(workbench_client);
    server.finish()?;
    Ok(())
}
