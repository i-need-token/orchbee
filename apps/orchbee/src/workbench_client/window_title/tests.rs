use super::*;
use crate::local_client::{LocalClient, tests::server::FakeServer};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use std::time::Instant;
use std::{
    io::{self, Read},
    num::NonZeroU64,
    sync::mpsc,
};
#[test]
fn listener_shutdown_cancels_blocked_handshake_or_poll_and_joins() {
    for handshake in [true, false] {
        let (tx, rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            if index == 0 || !handshake {
                write_message(
                    &mut stream,
                    &LocalClientMessage::ClientAccepted {
                        generation: NonZeroU64::new(index as u64 + 1).unwrap(),
                    },
                )
                .map_err(io::Error::other)?;
            }
            if index == 1 {
                if !handshake {
                    assert!(matches!(
                        orchbee_workbench_protocol::read_workbench_request(&mut stream)
                            .map_err(io::Error::other)?,
                        orchbee_workbench_protocol::WorkbenchRequest::WindowTitle(_)
                    ));
                }
                tx.send(()).unwrap();
                assert_eq!(stream.read(&mut [0])?, 0);
            }
            Ok(())
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let mut listener = Subscription::spawn(
            client.origin(),
            client.operation_clock(),
            SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        )
        .unwrap();
        drop(client);
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        listener.close();
        assert!(listener.task.is_none());
        listener.close();
        server.finish().unwrap();
    }
}

#[test]
fn live_window_title_pipe_hands_off_latest_snapshot_once() {
    use orchbee_workbench_protocol::{
        WindowTitleResult, WorkbenchRequest, WorkbenchResponse, read_workbench_request,
        write_workbench_response,
    };
    let message = WindowTitleSnapshot {
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        title: Some(orchbee_workbench_protocol::WindowTitle::normalize("ready").unwrap()),
    };
    let expected = message.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(index as u64 + 1).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        if index == 0 {
            return Ok(());
        }
        loop {
            let WorkbenchRequest::WindowTitle(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!()
            };
            let deliver = request.action
                == WindowTitleAction::Listen {
                    focused: true,
                    context: Default::default(),
                };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WindowTitle(WindowTitleResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome: WindowTitleOutcome::Listening {
                        snapshot: deliver.then(|| message.clone()),
                    },
                }),
            )
            .map_err(io::Error::other)?;
            if deliver {
                break;
            }
        }
        tx.send(()).unwrap();
        // Keep the connection alive until listener cancellation. Polls may already be queued.
        let mut data = [0_u8; 4096];
        while stream.read(&mut data)? != 0 {}
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let mut listener = Subscription::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    assert_eq!(listener.poll(true, Default::default()), None);
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let delivered = loop {
        if let Some(message) = listener.poll(true, Default::default()) {
            break message;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(delivered, expected);
    assert_eq!(listener.poll(false, Default::default()), None);
    listener.close();
    drop(client);
    server.finish().unwrap();
}

#[test]
fn window_title_latest_handoff_discards_demoted_state_and_republishes_on_promotion() {
    let (_reader, wake) = LocalStream::pair().unwrap();
    let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
    let listener = Subscription {
        shared: shared.clone(),
        wake,
        task: None,
    };
    let snapshot = WindowTitleSnapshot {
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        title: None,
    };
    shared.0.lock().unwrap().receive(Some(snapshot.clone()));
    assert_eq!(
        listener.poll(false, Default::default()),
        Some(snapshot.clone())
    );
    shared.0.lock().unwrap().receive(Some(snapshot.clone()));
    assert_eq!(listener.poll(false, Default::default()), None);
    shared.0.lock().unwrap().receive(None);
    shared.0.lock().unwrap().receive(Some(snapshot.clone()));
    assert_eq!(
        listener.poll(false, Default::default()),
        Some(snapshot.clone())
    );
    assert_eq!(listener.poll(true, Default::default()), None);
    shared.0.lock().unwrap().receive(Some(snapshot.clone()));
    assert_eq!(
        listener.poll(true, Default::default()),
        Some(snapshot.clone())
    );
    let newer = WindowTitleSnapshot {
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(2),
        title: Some(orchbee_workbench_protocol::WindowTitle::normalize("latest").unwrap()),
    };
    shared.0.lock().unwrap().receive(Some(newer.clone()));
    shared.0.lock().unwrap().receive(Some(snapshot));
    shared.0.lock().unwrap().receive(Some(newer.clone()));
    assert_eq!(listener.poll(true, Default::default()), Some(newer));
    shared.0.lock().unwrap().receive(None);
    assert_eq!(listener.poll(true, Default::default()), None);
}
