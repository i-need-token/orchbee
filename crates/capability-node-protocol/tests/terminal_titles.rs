use orchbee_capability_node_protocol::*;
use std::{io::Cursor, num::NonZeroU64};
fn identity(n: u8) -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([1; 16]).unwrap(),
        HostEpoch::from_bytes([2; 16]).unwrap(),
        HostTerminalId::from_bytes([n; 16]).unwrap(),
    )
}
fn fixture() -> TerminalTitlesRequest {
    TerminalTitlesRequest {
        request_id: OperationId::from_bytes([5; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([6; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        identities: vec![identity(3), identity(4)],
    }
}
#[test]
fn title_request_and_large_reply_roundtrip_on_both_readers() {
    let request = fixture();
    let result = TerminalTitlesResult {
        request_id: request.request_id,
        rows: request
            .identities
            .iter()
            .map(|identity| TerminalTitleRow {
                identity: *identity,
                observation: TerminalTitleObservation::Observed {
                    revision: NonZeroU64::new(u64::MAX).unwrap(),
                    title: Some("🙂".repeat(16384)),
                },
            })
            .collect(),
    };
    assert!(result.answers(&request));
    for message in [
        CapabilityNodeMessage::TerminalTitlesRequest(request),
        CapabilityNodeMessage::TerminalTitlesResult(result),
    ] {
        let frame = encode_message(&message).unwrap();
        assert_eq!(decode_message(&frame).unwrap(), message);
        assert_eq!(read_message(&mut Cursor::new(frame)).unwrap(), message);
    }
}
#[test]
fn invalid_title_frames_counts_lengths_and_correlations_are_rejected() {
    let mut request = fixture();
    request.identities.push(identity(3));
    assert!(
        encode_message(&CapabilityNodeMessage::TerminalTitlesRequest(
            request.clone()
        ))
        .is_err()
    );
    request = fixture();
    let mut result = TerminalTitlesResult {
        request_id: request.request_id,
        rows: request
            .identities
            .iter()
            .map(|id| TerminalTitleRow {
                identity: *id,
                observation: TerminalTitleObservation::Unavailable,
            })
            .collect(),
    };
    assert!(result.answers(&request));
    result.rows.reverse();
    assert!(!result.answers(&request));
    result.rows.reverse();
    result.rows[0].observation = TerminalTitleObservation::Observed {
        revision: NonZeroU64::MIN,
        title: None,
    };
    let frame = encode_message(&CapabilityNodeMessage::TerminalTitlesResult(result)).unwrap();
    for (offset, value) in [(6, 1), (28, 5), (77, 2)] {
        let mut bad = frame.clone();
        bad[offset] = value;
        assert!(decode_message(&bad).is_err());
    }
    let mut extra = frame.clone();
    extra.push(0);
    assert!(decode_message(&extra).is_err());
    assert!(decode_message(&frame[..frame.len() - 1]).is_err());
    let mut oversize = frame[..12].to_vec();
    oversize[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(read_message(&mut Cursor::new(oversize)).is_err());
}
