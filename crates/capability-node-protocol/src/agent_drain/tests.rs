use std::num::NonZeroU64;

use super::*;
use crate::{
    CapabilityNodeMessage, HostCreationId, HostEpoch, HostTerminalId, decode_message,
    encode_message,
};

fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("test value")
}

fn identity() -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([1; 16]).expect("id"),
        HostEpoch::from_bytes([2; 16]).expect("epoch"),
        HostTerminalId::from_bytes([3; 16]).expect("terminal"),
    )
}

fn event(sequence: u64, payload_bytes: usize) -> AgentDrainEntry {
    AgentDrainEntry::Event {
        sequence: nz(sequence),
        identity: identity(),
        payload: "a".repeat(payload_bytes),
    }
}

/// Wrap a raw payload in the OBCN frame header so the tests exercise the real
/// `decode_message` entry point including the exact-length check.
fn framed(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(b"OBCN");
    frame.extend_from_slice(&crate::codec::PROTOCOL_VERSION.to_be_bytes());
    frame.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("payload length")
            .to_be_bytes(),
    );
    frame.extend_from_slice(payload);
    frame
}

fn request() -> AgentDrainRequest {
    AgentDrainRequest {
        worker_id: WorkerId::from_bytes(*b"orchbee-local-v1").expect("worker"),
        worker_generation: WorkerGeneration::new(nz(7)),
        token: nz(11),
        acknowledged: 3,
    }
}

#[test]
fn request_frame_is_exactly_forty_one_payload_bytes() -> Result<(), ProtocolError> {
    let frame = encode_message(&CapabilityNodeMessage::AgentDrainRequest(request()))?;
    assert_eq!(frame.len(), 8 + REQUEST_PAYLOAD_BYTES);
    assert_eq!(&frame[..4], b"OBCN");
    assert_eq!(frame[8], AGENT_DRAIN_REQUEST_KIND);
    let decoded = decode_message(&frame)?;
    let CapabilityNodeMessage::AgentDrainRequest(decoded) = decoded else {
        return Err(ProtocolError::InvalidPayload);
    };
    assert_eq!(decoded, request());
    Ok(())
}

#[test]
fn request_rejects_zero_token_generation_and_worker() -> Result<(), ProtocolError> {
    let base = encode_message(&CapabilityNodeMessage::AgentDrainRequest(request()))?;
    // token zero
    let mut frame = base.clone();
    frame[33..41].copy_from_slice(&0_u64.to_be_bytes());
    assert!(decode_message(&frame).is_err());
    // generation zero
    let mut frame = base.clone();
    frame[25..33].copy_from_slice(&0_u64.to_be_bytes());
    assert!(decode_message(&frame).is_err());
    // worker id zero
    let mut frame = base;
    frame[9..25].copy_from_slice(&[0; 16]);
    assert!(decode_message(&frame).is_err());
    Ok(())
}

/// Positive boundary: GAP(1,2) + EVENT3(4155B) + EVENT4(3993B) fills the entry
/// budget exactly at 8165 bytes, matching the g01 fixture shape.
#[test]
fn maximal_exact_page_round_trips() -> Result<(), ProtocolError> {
    let reply = AgentDrainReply::new(
        nz(9),
        0,
        4,
        vec![
            AgentDrainEntry::Gap {
                low: nz(1),
                high: 2,
            },
            event(3, MAXIMUM_SIGNAL_PAYLOAD_BYTES),
            event(
                4,
                8165 - GAP_ENTRY_BYTES
                    - (EVENT_ENTRY_OVERHEAD_BYTES + 4096)
                    - EVENT_ENTRY_OVERHEAD_BYTES,
            ),
        ],
    )?;
    let frame = encode_message(&CapabilityNodeMessage::AgentDrainReply(reply.clone()))?;
    assert_eq!(frame.len(), 8 + REPLY_HEADER_BYTES + REPLY_ENTRY_BYTES);
    assert_eq!(frame.len(), 8 + 8192);
    let CapabilityNodeMessage::AgentDrainReply(decoded) = decode_message(&frame)? else {
        return Err(ProtocolError::InvalidPayload);
    };
    assert_eq!(decoded, reply);
    Ok(())
}

