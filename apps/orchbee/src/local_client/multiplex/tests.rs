//! Controlled peer traces cover cancellation and queue loss across actual carrier framing.
use super::*;
use crate::local_client::{
    LocalClient,
    tests::{
        multiplex_peer::{admit, read_open},
        server::FakeServer,
    },
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    read_multiplex_message, write_multiplex_message,
};
use std::sync::mpsc;

fn request(id: u64) -> WorkbenchRequest {
    WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
    })
}
fn frame(
    peer: &mut LocalStream,
    stream_id: MultiplexStreamId,
    sequence: u64,
    kind: StreamFrameKind,
    value: u8,
) -> io::Result<()> {
    write_multiplex_message(
        peer,
        &MultiplexMessage::StreamFrame {
            stream_id,
            sequence: NonZeroU64::new(sequence).unwrap(),
            kind,
            record: vec![value],
        },
    )
}
fn take(stream: &Subscription) -> io::Result<StreamEvent> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(event) = stream.poll()? {
            return Ok(event);
        }
        assert!(Instant::now() < deadline, "carrier delivery deadline");
        thread::yield_now();
    }
}

#[test]
fn cancellation_fences_a_late_reply_while_another_stream_completes() {
    let (opened, ready) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let first = read_open(&mut peer)?;
        let second = read_open(&mut peer)?;
        opened.send(()).unwrap();
        loop {
            if let MultiplexMessage::StreamClose { stream_id, .. } =
                read_multiplex_message(&mut peer)?
            {
                assert_eq!(stream_id, first.id);
                break;
            }
        }
        frame(&mut peer, first.id, 1, StreamFrameKind::Result, 1)?;
        frame(&mut peer, second.id, 1, StreamFrameKind::Result, 2)?;
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort")
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let first = client
        .origin()
        .subscribe(&request(1), StreamMode::Request)
        .unwrap();
    let second = client
        .origin()
        .subscribe(&request(2), StreamMode::Request)
        .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(first.cancel(), "the peer already received this request");
    assert!(first.poll().is_err());
    assert!(matches!(take(&second).unwrap(), StreamEvent::Record(bytes) if bytes == [2]));
    drop((first, second, client));
    server.finish().unwrap();
}

#[test]
fn event_overflow_requires_an_observed_gap_before_the_replacement_snapshot() {
    let (resynced, ready) = mpsc::sync_channel(1);
    let (release, finish) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let open = read_open(&mut peer)?;
        for sequence in 1..=34 {
            frame(
                &mut peer,
                open.id,
                sequence,
                if sequence == 1 {
                    StreamFrameKind::Snapshot
                } else {
                    StreamFrameKind::Update
                },
                sequence as u8,
            )?;
        }
        loop {
            if let MultiplexMessage::Resnapshot { stream_id } = read_multiplex_message(&mut peer)? {
                assert_eq!(stream_id, open.id);
                break;
            }
        }
        write_multiplex_message(
            &mut peer,
            &MultiplexMessage::Gap {
                stream_id: open.id,
                through: NonZeroU64::new(34).unwrap(),
            },
        )?;
        frame(&mut peer, open.id, 35, StreamFrameKind::Snapshot, 35)?;
        resynced.send(()).unwrap();
        finish.recv_timeout(Duration::from_secs(2)).unwrap();
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort")
        .unwrap()
        .into_multiplexed()
        .unwrap();
    let stream = client
        .origin()
        .subscribe(&request(1), StreamMode::Events)
        .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(take(&stream).unwrap(), StreamEvent::Gap));
    loop {
        match take(&stream).unwrap() {
            StreamEvent::Gap => {}
            StreamEvent::Record(bytes) => {
                assert_eq!(bytes, [35]);
                break;
            }
            StreamEvent::Disconnected => panic!("resnapshot preserves the connection"),
        }
    }
    release.send(()).unwrap();
    drop((stream, client));
    server.finish().unwrap();
}

#[test]
fn state_coalescing_releases_the_connection_budget_and_overrun_is_typed() {
    let budget = Arc::new(AtomicUsize::new(0));
    let state = Delivery::new(Arc::clone(&budget)).unwrap();
    state
        .publish(StreamEvent::Record(vec![1; 8]), StreamMode::State)
        .unwrap();
    state
        .publish(StreamEvent::Record(vec![2; 4]), StreamMode::State)
        .unwrap();
    assert_eq!(budget.load(Ordering::Acquire), 4);
    assert!(matches!(state.take().unwrap(), Some(StreamEvent::Record(bytes)) if bytes == [2; 4]));
    assert_eq!(budget.load(Ordering::Acquire), 0);
    budget.store(MAXIMUM_MULTIPLEX_BYTES - 1, Ordering::Release);
    let error = state
        .publish(StreamEvent::Record(vec![3; 2]), StreamMode::Events)
        .unwrap_err();
    assert!(matches!(
        error
            .get_ref()
            .and_then(|error| error.downcast_ref::<Disconnect>()),
        Some(Disconnect(
            orchbee_workbench_protocol::MultiplexDisconnectReason::Capacity
        ))
    ));
    budget.store(0, Ordering::Release);
}
