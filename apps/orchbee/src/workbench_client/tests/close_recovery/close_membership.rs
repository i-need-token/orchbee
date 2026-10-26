use super::*;

fn empty_snapshot() -> WorkbenchSnapshotOutcome {
    let WorkbenchSnapshotOutcome::Available { mut snapshot } = available_snapshot(Some(binding()))
    else {
        unreachable!()
    };
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.terminal_contexts[0].tabs.clear();
    WorkbenchSnapshotOutcome::Available { snapshot }
}

fn serve_closed(
    stream: &mut UnixStream,
    operation: orchbee_workbench_protocol::WorkbenchRequestId,
) -> io::Result<String> {
    let WorkbenchRequest::TabCloseRead(request) =
        read_workbench_request_absorbing_probes(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected exact Close Read"));
    };
    let id = request.request_id.as_str().to_owned();
    write_workbench_response(
        stream,
        &WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchTabCloseReadOutcome::Available {
                close_request_id: operation,
                terminal_context_id: request.terminal_context_id,
                tab_id: request.tab_id,
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                status: WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(2),
                },
            },
        }),
    )
    .map_err(io::Error::other)?;
    Ok(id)
}

fn is_empty(workbench_client: &WorkbenchClient) -> bool {
    {
        let view = workbench_client.view();
        view.phase() == DisplayPhase::Empty
    }
}

fn require_manual_gap(stream: &mut UnixStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(180)))?;
    let mut byte = [0; 1];
    let error = stream.read(&mut byte).expect_err(
        "an admitted failed Snapshot/status-only Closed cannot automatically send another request",
    );
    assert!(matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ));
    stream.set_read_timeout(Some(Duration::from_secs(1)))
}

fn tick_manual_gap(workbench_client: &mut WorkbenchClient) {
    for _ in 0..24 {
        workbench_client.tick();
        workbench_client.after_draw();
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn status_only_closed_requires_a_new_drawn_manual_membership_transition_before_empty() -> TestResult
{
    let geometry = fixture_geometry()?;
    let (gap_tx, gap_rx) = mpsc::sync_channel(1);
    let mut operation = None;
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            operation = Some(receive_close_after_frame(&mut stream)?.request_id);
            return Ok(());
        }
        admit(&mut stream)?;
        let first_read = serve_closed(&mut stream, operation.clone().unwrap())?;
        require_manual_gap(&mut stream)?;
        gap_tx.send(()).map_err(io::Error::other)?;
        let snapshot = respond_with_read(&mut stream, empty_snapshot())?;
        assert_ne!(snapshot, first_read);
        let mut unexpected = [0; 1];
        assert_eq!(
            stream.read(&mut unexpected)?,
            0,
            "Empty recovery must not bootstrap or revive the old Viewer"
        );
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    workbench_client.apply_client_action(ClientAction::RequestCloseTab);
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action(ClientAction::ConfirmClose);
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        recovered_status(workbench_client, CloseStatusKind::Closed)
    }));
    tick_manual_gap(&mut workbench_client);
    gap_rx.recv_timeout(Duration::from_secs(1))?;
    assert!(!is_empty(&workbench_client));
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(spin_with_draw(&mut workbench_client, is_empty));
    drop(workbench_client);
    server.finish()?;
    Ok(())
}

#[test]
fn failed_close_snapshot_retains_closed_and_manual_retry_uses_a_fresh_snapshot_id() -> TestResult {
    let geometry = fixture_geometry()?;
    let (gap_tx, gap_rx) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        let mut stream = UnixStream::new(stream);
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;
        let command = receive_close_after_frame(&mut stream)?;
        let operation = command.request_id.clone();
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id,
                outcome: WorkbenchCommandOutcome::TabCloseAccepted {
                    close_request_id: operation.clone(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
                    tab_id: WorkbenchTabId::parse(ID).unwrap(),
                    accepted_revision: command.expected_revision,
                },
            }),
        )
        .map_err(io::Error::other)?;
        let closed_read = serve_closed(&mut stream, operation)?;
        let failed = respond_with_read(&mut stream, WorkbenchSnapshotOutcome::Denied {})?;
        require_manual_gap(&mut stream)?;
        gap_tx.send(()).map_err(io::Error::other)?;
        let retry = respond_with_read(&mut stream, empty_snapshot())?;
        assert_ne!(retry, failed);
        assert_ne!(retry, closed_read);
        let mut unexpected = [0; 1];
        assert_eq!(stream.read(&mut unexpected)?, 0);
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    workbench_client.apply_client_action(ClientAction::RequestCloseTab);
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action(ClientAction::ConfirmClose);
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        let view = workbench_client.view();
        view.close_status().is_some_and(|status| {
            status.status() == CloseStatusKind::Closed && status.unavailable()
        })
    }));
    tick_manual_gap(&mut workbench_client);
    gap_rx.recv_timeout(Duration::from_secs(1))?;
    assert!(!is_empty(&workbench_client));
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(spin_with_draw(&mut workbench_client, is_empty));
    drop(workbench_client);
    server.finish()?;
    Ok(())
}

#[test]
fn close_closed_snapshot_uses_exact_gone_and_installs_empty_without_another_terminal_call()
-> TestResult {
    let geometry = fixture_geometry()?;
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        let mut stream = UnixStream::new(stream);
        serve_native_controlling_frame(&mut stream, geometry)?;
        accept_native_frame_feedback(&mut stream)?;
        let command = receive_close_after_frame(&mut stream)?;
        let operation = command.request_id.clone();
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id,
                outcome: WorkbenchCommandOutcome::TabCloseAccepted {
                    close_request_id: operation.clone(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
                    tab_id: WorkbenchTabId::parse(ID).unwrap(),
                    accepted_revision: command.expected_revision,
                },
            }),
        )
        .map_err(io::Error::other)?;
        let closed_read = serve_closed(&mut stream, operation)?;
        let snapshot = respond_with_read(&mut stream, empty_snapshot())?;
        assert_ne!(
            closed_read, snapshot,
            "membership Snapshot owns a fresh exact request"
        );
        done_tx.send(()).map_err(io::Error::other)?;
        let mut unexpected = [0; 1];
        assert_eq!(
            stream.read(&mut unexpected)?,
            0,
            "known exact Closed requires no stale Detach, Poll, Open or Create"
        );
        Ok(())
    })?;
    let (mut workbench_client, _mailbox) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    workbench_client.apply_client_action(ClientAction::RequestCloseTab);
    draw_recovery(&mut workbench_client)?;
    workbench_client.apply_client_action(ClientAction::ConfirmClose);
    assert!(
        spin_with_draw(&mut workbench_client, is_empty),
        "Closed must complete canonical removal and Empty"
    );
    done_rx.recv_timeout(Duration::from_secs(1))?;

    for _ in 0..10 {
        workbench_client.tick();
        workbench_client.after_draw();
    }
    assert!(is_empty(&workbench_client));
    drop(workbench_client);
    server.finish()?;
    Ok(())
}
