use super::*;
use crate::local_client::{LocalClient, tests::server::FakeServer};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::NotificationSound;
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
                        orchbee_workbench_protocol::WorkbenchRequest::Notification(_)
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
fn ui_handoff_is_once_and_an_intervening_modal_discards_the_message() {
    let (reader, wake) = LocalStream::pair().unwrap();
    let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
    let listener = Subscription {
        shared: shared.clone(),
        wake,
        task: None,
    };
    let message = Notification::normalize(
        "title",
        None,
        super::super::Position::TopRight,
        NotificationSound::None,
    )
    .unwrap();
    shared.0.lock().unwrap().message = Some(message.clone());
    assert_eq!(listener.poll(true), Some(message.clone()));
    assert_eq!(listener.poll(true), None);
    shared.0.lock().unwrap().message = Some(message);
    assert_eq!(listener.poll(false), None);
    assert_eq!(listener.poll(true), None);
    drop(reader);
}

#[test]
fn live_notification_pipe_hands_off_only_an_accepting_delivery_once() {
    use orchbee_workbench_protocol::{
        NotificationResult, WorkbenchRequest, WorkbenchResponse, read_workbench_request,
        write_workbench_response,
    };
    let message = Notification::normalize(
        "ready",
        Some("done"),
        super::super::Position::BottomRight,
        NotificationSound::None,
    )
    .unwrap();
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
            let WorkbenchRequest::Notification(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!()
            };
            let deliver = request.action == NotificationAction::Listen { accepting: true };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Notification(NotificationResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome: NotificationOutcome::Listening {
                        notification: deliver.then(|| message.clone()),
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
    assert_eq!(listener.poll(true), None);
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let delivered = loop {
        if let Some(message) = listener.poll(true) {
            break message;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(delivered, expected);
    assert_eq!(listener.poll(false), None);
    listener.close();
    drop(client);
    server.finish().unwrap();
}