#[test]
fn empty_reply_is_legal() -> Result<(), ProtocolError> {
    let reply = AgentDrainReply::new(nz(1), 0, 0, Vec::new())?;
    let frame = encode_message(&CapabilityNodeMessage::AgentDrainReply(reply.clone()))?;
    let CapabilityNodeMessage::AgentDrainReply(decoded) = decode_message(&frame)? else {
        return Err(ProtocolError::InvalidPayload);
    };
    assert_eq!(decoded, reply);
    Ok(())
}

/// n01: bytes after the exact frame end reject through the outer frame length
/// check and through payload finish.
#[test]
fn trailing_bytes_reject() -> Result<(), ProtocolError> {
    let reply = AgentDrainReply::new(nz(9), 0, 4, vec![event(1, 8)])?;
    let mut frame = encode_message(&CapabilityNodeMessage::AgentDrainReply(reply))?;
    let declared = u16::from_be_bytes([frame[6], frame[7]]);
    frame[6..8].copy_from_slice(&(declared + 1).to_be_bytes());
    frame.push(0);
    assert!(decode_message(&frame).is_err());
    Ok(())
}

/// n02: a coverage hole inside the page rejects (entry 4 after entry 2).
#[test]
fn coverage_hole_rejects() {
    let entries = vec![event(2, 4), event(4, 4)];
    assert!(AgentDrainReply::new(nz(9), 0, 9, entries.clone()).is_err());
    let frame = encode_message(&CapabilityNodeMessage::AgentDrainReply(
        AgentDrainReply::new(nz(9), 0, 9, vec![event(2, 4), event(3, 4)]).expect("valid page"),
    ))
    .expect("frame");
    assert!(decode_message(&frame).is_ok());
    // Same hole encoded raw must reject at decode.
    let mut payload = vec![AGENT_DRAIN_REPLY_KIND];
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&2_u16.to_be_bytes());
    payload.push(1);
    payload.extend_from_slice(&2_u64.to_be_bytes());
    payload.extend_from_slice(&[1; 16]);
    payload.extend_from_slice(&[2; 16]);
    payload.extend_from_slice(&[3; 16]);
    payload.extend_from_slice(&4_u16.to_be_bytes());
    payload.extend_from_slice(b"aaaa");
    payload.push(1);
    payload.extend_from_slice(&4_u64.to_be_bytes());
    payload.extend_from_slice(&[1; 16]);
    payload.extend_from_slice(&[2; 16]);
    payload.extend_from_slice(&[3; 16]);
    payload.extend_from_slice(&4_u16.to_be_bytes());
    payload.extend_from_slice(b"aaaa");
    assert!(decode_message(&framed(&payload)).is_err());
}

/// n03: overlapping coverage rejects (entry 2 after entry 2).
#[test]
fn coverage_overlap_rejects() {
    assert!(AgentDrainReply::new(nz(9), 0, 9, vec![event(2, 4), event(2, 4)]).is_err());
}

/// n04: an inverted GAP range rejects at construction, encode input and decode.
#[test]
fn inverted_gap_rejects() {
    assert!(
        AgentDrainReply::new(
            nz(9),
            0,
            9,
            vec![AgentDrainEntry::Gap {
                low: nz(5),
                high: 2
            }]
        )
        .is_err()
    );
    let mut payload = vec![AGENT_DRAIN_REPLY_KIND];
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&1_u16.to_be_bytes());
    payload.push(2);
    payload.extend_from_slice(&5_u64.to_be_bytes());
    payload.extend_from_slice(&2_u64.to_be_bytes());
    assert!(decode_message(&framed(&payload)).is_err());
}

/// n05: an EVENT with sequence zero cannot decode.
#[test]
fn zero_sequence_event_rejects() {
    let mut payload = vec![AGENT_DRAIN_REPLY_KIND];
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&1_u16.to_be_bytes());
    payload.push(1);
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&[1; 16]);
    payload.extend_from_slice(&[2; 16]);
    payload.extend_from_slice(&[3; 16]);
    payload.extend_from_slice(&0_u16.to_be_bytes());
    assert!(decode_message(&framed(&payload)).is_err());
}

