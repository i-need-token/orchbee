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

fn changes_request(after: u64) -> TerminalTitleChangesRequest {
    TerminalTitleChangesRequest {
        request_id: OperationId::from_bytes([5; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([6; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        after,
    }
}
fn observed(n: u8, title: Option<String>) -> TerminalTitleRow {
    TerminalTitleRow {
        identity: identity(n),
        observation: TerminalTitleObservation::Observed {
            revision: NonZeroU64::MIN,
            title,
        },
    }
}
fn page(
    after: u64,
    rows: Vec<TerminalTitleRow>,
    next: u64,
    complete: bool,
) -> TerminalTitleChangesResult {
    TerminalTitleChangesResult {
        request: changes_request(after),
        outcome: TerminalTitleChangesOutcome::Page(TerminalTitleChangePage {
            rows,
            next,
            complete,
        }),
    }
}
fn assert_both_readers(frame: &[u8], message: &CapabilityNodeMessage) {
    assert_eq!(&decode_message(frame).unwrap(), message);
    assert_eq!(&read_message(&mut Cursor::new(frame)).unwrap(), message);
}

#[test]
fn change_note_read_and_page_have_their_specified_bytes() {
    let note = CapabilityNodeMessage::TerminalTitlesChanged(TerminalTitlesChanged {
        through: NonZeroU64::new(0x0102_0304_0506_0708).unwrap(),
    });
    let mut expected = b"OBCN\0\x01\0\x09\x23".to_vec();
    expected.extend([1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(encode_message(&note).unwrap(), expected);
    assert_both_readers(&expected, &note);

    let request = CapabilityNodeMessage::TerminalTitleChangesRequest(changes_request(7));
    let mut echo = [5; 16].to_vec();
    echo.extend([6; 16]);
    echo.extend(1_u64.to_be_bytes());
    let mut expected = b"OBCN\0\x01\0\x31\x24".to_vec();
    expected.extend(&echo);
    expected.extend(7_u64.to_be_bytes());
    assert_eq!(encode_message(&request).unwrap(), expected);
    assert_both_readers(&expected, &request);

    let result = CapabilityNodeMessage::TerminalTitleChangesResult(page(
        0,
        vec![observed(3, Some("a".into()))],
        3,
        true,
    ));
    let mut body = echo.clone();
    body.extend(0_u64.to_be_bytes());
    body.push(1);
    body.extend(3_u64.to_be_bytes());
    body.extend([1, 0, 1]);
    body.extend([1; 16]);
    body.extend([2; 16]);
    body.extend([3; 16]);
    body.push(1);
    body.extend(1_u64.to_be_bytes());
    body.extend([1, 0, 0, 0, 1, b'a']);
    let mut expected = b"OBNC\0\x01\0\0".to_vec();
    expected.extend(u32::try_from(body.len()).unwrap().to_be_bytes());
    expected.extend(&body);
    assert_eq!(encode_message(&result).unwrap(), expected);
    assert_both_readers(&expected, &result);

    for (outcome, tag) in [
        (TerminalTitleChangesOutcome::Reset, 2),
        (TerminalTitleChangesOutcome::RejectedFencing, 3),
    ] {
        let message =
            CapabilityNodeMessage::TerminalTitleChangesResult(TerminalTitleChangesResult {
                request: changes_request(7),
                outcome,
            });
        let mut expected = b"OBNC\0\x01\0\0\0\0\0\x31".to_vec();
        expected.extend(&echo);
        expected.extend(7_u64.to_be_bytes());
        expected.push(tag);
        assert_eq!(encode_message(&message).unwrap(), expected);
        assert_both_readers(&expected, &message);
    }
}

#[test]
fn a_full_change_page_fits_its_record_and_one_more_title_byte_does_not() {
    let rows: Vec<_> = (1..=MAXIMUM_TERMINAL_TITLE_CHANGES)
        .map(|n| TerminalTitleRow {
            identity: HostTerminalIdentity::new(
                HostCreationId::from_bytes([1; 16]).unwrap(),
                HostEpoch::from_bytes([2; 16]).unwrap(),
                HostTerminalId::from_bytes(u128::try_from(n).unwrap().to_be_bytes()).unwrap(),
            ),
            observation: if n == MAXIMUM_TERMINAL_TITLE_CHANGES {
                TerminalTitleObservation::Unavailable
            } else {
                TerminalTitleObservation::Observed {
                    revision: NonZeroU64::MAX,
                    title: Some(if n <= 16 {
                        "🙂".repeat(16384)
                    } else {
                        String::new()
                    }),
                }
            },
        })
        .collect();
    let mut result = page(u64::MAX - 1, rows, u64::MAX, false);
    let message = CapabilityNodeMessage::TerminalTitleChangesResult(result.clone());
    assert_both_readers(&encode_message(&message).unwrap(), &message);
    if let TerminalTitleChangesOutcome::Page(page) = &mut result.outcome {
        page.rows[16].observation = TerminalTitleObservation::Observed {
            revision: NonZeroU64::MIN,
            title: Some("a".into()),
        };
    }
    assert!(encode_message(&CapabilityNodeMessage::TerminalTitleChangesResult(result)).is_err());
}

#[test]
fn change_pages_only_move_forward_and_never_reset_a_fresh_read() {
    let rows = || vec![observed(3, None), observed(4, Some("b".into()))];
    let removed = TerminalTitleRow {
        identity: identity(4),
        observation: TerminalTitleObservation::Unavailable,
    };
    for valid in [
        page(0, vec![], 0, true),
        page(0, vec![], 9, true),
        page(0, rows(), 2, false),
        page(2, vec![removed.clone()], 5, true),
        page(5, vec![], 5, true),
        page(5, vec![], 8, true),
        TerminalTitleChangesResult {
            request: changes_request(5),
            outcome: TerminalTitleChangesOutcome::Reset,
        },
        TerminalTitleChangesResult {
            request: changes_request(0),
            outcome: TerminalTitleChangesOutcome::RejectedFencing,
        },
    ] {
        assert!(valid.answers(&valid.request), "{valid:?}");
    }
    let mut duplicate = rows();
    duplicate.push(observed(3, Some("c".into())));
    for invalid in [
        page(0, vec![removed], 3, true),
        page(5, rows(), 5, true),
        page(5, vec![], 4, true),
        page(5, vec![], 6, false),
        page(0, duplicate, 3, true),
        TerminalTitleChangesResult {
            request: changes_request(0),
            outcome: TerminalTitleChangesOutcome::Reset,
        },
    ] {
        assert!(invalid.validate().is_err(), "{invalid:?}");
        assert!(
            encode_message(&CapabilityNodeMessage::TerminalTitleChangesResult(invalid)).is_err()
        );
    }
    let result = page(0, rows(), 2, true);
    let mut other = changes_request(0);
    other.request_id = OperationId::from_bytes([9; 16]).unwrap();
    assert!(!result.answers(&other));
    assert!(!result.answers(&changes_request(1)));
}

#[test]
fn malformed_change_frames_are_rejected() {
    let note = encode_message(&CapabilityNodeMessage::TerminalTitlesChanged(
        TerminalTitlesChanged {
            through: NonZeroU64::MIN,
        },
    ))
    .unwrap();
    let mut zero = note.clone();
    zero[16] = 0;
    let mut extra = note;
    extra[7] += 1;
    extra.push(0);
    for bad in [zero, extra] {
        assert!(decode_message(&bad).is_err());
    }
    let frame = encode_message(&CapabilityNodeMessage::TerminalTitleChangesResult(page(
        0,
        vec![observed(3, Some("a".into()))],
        3,
        true,
    )))
    .unwrap();
    // Outcome tag, completion byte, row count, observation tag and title tag.
    for (offset, value) in [(60, 4), (69, 2), (70, 1), (120, 2), (129, 2)] {
        let mut bad = frame.clone();
        bad[offset] = value;
        assert!(decode_message(&bad).is_err(), "{offset}");
        assert!(read_message(&mut Cursor::new(bad)).is_err(), "{offset}");
    }
    let mut extra = frame.clone();
    extra.push(0);
    assert!(decode_message(&extra).is_err());
    assert!(decode_message(&frame[..frame.len() - 1]).is_err());
    let mut oversize = frame[..12].to_vec();
    oversize[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(read_message(&mut Cursor::new(oversize)).is_err());
}
