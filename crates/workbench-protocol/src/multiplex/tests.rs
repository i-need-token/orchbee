use super::*;
use std::io::Cursor;

#[test]
fn partial_and_adjacent_records_preserve_boundaries_and_identities() {
    let messages = [
        MultiplexMessage::LaneOpen {
            lane_id: MultiplexLaneId::new(2).unwrap(),
        },
        MultiplexMessage::StreamOpen {
            stream_id: MultiplexStreamId::new(3).unwrap(),
            lane_id: MultiplexLaneId::new(2).unwrap(),
            mode: StreamMode::Events,
            request: b"OBWP request".to_vec(),
        },
        MultiplexMessage::Gap {
            stream_id: MultiplexStreamId::new(3).unwrap(),
            through: NonZeroU64::MIN,
        },
        MultiplexMessage::StreamFrame {
            stream_id: MultiplexStreamId::new(3).unwrap(),
            sequence: NonZeroU64::new(2).unwrap(),
            kind: StreamFrameKind::Snapshot,
            record: vec![0, 255],
        },
        MultiplexMessage::Disconnect {
            reason: MultiplexDisconnectReason::SlowConsumer,
        },
    ];
    let bytes = messages
        .iter()
        .flat_map(|message| encode_multiplex_message(message).unwrap())
        .collect::<Vec<_>>();
    for chunk in [1, 7, 12, 23, bytes.len()] {
        let mut decoder = MultiplexDecoder::default();
        let mut decoded = Vec::new();
        for slice in bytes.chunks(chunk) {
            decoder.push(slice).unwrap();
            while let Some(message) = decoder.next_message().unwrap() {
                decoded.push(message);
            }
        }
        assert_eq!(decoded, messages);
        assert!(decoder.is_empty());
    }
    let large = MultiplexMessage::StreamFrame {
        stream_id: MultiplexStreamId::new(3).unwrap(),
        sequence: NonZeroU64::MIN,
        kind: StreamFrameKind::Snapshot,
        record: vec![7; 512 * 1024],
    };
    let mut bytes = encode_multiplex_message(&large).unwrap();
    let tail = encode_multiplex_message(&messages[4]).unwrap();
    bytes.extend_from_slice(&tail[..7]);
    let mut decoder = MultiplexDecoder::default();
    decoder.push(&bytes).unwrap();
    assert_eq!(decoder.next_message().unwrap(), Some(large));
    assert!(
        decoder.bytes.capacity() <= 64 * 1024,
        "a delivered payload cannot leave its large scratch allocation resident"
    );
    decoder.push(&tail[7..]).unwrap();
    assert_eq!(decoder.next_message().unwrap(), Some(messages[4].clone()));
    assert!(decoder.is_empty());
    assert!(decoder.bytes.capacity() <= 64 * 1024);
}

#[test]
fn untrusted_headers_and_zero_or_unknown_fields_are_rejected() {
    let open = encode_multiplex_message(&MultiplexMessage::StreamOpen {
        stream_id: MultiplexStreamId::new(1).unwrap(),
        lane_id: MultiplexLaneId::ROOT,
        mode: StreamMode::State,
        request: vec![1],
    })
    .unwrap();
    for (range, replacement) in [
        (0..4, b"OBLC".to_vec()),
        (4..6, 2u16.to_be_bytes().to_vec()),
        (6..8, 63u16.to_be_bytes().to_vec()),
        (8..12, u32::MAX.to_be_bytes().to_vec()),
        (12..20, vec![0; 8]),
        (20..28, vec![0; 8]),
        (28..29, vec![4]),
    ] {
        let mut invalid = open.clone();
        invalid[range].copy_from_slice(&replacement);
        assert_eq!(
            read_multiplex_message(&mut Cursor::new(invalid))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    // Header validation must reject a huge claim before attempting any payload read/allocation.
    let mut huge = open[..HEADER_BYTES].to_vec();
    huge[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        read_multiplex_message(&mut Cursor::new(huge))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}
