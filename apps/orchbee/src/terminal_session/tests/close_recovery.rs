use super::*;
mod close_membership;
use orchbee_workbench_protocol::{
    WorkbenchCommand, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseStatus,
};

fn receive_close_after_frame(stream: &mut UnixStream) -> io::Result<WorkbenchCommand> {
    loop {
        let mut magic = [0; 4];
        stream.read_exact(&mut magic)?;
        let mut reader = io::Cursor::new(magic).chain(&mut *stream);
        if magic == *b"OBWP" {
            let WorkbenchRequest::Command(command) =
                read_workbench_request(&mut reader).map_err(io::Error::other)?
            else {
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
        assert_eq!(poll.action(), &TerminalViewerAction::Poll {});
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

fn draw_recovery(session: &mut TerminalSession) -> TestResult {
    let view = ClientView::new(120);
    let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
    let mut receipt = NativeDrawReceipt::default();
    terminal.draw(|frame| {
        receipt = render_terminal_shell(frame, &view, Locale::EnUs, Some(session.view()));
    })?;
    session.after_close_draw(receipt);
    Ok(())
}

fn recovered_status(session: &TerminalSession, expected: NativeCloseStatusKind) -> bool {
    matches!(session.view(), TerminalSessionView::Native(view)
        if view.close_status().is_some_and(|status| status.status() == expected && !status.unavailable()))
}

#[test]
fn recovery_rejects_foreign_target_without_adopting_its_operation() -> TestResult {
    let geometry = fixture_geometry()?;
    let (wire_sender, wire_receiver) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
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
        assert_eq!(request.site_id.as_str(), SITE_ID);
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
    let (mut session, _) = native_session_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    session.apply_client_action(ClientAction::RequestCloseTab);
    draw_recovery(&mut session)?;
    session.apply_client_action(ClientAction::ConfirmClose);
    assert!(spin_with_draw(&mut session, |session| session
        .task
        .is_none()));
    let wire_id = wire_receiver.recv_timeout(Duration::from_secs(1))?;
    session.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(session.task.is_some());
    assert!(spin_with_draw(&mut session, |session| session
        .task
        .is_none()));
    let SessionBackend::Native(native) = &session.backend else {
        unreachable!()
    };
    let (retained, canonical) = native.uncertain_close_identity().unwrap();
    assert_eq!(retained.request_id(), &wire_id);
    assert_eq!(retained.target().tab_id().as_str(), ID);
    assert!(canonical.is_none());
    draw_recovery(&mut session)?;
    session.apply_client_action(ClientAction::RequestCloseTab);
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.close_confirmation().is_none())
    );
    server.finish()?;
    Ok(())
}

#[test]
fn early_not_closing_preserves_uncertain_identity_until_explicit_read_resolves_it() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut original = None;
    let (wire_sender, wire_receiver) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
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
    let (mut session, _) = native_session_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    session.apply_client_action(ClientAction::RequestCloseTab);
    draw_recovery(&mut session)?;
    session.apply_client_action(ClientAction::ConfirmClose);
    assert!(spin_with_draw(&mut session, |session| session
        .task
        .is_none()));
    let wire_id = wire_receiver.recv_timeout(Duration::from_secs(1))?;
    session.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    session.tick();
    let read_id = wire_receiver.recv_timeout(Duration::from_secs(1))?;
    assert!(
        spin_with_draw(&mut session, |session| {
            matches!(&session.backend, SessionBackend::Native(native) if native.settled_close_identity().is_some_and(|key| key.request_id() == &read_id))
        }),
        "the exact NotClosing observation must semantically settle"
    );
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.close_status().is_some_and(|status| status.unavailable() && !status.resumable())),
        "early NotClosing remains inconclusive"
    );
    assert!(session.task.is_some());
    let SessionBackend::Native(native) = &session.backend else {
        unreachable!()
    };
    let (retained, canonical) = native
        .uncertain_close_identity()
        .expect("inconclusive Read cannot erase the submitted identity");
    assert_eq!(retained.request_id(), &wire_id);
    assert!(canonical.is_none());
    session.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(spin_with_draw(&mut session, |session| recovered_status(
        session,
        NativeCloseStatusKind::CleanupBlocked
    )));
    let SessionBackend::Native(native) = &session.backend else {
        unreachable!()
    };
    assert!(
        native.uncertain_close_identity().is_none(),
        "only validated canonical status resolves local uncertainty"
    );
    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn explicit_recovery_reads_then_confirmed_resume_retires_and_recovers_closed() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut original: Option<WorkbenchCommand> = None;
    let mut previous_request = None;
    let server = FakeServer::start_connections(3, move |index, mut stream| {
        if index == 0 {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let command = receive_close_after_frame(&mut stream)?;
            previous_request = Some(command.request_id.clone());
            original = Some(command);
            return Ok(()); // The submitted Close has no reply.
        }
        admit(&mut stream)?;
        let WorkbenchRequest::TabCloseRead(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("recovery must start with status Read"));
        };
        let command = original.as_ref().unwrap();
        assert_eq!(request.site_id, command.site_id);
        assert_eq!(request.terminal_context_id.as_str(), ID);
        assert_eq!(request.tab_id.as_str(), ID);
        assert_ne!(Some(&request.request_id), previous_request.as_ref());
        previous_request = Some(request.request_id.clone());
        let status = if index == 1 {
            WorkbenchTabCloseStatus::CleanupBlocked {
                reason: WorkbenchTabCloseBlockedReason::HostRefused,
                resumable: true,
            }
        } else {
            WorkbenchTabCloseStatus::Closed {
                revision: WorkbenchRevision::new(2),
            }
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: WorkbenchTabCloseReadOutcome::Available {
                    close_request_id: command.request_id.clone(),
                    terminal_context_id: request.terminal_context_id,
                    tab_id: request.tab_id,
                    attempt_generation: WorkbenchTabCloseAttemptGeneration::new(index as u64)
                        .unwrap(),
                    status,
                },
            }),
        )
        .map_err(io::Error::other)?;
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
            assert_eq!(resume.site_id, command.site_id);
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
    let (mut session, _) = native_session_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    session.apply_client_action(ClientAction::RequestCloseTab);
    draw_recovery(&mut session)?;
    session.apply_client_action(ClientAction::ConfirmClose);
    assert!(spin_with_draw(&mut session, |session| session
        .task
        .is_none()));

    session.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(
        session.task.is_some(),
        "explicit status recovery creates one fresh worker"
    );
    assert!(spin_with_draw(&mut session, |session| recovered_status(
        session,
        NativeCloseStatusKind::CleanupBlocked
    )));
    session.apply_client_action(ClientAction::RequestCloseTab);
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.close_confirmation().is_none()),
        "undrawn fresh status cannot authorize Resume"
    );
    draw_recovery(&mut session)?;
    session.apply_client_action(ClientAction::RequestCloseTab);
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.close_confirmation().is_some_and(|prompt| prompt.kind() == NativeCloseConfirmationKind::Resume))
    );
    session.apply_client_action(ClientAction::ConfirmClose);
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.close_confirmation().is_some()),
        "undrawn confirmation remains unsubmitted"
    );
    draw_recovery(&mut session)?;
    session.apply_client_action(ClientAction::ConfirmClose);
    assert!(
        spin_with_draw(&mut session, |session| session.task.is_none()),
        "the second carrier also requires its own completed join"
    );

    session.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(spin_with_draw(&mut session, |session| recovered_status(
        session,
        NativeCloseStatusKind::Closed
    )));
    draw_recovery(&mut session)?;
    for _ in 0..4 {
        session.tick();
    }
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.projection().is_none())
    );
    drop(session);
    server.finish()?;
    Ok(())
}
