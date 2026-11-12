//! The two-ended exercise: state machines joined by in-memory byte queues whose readers take odd
//! pieces, so frames straddle reads — open, flood, half-close, cancel, and a three-machine relay —
//! and the rules the fixtures cannot express in a few frames: the open-channel ceiling, the route
//! reply bound, and event coalescing.

use std::collections::VecDeque;

use orchbee_machine_protocol::{
    CREDIT_REGRANT_THRESHOLD, Connection, Count, Delivery, Extensions, FrameKind, INITIAL_CREDIT,
    MAXIMUM_DATA_PAYLOAD_BYTES, MAXIMUM_OPEN_CHANNELS, MAXIMUM_ROUTE_REPLY_BYTES, MachineId,
    Message, Open, Record, ResetReason, Role, SendError, Verdict, Violation,
    hub::{self, Hub},
    link::{self, Link},
};

use crate::support::{self, TestResult, Wire, absorb, transmit};

/// The reader's piece: odd, so headers and payloads split across reads.
const PIECE: usize = 4_099;
const WINDOW: usize = INITIAL_CREDIT as usize;
const MACHINE: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";

fn data_on(channel: u32, deliveries: &[Delivery<Link>]) -> Vec<u8> {
    deliveries
        .iter()
        .flat_map(|delivery| match delivery {
            Delivery::Data { channel: on, bytes } if *on == channel => bytes.clone(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn open_with_an_early_request_ends_with_no_open_channel() -> TestResult {
    let mut hub = Connection::<Link>::initiator();
    let mut helper = Connection::<Link>::acceptor(support::facts()?);
    let (mut down, mut up) = (Wire::new(PIECE), Wire::new(PIECE));
    let request = br#"{"argv":["status"]}"#;
    let id = hub.open(link::Channel::Route)?;
    assert_eq!(hub.write(id, request)?, request.len());
    hub.end(id)?;
    transmit(&mut hub, &mut down);
    assert_eq!(
        absorb(&mut helper, &mut down)?,
        vec![
            Delivery::Open {
                channel: id,
                open: link::Channel::Route,
                extensions: Extensions::new(),
            },
            Delivery::Data {
                channel: id,
                bytes: request.to_vec(),
            },
            Delivery::End { channel: id },
        ]
    );
    helper.take(id, request.len())?;
    helper.accept(id)?;
    let reply = br#"{"ok":true}"#;
    assert_eq!(helper.write(id, reply)?, reply.len());
    helper.close(id, link::Outcome::Replied)?;
    transmit(&mut helper, &mut up);
    let delivered = absorb(&mut hub, &mut up)?;
    assert!(
        matches!(
            delivered.as_slice(),
            [
                Delivery::Message(Message::Event {
                    event: link::Event::Facts(_),
                    ..
                }),
                Delivery::Accept { .. },
                Delivery::Data { bytes, .. },
                Delivery::Close {
                    outcome: link::Outcome::Replied,
                    ..
                },
            ] if bytes == reply
        ),
        "{delivered:?}"
    );
    assert_eq!(hub.open_channels(), 0);
    assert_eq!(helper.open_channels(), 0);
    Ok(())
}

#[test]
fn a_flood_stops_at_one_window_and_other_frames_keep_moving() -> TestResult {
    const TOTAL: usize = 8 * 1024 * 1024;
    let stream = support::pattern(TOTAL);
    let mut hub = Connection::<Link>::initiator();
    let mut helper = Connection::<Link>::acceptor(support::facts()?);
    let (mut down, mut up) = (Wire::new(PIECE), Wire::new(PIECE));

    let flood = hub.open(support::session("default")?)?;
    let mut sent = hub.write(flood, &stream)?;
    assert_eq!(
        sent, WINDOW,
        "the sender stops at exactly the initial credit"
    );
    assert_eq!(hub.write(flood, stream.get(sent..).unwrap_or_default())?, 0);
    hub.ping(*b"liveness")?;
    let second = hub.open(support::session("second")?)?;
    assert_eq!(hub.write(second, b"still moving")?, 12);

    let mut order = Vec::new();
    while let Some(frame) = hub.poll_transmit() {
        order.push((frame.kind(), frame.channel()));
        down.write(&frame);
    }
    let mut expected = vec![
        (FrameKind::Open, flood),
        (FrameKind::Ping, 0),
        (FrameKind::Open, second),
        (FrameKind::Data, flood),
        (FrameKind::Data, second),
    ];
    expected.extend(std::iter::repeat_n(
        (FrameKind::Data, flood),
        WINDOW / MAXIMUM_DATA_PAYLOAD_BYTES - 1,
    ));
    assert_eq!(
        order, expected,
        "other frames first, then one data frame per ready channel per round"
    );

    let delivered = absorb(&mut helper, &mut down)?;
    let mut received = data_on(flood, &delivered);
    assert_eq!(received.len(), WINDOW, "the receiver holds one window");
    assert_eq!(data_on(second, &delivered), b"still moving");
    helper.accept(flood)?;
    helper.accept(second)?;
    transmit(&mut helper, &mut up);
    assert!(absorb(&mut hub, &mut up)?.contains(&Delivery::Pong));
    assert_eq!(
        hub.write(flood, stream.get(sent..).unwrap_or_default())?,
        0,
        "no credit while the consumer takes nothing"
    );

    let mut taken = 0;
    while !helper.has_pending() && taken < WINDOW {
        helper.take(flood, MAXIMUM_DATA_PAYLOAD_BYTES)?;
        taken += MAXIMUM_DATA_PAYLOAD_BYTES;
    }
    assert!(helper.has_pending(), "taking the window regrants credit");
    assert!(
        taken <= CREDIT_REGRANT_THRESHOLD as usize,
        "the regrant comes by {CREDIT_REGRANT_THRESHOLD} taken bytes, not after {taken}"
    );
    helper.take(flood, WINDOW - taken)?;

    while received.len() < TOTAL {
        let written = hub.write(flood, stream.get(sent..).unwrap_or_default())?;
        sent += written;
        let moved = transmit(&mut hub, &mut down) + transmit(&mut helper, &mut up);
        for delivery in absorb(&mut helper, &mut down)? {
            if let Delivery::Data { channel, bytes } = delivery {
                helper.take(channel, bytes.len())?;
                received.extend(bytes);
            }
        }
        absorb(&mut hub, &mut up)?;
        assert!(
            written + moved > 0,
            "the flood stalled at {}",
            received.len()
        );
    }
    assert!(received == stream, "every byte arrives in order");
    Ok(())
}

#[test]
fn half_close_delivers_every_byte_before_the_result() -> TestResult {
    const TOTAL: usize = 1024 * 1024;
    let reply = support::pattern(TOTAL);
    let mut hub = Connection::<Link>::initiator();
    let mut helper = Connection::<Link>::acceptor(support::facts()?);
    let (mut down, mut up) = (Wire::new(PIECE), Wire::new(PIECE));
    let id = hub.open(support::session("default")?)?;
    assert_eq!(hub.write(id, b"input")?, 5);
    hub.end(id)?;
    transmit(&mut hub, &mut down);
    absorb(&mut helper, &mut down)?;
    helper.take(id, 5)?;
    helper.accept(id)?;

    let (mut sent, mut received, mut result) = (0, Vec::new(), None);
    while result.is_none() {
        if sent < TOTAL {
            sent += helper.write(id, reply.get(sent..).unwrap_or_default())?;
            if sent == TOTAL {
                helper.close(id, link::Outcome::Ended)?;
            }
        }
        let moved = transmit(&mut helper, &mut up) + transmit(&mut hub, &mut down);
        for delivery in absorb(&mut hub, &mut up)? {
            match delivery {
                Delivery::Data { channel, bytes } => {
                    assert_eq!(result, None, "data after the result");
                    hub.take(channel, bytes.len())?;
                    received.extend(bytes);
                }
                Delivery::Close { outcome, .. } => result = Some(outcome),
                _ => {}
            }
        }
        absorb(&mut helper, &mut down)?;
        assert!(
            moved > 0 || result.is_some(),
            "half-close stalled at {sent}"
        );
    }
    assert_eq!(result, Some(link::Outcome::Ended));
    assert!(received == reply, "every byte arrives before the result");
    assert_eq!(hub.open_channels() + helper.open_channels(), 0);
    Ok(())
}

#[test]
fn cancel_discards_late_frames_and_never_reuses_the_id() -> TestResult {
    let machine = MachineId::new(MACHINE)?;
    let mut client = Connection::<Hub>::initiator();
    let mut hub = Connection::<Hub>::acceptor();
    let (mut up, mut down) = (Wire::new(PIECE), Wire::new(PIECE));
    let first = client.open(hub::Channel::Session {
        machine: machine.clone(),
    })?;
    transmit(&mut client, &mut up);
    absorb(&mut hub, &mut up)?;
    hub.accept(first)?;
    assert_eq!(hub.write(first, &support::pattern(200_000))?, 200_000);
    transmit(&mut hub, &mut down);

    for _ in 0..2 {
        let frame = down.read()?.ok_or("the hub wrote accept and data")?;
        assert_eq!(client.receive(&frame)?.verdict, Verdict::Accepted);
    }
    assert_eq!(client.write(first, b"pending")?, 7);
    client.reset(first, ResetReason::Cancelled)?;
    let mut late = 0;
    while let Some(frame) = down.read()? {
        assert_eq!(client.receive(&frame)?.verdict, Verdict::Discarded);
        late += 1;
    }
    assert!(late > 0, "frames already queued arrive after the cancel");
    let cancel = client.poll_transmit().ok_or("the cancel")?;
    assert_eq!((cancel.kind(), cancel.channel()), (FrameKind::Reset, first));
    assert_eq!(
        client.poll_transmit(),
        None,
        "the cancel drops pending data"
    );
    up.write(&cancel);
    assert!(matches!(
        absorb(&mut hub, &mut up)?.as_slice(),
        [Delivery::Reset { channel, reason: ResetReason::Cancelled, .. }] if *channel == first
    ));
    assert_eq!(hub.write(first, b"more"), Err(SendError::UnknownChannel));

    let next = client.open(hub::Channel::Route { machine })?;
    assert_eq!(next, first + 2, "a cancelled id is never reused");
    transmit(&mut client, &mut up);
    absorb(&mut hub, &mut up)?;
    hub.accept(next)?;
    assert_eq!(hub.write(next, b"{}")?, 2);
    hub.close(next, hub::Outcome::Replied)?;
    transmit(&mut hub, &mut down);
    assert!(matches!(
        absorb(&mut client, &mut down)?.as_slice(),
        [
            Delivery::Accept { .. },
            Delivery::Data { .. },
            Delivery::Close {
                outcome: hub::Outcome::Replied,
                ..
            },
        ]
    ));
    assert_eq!(client.open_channels() + hub.open_channels(), 0);
    Ok(())
}

/// The hub's splice of one client `session` channel onto one link `session` channel.
#[derive(Default)]
struct Splice {
    near: Option<u32>,
    far: Option<u32>,
    buffer: VecDeque<u8>,
}

#[test]
fn a_relay_holds_at_most_one_window_per_hop_while_the_far_consumer_stalls() -> TestResult {
    const TOTAL: usize = 4 * 1024 * 1024;
    let stream = support::pattern(TOTAL);
    let mut client = Connection::<Hub>::initiator();
    let mut front = Connection::<Hub>::acceptor();
    let mut back = Connection::<Link>::initiator();
    let mut helper = Connection::<Link>::acceptor(support::facts()?);
    let mut wires: [Wire; 4] = std::array::from_fn(|_| Wire::new(PIECE));
    let [client_up, client_down, link_down, link_up] = &mut wires;
    let mut splice = Splice::default();
    let near = client.open(hub::Channel::Session {
        machine: MachineId::new(MACHINE)?,
    })?;
    let (mut sent, mut far, mut untaken, mut stalled) = (0, Vec::new(), 0, true);

    while far.len() < TOTAL {
        let written = client.write(near, stream.get(sent..).unwrap_or_default())?;
        sent += written;
        let mut progress = written
            + transmit(&mut client, client_up)
            + transmit(&mut front, client_down)
            + transmit(&mut back, link_down)
            + transmit(&mut helper, link_up);
        for delivery in absorb(&mut front, client_up)? {
            match delivery {
                Delivery::Open { channel, .. } => {
                    splice.near = Some(channel);
                    splice.far = Some(back.open(support::session("default")?)?);
                }
                Delivery::Data { bytes, .. } => splice.buffer.extend(bytes),
                _ => {}
            }
        }
        for delivery in absorb(&mut back, link_up)? {
            if let (Delivery::Accept { .. }, Some(near)) = (delivery, splice.near) {
                front.accept(near)?;
            }
        }
        if let (Some(near), Some(far_channel)) = (splice.near, splice.far) {
            let spliced = back.write(far_channel, splice.buffer.make_contiguous())?;
            splice.buffer.drain(..spliced);
            front.take(near, spliced)?;
            progress += spliced;
        }
        for delivery in absorb(&mut helper, link_down)? {
            match delivery {
                Delivery::Open { channel, .. } => helper.accept(channel)?,
                Delivery::Data { channel, bytes } => {
                    if stalled {
                        untaken += bytes.len();
                    } else {
                        helper.take(channel, bytes.len())?;
                    }
                    far.extend(bytes);
                }
                _ => {}
            }
        }
        absorb(&mut client, client_down)?;
        if progress > 0 {
            continue;
        }
        assert!(stalled, "the relay stalled at {} bytes", far.len());
        assert_eq!(far.len(), WINDOW, "the far end holds one window");
        assert_eq!(splice.buffer.len(), WINDOW, "the hub holds one window");
        assert_eq!(sent, 2 * WINDOW, "the near sender stops two windows out");
        stalled = false;
        helper.take(splice.far.ok_or("the far channel")?, untaken)?;
    }
    assert!(far == stream, "every byte crosses both hops in order");
    Ok(())
}

#[test]
fn the_257th_open_is_refused_with_reset_capacity() -> TestResult {
    let machine = MachineId::new(MACHINE)?;
    let session = || hub::Channel::Session {
        machine: machine.clone(),
    };
    let mut client = Connection::<Hub>::initiator();
    let mut hub = Connection::<Hub>::acceptor();
    let mut ids = Vec::new();
    for _ in 0..MAXIMUM_OPEN_CHANNELS {
        ids.push(client.open(session())?);
    }
    assert_eq!(client.open(session()), Err(SendError::Capacity));
    let mut up = Wire::new(PIECE);
    transmit(&mut client, &mut up);
    assert_eq!(absorb(&mut hub, &mut up)?.len(), MAXIMUM_OPEN_CHANNELS);
    assert_eq!(hub.open_channels(), MAXIMUM_OPEN_CHANNELS);

    let next = ids.last().ok_or("an id")? + 2;
    let extra = Record::<Hub>::Open(Open::Known {
        channel: session(),
        extensions: Extensions::new(),
    })
    .encode(next)?;
    let receipt = hub.receive(&extra)?;
    assert_eq!((receipt.verdict, receipt.delivery), (Verdict::Reply, None));
    let refusal = hub.poll_transmit().ok_or("the refusal")?;
    assert_eq!(
        Record::<Hub>::decode(Role::Acceptor, &refusal)?,
        Record::Reset {
            reason: ResetReason::Capacity,
            extensions: Extensions::new(),
        }
    );
    assert_eq!(refusal.channel(), next);
    assert_eq!(hub.open_channels(), MAXIMUM_OPEN_CHANNELS);
    Ok(())
}

#[test]
fn a_route_reply_may_reach_its_bound_and_not_one_byte_more() -> TestResult {
    let bound = usize::try_from(MAXIMUM_ROUTE_REPLY_BYTES)?;
    let reply = support::pattern(bound);
    let mut hub = Connection::<Link>::initiator();
    let mut helper = Connection::<Link>::acceptor(support::facts()?);
    let (mut down, mut up) = (Wire::new(PIECE), Wire::new(PIECE));
    let id = hub.open(link::Channel::Route)?;
    hub.end(id)?;
    transmit(&mut hub, &mut down);
    absorb(&mut helper, &mut down)?;
    helper.accept(id)?;

    let (mut sent, mut received) = (0, 0);
    while received < bound {
        sent += helper.write(id, reply.get(sent..).unwrap_or_default())?;
        let moved = transmit(&mut helper, &mut up) + transmit(&mut hub, &mut down);
        for delivery in absorb(&mut hub, &mut up)? {
            if let Delivery::Data { channel, bytes } = delivery {
                assert!(
                    bytes
                        .iter()
                        .zip(received..)
                        .all(|(byte, at)| usize::from(*byte) == at % 251)
                );
                received += bytes.len();
                hub.take(channel, bytes.len())?;
            }
        }
        absorb(&mut helper, &mut down)?;
        assert!(moved > 0, "the reply stalled at {received}");
    }
    assert_eq!(helper.write(id, &[0]), Err(SendError::DataBound));

    let over = Record::<Link>::Data(vec![0]).encode(id)?;
    assert_eq!(hub.receive(&over).err(), Some(Violation::DataBound));
    Ok(())
}

#[test]
fn a_state_event_replaces_the_newest_undelivered_event_of_its_subject_in_place() -> TestResult {
    let catalog = |revision| -> Result<hub::Event, Box<dyn std::error::Error>> {
        Ok(hub::Event::Catalog(hub::Catalog::new(
            Count::new(revision)?,
            Vec::new(),
        )?))
    };
    let mut hub = Connection::<Hub>::acceptor();
    hub.event(catalog(1)?)?;
    hub.event(hub::Event::Hub(hub::HubState::Held))?;
    hub.event(catalog(2)?)?;
    hub.event(hub::Event::Hub(hub::HubState::Closing))?;
    let mut sent = Vec::new();
    while let Some(frame) = hub.poll_transmit() {
        match Record::<Hub>::decode(Role::Acceptor, &frame)? {
            Record::Message(Message::Event { event, .. }) => sent.push(event),
            other => return Err(format!("{other:?} is not an event").into()),
        }
    }
    assert_eq!(
        sent,
        vec![catalog(2)?, hub::Event::Hub(hub::HubState::Closing)],
        "each subject keeps its first place and its newest value"
    );
    hub.event(catalog(3)?)?;
    hub.event(catalog(4)?)?;
    let frame = hub.poll_transmit().ok_or("the newest catalog")?;
    assert!(matches!(
        Record::<Hub>::decode(Role::Acceptor, &frame)?,
        Record::Message(Message::Event { event, .. }) if event == catalog(4)?
    ));
    Ok(())
}
