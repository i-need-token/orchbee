//! The Rust and TypeScript OBWP stream codecs consume the same corpus, so they agree on the bytes.
use std::num::NonZeroU64;

use orchbee_workbench_protocol::{
    MultiplexDecoder, MultiplexDisconnectReason, MultiplexLaneId, MultiplexMessage,
    MultiplexStreamId, StreamCloseReason, StreamFrameKind, StreamMode, encode_multiplex_message,
};
use serde::Deserialize;

const CORPUS: &str =
    include_str!("../../../schemas/workbench/v1/workbench-multiplex-conformance-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    contract_version: u8,
    valid: Vec<ValidCase>,
    invalid: Vec<InvalidCase>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidCase {
    name: String,
    hex: String,
    message: Message,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidCase {
    name: String,
    hex: String,
}
#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Message {
    StreamOpen {
        stream_id: String,
        lane_id: String,
        mode: String,
        request: String,
    },
    StreamFrame {
        stream_id: String,
        sequence: String,
        frame_kind: String,
        record: String,
    },
    StreamClose {
        stream_id: String,
        reason: String,
    },
    Gap {
        stream_id: String,
        through: String,
    },
    Disconnect {
        reason: String,
    },
    Resnapshot {
        stream_id: String,
    },
    LaneOpen {
        lane_id: String,
    },
    LaneClose {
        lane_id: String,
    },
}

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}
fn stream(text: &str) -> MultiplexStreamId {
    MultiplexStreamId::new(text.parse().unwrap()).unwrap()
}
fn lane(text: &str) -> MultiplexLaneId {
    MultiplexLaneId::new(text.parse().unwrap()).unwrap()
}
fn positive(text: &str) -> NonZeroU64 {
    text.parse().unwrap()
}
fn expected(message: &Message) -> MultiplexMessage {
    match message {
        Message::StreamOpen {
            stream_id,
            lane_id,
            mode,
            request,
        } => MultiplexMessage::StreamOpen {
            stream_id: stream(stream_id),
            lane_id: lane(lane_id),
            mode: match mode.as_str() {
                "request" => StreamMode::Request,
                "state" => StreamMode::State,
                "events" => StreamMode::Events,
                "invalidations" => StreamMode::Invalidations,
                other => panic!("unknown mode {other}"),
            },
            request: bytes(request),
        },
        Message::StreamFrame {
            stream_id,
            sequence,
            frame_kind,
            record,
        } => MultiplexMessage::StreamFrame {
            stream_id: stream(stream_id),
            sequence: positive(sequence),
            kind: match frame_kind.as_str() {
                "result" => StreamFrameKind::Result,
                "snapshot" => StreamFrameKind::Snapshot,
                "update" => StreamFrameKind::Update,
                other => panic!("unknown frame kind {other}"),
            },
            record: bytes(record),
        },
        Message::StreamClose { stream_id, reason } => MultiplexMessage::StreamClose {
            stream_id: stream(stream_id),
            reason: match reason.as_str() {
                "complete" => StreamCloseReason::Complete,
                "cancelled" => StreamCloseReason::Cancelled,
                "unsupported" => StreamCloseReason::Unsupported,
                "capacity" => StreamCloseReason::Capacity,
                "invalid-request" => StreamCloseReason::InvalidRequest,
                other => panic!("unknown close reason {other}"),
            },
        },
        Message::Gap { stream_id, through } => MultiplexMessage::Gap {
            stream_id: stream(stream_id),
            through: positive(through),
        },
        Message::Disconnect { reason } => MultiplexMessage::Disconnect {
            reason: match reason.as_str() {
                "slow-consumer" => MultiplexDisconnectReason::SlowConsumer,
                "capacity" => MultiplexDisconnectReason::Capacity,
                "invalid-stream" => MultiplexDisconnectReason::InvalidStream,
                "unavailable" => MultiplexDisconnectReason::Unavailable,
                other => panic!("unknown disconnect reason {other}"),
            },
        },
        Message::Resnapshot { stream_id } => MultiplexMessage::Resnapshot {
            stream_id: stream(stream_id),
        },
        Message::LaneOpen { lane_id } => MultiplexMessage::LaneOpen {
            lane_id: lane(lane_id),
        },
        Message::LaneClose { lane_id } => MultiplexMessage::LaneClose {
            lane_id: lane(lane_id),
        },
    }
}
fn decode(frame: &[u8]) -> std::io::Result<Option<MultiplexMessage>> {
    let mut decoder = MultiplexDecoder::default();
    decoder.push(frame)?;
    decoder.next_message()
}

#[test]
fn stream_frames_match_the_shared_corpus() {
    let corpus: Corpus = serde_json::from_str(CORPUS).unwrap();
    assert_eq!(corpus.contract_version, 1);
    assert!(!corpus.valid.is_empty() && !corpus.invalid.is_empty());
    for case in &corpus.valid {
        let frame = bytes(&case.hex);
        let message = expected(&case.message);
        assert_eq!(
            decode(&frame).unwrap().as_ref(),
            Some(&message),
            "{}",
            case.name
        );
        assert_eq!(
            encode_multiplex_message(&message).unwrap(),
            frame,
            "{}",
            case.name
        );
    }
    for case in &corpus.invalid {
        assert!(decode(&bytes(&case.hex)).is_err(), "{}", case.name);
    }
}