/// The count gate rejects before any allocation: 481 entries cannot be
/// admitted even when the body would otherwise fit.
#[test]
fn entry_count_above_480_rejects_before_allocation() {
    let mut payload = vec![AGENT_DRAIN_REPLY_KIND];
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&u64::MAX.to_be_bytes());
    payload.extend_from_slice(&481_u16.to_be_bytes());
    assert!(decode_message(&framed(&payload)).is_err());
}

#[test]
fn entry_end_above_queue_tail_rejects() {
    // tail 4 while the page covers through 5.
    assert!(AgentDrainReply::new(nz(9), 0, 4, vec![event(5, 0)]).is_err());
}

#[test]
fn entry_bytes_above_budget_reject() {
    // Two maximal events exceed the 8165-byte entry budget.
    let entries = vec![event(1, MAXIMUM_SIGNAL_PAYLOAD_BYTES), {
        let mut entry = event(2, 0);
        if let AgentDrainEntry::Event { payload, .. } = &mut entry {
            *payload = "b".repeat(8165 - EVENT_ENTRY_OVERHEAD_BYTES - 1);
        }
        entry
    }];
    assert!(AgentDrainReply::new(nz(9), 0, 9, entries).is_err());
}

/// Contract: a page whose FINAL entry ends at u64::MAX is legal —
/// constructor accepts, encoder emits, decoder round-trips. The successor
/// bound is only required when another entry exists (root26-A / input44 A1).
#[test]
fn final_event_at_max_round_trips() -> Result<(), ProtocolError> {
    let reply = AgentDrainReply::new(nz(9), 0, u64::MAX, vec![event(u64::MAX, 4)])?;
    let frame = encode_message(&CapabilityNodeMessage::AgentDrainReply(reply.clone()))?;
    assert_eq!(
        frame.len(),
        8 + REPLY_HEADER_BYTES + EVENT_ENTRY_OVERHEAD_BYTES + 4
    );
    let CapabilityNodeMessage::AgentDrainReply(decoded) = decode_message(&frame)? else {
        return Err(ProtocolError::InvalidPayload);
    };
    assert_eq!(decoded, reply);
    assert_eq!(decoded.queue_tail(), u64::MAX);
    Ok(())
}

/// Contract: a FINAL GAP may close at u64::MAX behind a contiguous event
/// (input44 A2).
#[test]
fn final_gap_at_max_round_trips() -> Result<(), ProtocolError> {
    let reply = AgentDrainReply::new(
        nz(9),
        0,
        u64::MAX,
        vec![
            event(5, 4),
            AgentDrainEntry::Gap {
                low: nz(6),
                high: u64::MAX,
            },
        ],
    )?;
    let frame = encode_message(&CapabilityNodeMessage::AgentDrainReply(reply.clone()))?;
    let CapabilityNodeMessage::AgentDrainReply(decoded) = decode_message(&frame)? else {
        return Err(ProtocolError::InvalidPayload);
    };
    assert_eq!(decoded, reply);
    Ok(())
}

/// Decoder must accept a raw wire frame whose single EVENT ends at MAX.
/// Raw bytes exercise `decode_message` itself, not only the constructor's
/// acceptance (input44 A3).
#[test]
fn raw_final_event_at_max_decodes() {
    let mut payload = vec![AGENT_DRAIN_REPLY_KIND];
    payload.extend_from_slice(&9_u64.to_be_bytes()); // token_echo
    payload.extend_from_slice(&0_u64.to_be_bytes()); // acknowledged_echo
    payload.extend_from_slice(&u64::MAX.to_be_bytes()); // queue_tail
    payload.extend_from_slice(&1_u16.to_be_bytes()); // entry count
    payload.push(1); // EVENT tag
    payload.extend_from_slice(&u64::MAX.to_be_bytes()); // sequence
    payload.extend_from_slice(&[1; 16]); // creation id
    payload.extend_from_slice(&[2; 16]); // host epoch
    payload.extend_from_slice(&[3; 16]); // terminal id
    payload.extend_from_slice(&4_u16.to_be_bytes()); // payload length
    payload.extend_from_slice(b"aaaa");
    let frame = framed(&payload);
    let Ok(CapabilityNodeMessage::AgentDrainReply(decoded)) = decode_message(&frame) else {
        panic!("a final EVENT ending at u64::MAX must decode");
    };
    assert_eq!(decoded.queue_tail(), u64::MAX);
    assert_eq!(decoded.entries().len(), 1);
}

