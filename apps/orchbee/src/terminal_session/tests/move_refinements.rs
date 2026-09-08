use super::*;
use crate::local_client::test_server::connect_with_ui_sample_expiry_clock;
use orchbee_workbench_protocol::WorkbenchTabMoveDirection;

#[test]
fn native_ui_move_expiry_returns_while_the_contended_mailbox_remains_locked() -> TestResult {
    let geometry = fixture_geometry()?;
    let (poll_sent, poll_seen) = mpsc::sync_channel(0);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;
        let _poll = read_client_message(&mut stream).map_err(io::Error::other)?;
        poll_sent.send(()).map_err(io::Error::other)?;
        let mut trailing = vec![];
        stream.read_to_end(&mut trailing)?;
        assert!(trailing.is_empty());
        Ok(())
    })?;
    let phase = Arc::new(AtomicU8::new(0));
    let mut client = connect_with_ui_sample_expiry_clock(
        server.endpoint(),
        "test-cohort",
        Arc::clone(&phase),
        2,
    )?;
    client.set_submission_deadline(Some(
        client.operation_clock().now() + Duration::from_secs(5),
    ));
    let (mut session, mailbox) = native_session_with_identity(client, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    poll_seen.recv_timeout(Duration::from_secs(1))?;
    assert_eq!(
        TerminalInputPort::apply_client_action(&mut session, ClientAction::SelectTab(9)),
        Some(SessionActionDisposition::KeepNavigator)
    );
    let before = render_native_session(&session, &ClientView::new(120))?;
    let (held_sent, held_seen) = mpsc::sync_channel(0);
    let (returned_sent, returned_seen) = mpsc::sync_channel(1);
    let holder_mailbox = Arc::clone(&mailbox);
    let holder = thread::spawn(move || {
        let _held = holder_mailbox.hold_lock_for_test();
        held_sent.send(()).unwrap();
        // The bounded test guard releases even on regression, so the failing UI call can finish.
        returned_seen.recv_timeout(Duration::from_secs(1)).is_ok()
    });
    held_seen.recv_timeout(Duration::from_secs(1))?;
    phase.store(1, Ordering::SeqCst);
    assert_eq!(
        TerminalInputPort::apply_client_action(&mut session, ClientAction::MoveNextTab),
        Some(SessionActionDisposition::KeepNavigator)
    );
    let _sent = returned_sent.send(());
    assert!(
        holder.join().unwrap(),
        "Move refusal reacquired the contended mailbox after expiry"
    );
    assert_eq!(phase.load(Ordering::SeqCst), 4);
    assert_eq!(session.status(), TerminalStatus::Controlling);
    assert_eq!(
        render_native_session(&session, &ClientView::new(120))?,
        before
    );
    assert!(mailbox.is_closed().is_none());
    assert_eq!(
        mailbox.allocate_workbench_request_id().unwrap().as_str(),
        native_identity(0x109)
    );
    drop(session);
    server.finish()?;
    Ok(())
}

pub(super) fn two_tabs() -> Snapshot {
    let WorkbenchSnapshotOutcome::Available { mut snapshot } = available_snapshot(Some(binding()))
    else {
        unreachable!()
    };
    let mut second = snapshot.terminal_contexts[0].tabs[0].clone();
    second.tab_id = WorkbenchTabId::parse(CREATED_TAB_ID).unwrap();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &mut second.root
    else {
        unreachable!()
    };
    *panel_id = WorkbenchPanelId::parse(CREATED_PANEL_ID).unwrap();
    primary_presentation.presentation_id =
        TerminalPresentationId::parse(CREATED_PRESENTATION_ID).unwrap();
    primary_presentation.binding.as_mut().unwrap().terminal_id =
        TerminalId::parse(CREATED_TAB_ID).unwrap();
    snapshot.terminal_contexts[0].tabs.push(second);
    snapshot
}

#[test]
fn outer_native_shutdown_join_finalization_retains_exact_pending_move_and_first_failure()
-> TestResult {
    let geometry = fixture_geometry()?;
    let (poll_sent, poll_seen) = mpsc::sync_channel(0);
    let (release_poll, poll_released) = mpsc::sync_channel(0);
    let (snapshot_sent, snapshot_seen) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame_with_snapshot(
            &mut stream,
            geometry,
            WorkbenchSnapshotOutcome::Available {
                snapshot: two_tabs(),
            },
        )?;
        accept_native_frame_feedback(&mut stream)?;
        let ClientMessage::TerminalViewer(poll) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected Poll"));
        };
        poll_sent.send(()).map_err(io::Error::other)?;
        poll_released.recv().map_err(io::Error::other)?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            )),
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::Command(command) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected Move"));
        };
        assert!(
            matches!(&command.action, WorkbenchAction::TabMove { tab_id, direction: WorkbenchTabMoveDirection::Last, .. } if tab_id.as_str() == ID)
        );
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id.clone(),
                outcome: WorkbenchCommandOutcome::TabMoved {
                    revision: WorkbenchRevision::new(2),
                    terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
                    tab_id: WorkbenchTabId::parse(ID).unwrap(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::SnapshotRead(_) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("expected postcommit Snapshot"));
        };
        // Reading Snapshot proves the worker already retained the typed Move commit. Do not answer:
        // the outer session shutdown must interrupt this read, join, and retain the worker's fact.
        snapshot_sent.send(command).map_err(io::Error::other)?;
        let mut trailing = vec![];
        stream.read_to_end(&mut trailing)?;
        assert!(trailing.is_empty());
        Ok(())
    })?;
    let (mut session, mailbox) = native_session_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    poll_seen.recv_timeout(Duration::from_secs(1))?;
    assert_eq!(
        TerminalInputPort::apply_client_action(&mut session, ClientAction::MovePreviousTab),
        Some(SessionActionDisposition::DismissNavigator)
    );
    release_poll.send(())?;
    let command = std::cell::RefCell::new(None);
    assert!(spin_with_draw(&mut session, |_| {
        *command.borrow_mut() = snapshot_seen.try_recv().ok();
        command.borrow().is_some()
    }));
    let command = command.into_inner().unwrap();
    assert!(session.task.is_some());
    assert_eq!(
        mailbox.close(CarrierFailure::Deadline).failure(),
        CarrierFailure::Deadline
    );
    assert_eq!(
        mailbox.close(CarrierFailure::Protocol).failure(),
        CarrierFailure::Deadline
    );
    assert!(session.tick());
    assert!(
        session.task.is_none(),
        "the outer tick must join the worker before returning"
    );
    let SessionBackend::Native(native) = &session.backend else {
        unreachable!()
    };
    let (pending, failure) = native
        .move_retirement_for_test()
        .expect("outer join discarded PendingMoveCommit");
    assert_eq!(failure, CarrierFailure::Deadline);
    assert_eq!(pending.operation().request_id(), &command.request_id);
    assert_eq!(
        pending.operation().direction(),
        WorkbenchTabMoveDirection::Last
    );
    assert_eq!(pending.operation().target().generation(), 1);
    assert_eq!(pending.operation().target().tab_id().as_str(), ID);
    assert_eq!(
        pending.operation().baseline(),
        &workbench_projection::project_native_standalone_tabs(&two_tabs()).unwrap()
    );
    assert_eq!(
        pending.commit(),
        &crate::local_client::workbench::WorkbenchTabMoveCommit {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: WorkbenchTerminalContextId::parse(ID)?,
            tab_id: WorkbenchTabId::parse(ID)?
        }
    );
    assert!(!session.tick());
    drop(session);
    server.finish()?;
    Ok(())
}
