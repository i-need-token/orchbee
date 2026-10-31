use super::*;
use crate::local_client::{
    LocalClient,
    test_support::{
        multiplex_peer::{admit, read_open, respond},
        server::FakeServer,
    },
};
use orchbee_workbench_protocol::{NotificationResult, NotificationSound, read_workbench_request};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn notification_subscription_uses_one_admission_and_cancels_its_stream() {
    let message = Notification::normalize(
        "ready",
        Some("done"),
        orchbee_workbench_protocol::NotificationPosition::BottomRight,
        NotificationSound::None,
    )
    .unwrap();
    let expected = message.clone();
    let (sent, ready) = mpsc::sync_channel(1);
    let (closed, cancelled) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let accepted = loop {
            let open = read_open(&mut peer)?;
            let WorkbenchRequest::Notification(request) =
                read_workbench_request(&mut Cursor::new(&open.request)).unwrap()
            else {
                panic!()
            };
            let accepting = request.action == NotificationAction::Listen { accepting: true };
            respond(
                &mut peer,
                &open,
                &WorkbenchResponse::Notification(NotificationResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome: NotificationOutcome::Listening {
                        notification: accepting.then(|| message.clone()),
                    },
                }),
                1,
            )?;
            if accepting {
                break open.id;
            }
        };
        sent.send(()).unwrap();
        loop {
            if let orchbee_workbench_protocol::MultiplexMessage::StreamClose { stream_id, .. } =
                orchbee_workbench_protocol::read_multiplex_message(&mut peer)?
                && stream_id == accepted
            {
                break;
            }
        }
        closed.send(()).unwrap();
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort")
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut listener = Subscription::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    let mut delivered = listener.poll(true);
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while delivered.is_none() {
        delivered = listener.poll(true);
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(delivered, Some(expected));
    assert_eq!(listener.poll(true), None);
    listener.close();
    // Closing the carrier may discard a queued cancellation, so observe it first.
    cancelled.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(client);
    server.finish().unwrap();
}

#[test]
fn ui_handoff_is_once_and_an_intervening_modal_discards_the_message() {
    let listener = Subscription {
        source: None,
        state: Mutex::default(),
    };
    let message = Notification::normalize(
        "title",
        None,
        orchbee_workbench_protocol::NotificationPosition::TopRight,
        NotificationSound::None,
    )
    .unwrap();
    listener.state.lock().unwrap().message = Some(message.clone());
    assert_eq!(listener.poll(true), Some(message.clone()));
    assert_eq!(listener.poll(true), None);
    listener.state.lock().unwrap().message = Some(message);
    assert_eq!(listener.poll(false), None);
    assert_eq!(listener.poll(true), None);
}
