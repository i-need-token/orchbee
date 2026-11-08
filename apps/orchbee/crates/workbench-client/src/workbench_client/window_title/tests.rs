use super::*;
use crate::local_client::{
    LocalClient,
    test_support::{
        multiplex_peer::{admit, read_open, respond},
        server::FakeServer,
    },
};
use orchbee_workbench_protocol::{WindowTitleResult, WorkbenchRevision, read_workbench_request};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn title_uses_one_admission_and_cancellation_preserves_the_carrier() {
    let (opened, ready) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let open = read_open(&mut peer)?;
        assert!(matches!(
            read_workbench_request(&mut Cursor::new(&open.request)).unwrap(),
            WorkbenchRequest::WindowTitle(_)
        ));
        opened.send(open.id).unwrap();
        loop {
            if let orchbee_workbench_protocol::MultiplexMessage::StreamClose { stream_id, .. } =
                orchbee_workbench_protocol::read_multiplex_message(&mut peer)?
            {
                assert_eq!(stream_id, open.id);
                break;
            }
        }
        let next = read_open(&mut peer)?;
        let WorkbenchRequest::SnapshotRead(request) =
            read_workbench_request(&mut Cursor::new(&next.request)).unwrap()
        else {
            panic!()
        };
        respond(
            &mut peer,
            &next,
            &WorkbenchResponse::Snapshot(orchbee_workbench_protocol::WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Denied {},
            }),
            1,
        )
    })
    .unwrap();
    let mut client = LocalClient::connect(server.endpoint())
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut listener = Subscription::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    listener.close();
    let reply = client
        .submit_workbench_snapshot_read_before_cancellable(
            orchbee_workbench_protocol::SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
            },
            Instant::now() + Duration::from_secs(2),
            None,
        )
        .unwrap();
    assert!(matches!(
        reply.outcome,
        orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Denied {}
    ));
    drop(client);
    server.finish().unwrap();
}

#[test]
fn title_latest_value_is_consumed_once_without_a_lane_thread() {
    let (sent, ready) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let open = read_open(&mut peer)?;
        let WorkbenchRequest::WindowTitle(request) =
            read_workbench_request(&mut Cursor::new(&open.request)).unwrap()
        else {
            panic!()
        };
        for sequence in 1..=3 {
            respond(
                &mut peer,
                &open,
                &WorkbenchResponse::WindowTitle(WindowTitleResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id.clone(),
                    session_id: request.session_id.clone(),
                    outcome: WindowTitleOutcome::Listening {
                        snapshot: Some(WindowTitleSnapshot {
                            revision: WorkbenchRevision::new(sequence),
                            title: Some(
                                orchbee_workbench_protocol::WindowTitle::normalize(&format!(
                                    "title-{sequence}"
                                ))
                                .unwrap(),
                            ),
                        }),
                    },
                }),
                sequence,
            )?;
        }
        sent.send(()).unwrap();
        while let Ok(message) = orchbee_workbench_protocol::read_multiplex_message(&mut peer) {
            if matches!(
                message,
                orchbee_workbench_protocol::MultiplexMessage::StreamClose { .. }
            ) {
                break;
            }
        }
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint())
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let mut listener = Subscription::spawn(
        client.origin(),
        client.operation_clock(),
        SessionId::parse(&format!("{:032x}", 2)).unwrap(),
    )
    .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if listener
            .poll(false, Default::default())
            .is_some_and(|snapshot| snapshot.revision.value() == 3)
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(listener.poll(false, Default::default()), None);
    listener.close();
    drop(client);
    server.finish().unwrap();
}

#[test]
fn title_handoff_republishes_on_promotion_and_suppresses_unchanged_values() {
    let listener = Subscription {
        source: None,
        state: Mutex::default(),
    };
    let snapshot = WindowTitleSnapshot {
        revision: WorkbenchRevision::new(1),
        title: None,
    };
    listener
        .state
        .lock()
        .unwrap()
        .receive(Some(snapshot.clone()));
    assert_eq!(
        listener.poll(false, Default::default()),
        Some(snapshot.clone())
    );
    listener
        .state
        .lock()
        .unwrap()
        .receive(Some(snapshot.clone()));
    assert_eq!(listener.poll(false, Default::default()), None);
    assert_eq!(listener.poll(true, Default::default()), None);
    listener
        .state
        .lock()
        .unwrap()
        .receive(Some(snapshot.clone()));
    assert_eq!(listener.poll(true, Default::default()), Some(snapshot));
}
