use orchbee_files_protocol::{LinkSide, decode_link_message};

use super::*;

#[test]
fn every_call_closes_its_client_stream_once() {
    let outbound = Arc::new(Outbound::new());
    let calls = Calls::new(Arc::clone(&outbound));
    let id = |value| CallId::new(value).unwrap();
    let connection = ConnectionId::new(1).unwrap();
    let lane = LaneId::new(2).unwrap();
    let cancelled = calls.open(id(1), connection, lane).unwrap().unwrap();
    let abandoned = calls
        .open(id(2), connection, LaneId::new(3).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        calls.open(id(1), connection, lane).err(),
        Some(DuplicateCall)
    );
    calls.cancel_lane(connection, lane);
    drop(cancelled);
    drop(abandoned);
    calls.drain();
    assert!(calls.open(id(3), connection, lane).unwrap().is_none());
    assert!(calls.settled_before(Instant::now()));
    let closes: Vec<_> = (0..3)
        .map(|_| decode_link_message(&outbound.next().unwrap(), LinkSide::Files).unwrap())
        .collect();
    assert_eq!(
        closes,
        [
            LinkMessage::Close {
                call: id(1),
                reason: CallClose::Cancelled,
            },
            LinkMessage::Close {
                call: id(2),
                reason: CallClose::OutcomeUnknown,
            },
            LinkMessage::Close {
                call: id(3),
                reason: CallClose::Stopping,
            },
        ]
    );
}