/// Any entry after a MAX-ending entry is unrepresentable: typed
/// construction must reject (no wrap to a lower start) (input44 A4).
#[test]
fn entry_after_max_rejects_typed() {
    assert!(
        AgentDrainReply::new(nz(9), 0, u64::MAX, vec![event(u64::MAX, 0), event(1, 0)]).is_err()
    );
    // Same violation through a GAP successor.
    assert!(
        AgentDrainReply::new(
            nz(9),
            0,
            u64::MAX,
            vec![
                event(u64::MAX, 0),
                AgentDrainEntry::Gap {
                    low: nz(1),
                    high: 1
                }
            ]
        )
        .is_err()
    );
}

/// Decoder-side equivalent of the typed rejection: a raw frame with an
/// entry after a MAX-ending entry must be rejected by decode_message.
/// This must stay a decoder rejection — the invalid page is never
/// constructible through AgentDrainReply::new (input44 A5).
#[test]
fn entry_after_max_rejects_on_wire() {
    let mut payload = vec![AGENT_DRAIN_REPLY_KIND];
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&u64::MAX.to_be_bytes());
    payload.extend_from_slice(&2_u16.to_be_bytes()); // TWO entries
    payload.push(1); // EVENT tag
    payload.extend_from_slice(&u64::MAX.to_be_bytes()); // ends at MAX
    payload.extend_from_slice(&[1; 16]);
    payload.extend_from_slice(&[2; 16]);
    payload.extend_from_slice(&[3; 16]);
    payload.extend_from_slice(&0_u16.to_be_bytes()); // empty payload
    payload.push(2); // GAP tag — a successor must not exist after MAX
    payload.extend_from_slice(&1_u64.to_be_bytes());
    payload.extend_from_slice(&1_u64.to_be_bytes());
    assert!(decode_message(&framed(&payload)).is_err());
}

/// A non-final GAP whose high bound is MAX likewise leaves no room for a
/// successor; the wire form must reject in the decoder (input44 A6).
#[test]
fn nonfinal_gap_high_at_max_rejects_on_wire() {
    let mut payload = vec![AGENT_DRAIN_REPLY_KIND];
    payload.extend_from_slice(&9_u64.to_be_bytes());
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&u64::MAX.to_be_bytes());
    payload.extend_from_slice(&2_u16.to_be_bytes());
    payload.push(2); // GAP tag — covers 1..=MAX but is NOT final
    payload.extend_from_slice(&1_u64.to_be_bytes());
    payload.extend_from_slice(&u64::MAX.to_be_bytes());
    payload.push(1); // EVENT tag after the MAX-closing GAP
    payload.extend_from_slice(&1_u64.to_be_bytes());
    payload.extend_from_slice(&[1; 16]);
    payload.extend_from_slice(&[2; 16]);
    payload.extend_from_slice(&[3; 16]);
    payload.extend_from_slice(&0_u16.to_be_bytes());
    assert!(decode_message(&framed(&payload)).is_err());
}

#[test]
fn stale_first_entry_is_legal() -> Result<(), ProtocolError> {
    // The first entry is not required to equal acknowledged_echo + 1: a stale
    // acknowledgment may legally precede already-evicted coverage.
    let reply = AgentDrainReply::new(
        nz(9),
        0,
        6,
        vec![
            AgentDrainEntry::Gap {
                low: nz(1),
                high: 2,
            },
            event(3, 1),
        ],
    )?;
    let frame = encode_message(&CapabilityNodeMessage::AgentDrainReply(reply.clone()))?;
    let CapabilityNodeMessage::AgentDrainReply(decoded) = decode_message(&frame)? else {
        return Err(ProtocolError::InvalidPayload);
    };
    assert_eq!(decoded, reply);
    Ok(())
}
