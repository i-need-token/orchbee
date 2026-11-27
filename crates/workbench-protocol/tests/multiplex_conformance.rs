//! The Rust and TypeScript OBWP stream codecs consume the same corpus, so they agree on the bytes.
use std::num::NonZeroU64;

use orchbee_workbench_protocol::{
    MultiplexCapabilities, MultiplexDecoder, MultiplexDisconnectReason, MultiplexLaneId,
    MultiplexMessage, MultiplexStreamId, MultiplexViewerId, StreamCloseReason, StreamFrameKind,
    StreamMode, encode_multiplex_message,
};
use serde::Deserialize;

/// The frozen generation 1 corpus: what a carrier admits when its handshake answer lists no
/// capability.
const CORPUS: &str =
    include_str!("../../../schemas/workbench/v1/workbench-multiplex-conformance-v1.json");
/// What a carrier whose answer lists `stream-outcome-unknown` admits besides the base corpus.
const OUTCOME_UNKNOWN_CORPUS: &str = include_str!(
    "../../../schemas/workbench/v1/workbench-multiplex-outcome-unknown-conformance-v1.json"
);
/// What a carrier whose answer lists `files` admits besides the base corpus: OBFB records where
/// they travel.
const FILES_CORPUS: &str =
    include_str!("../../../schemas/workbench/v1/workbench-multiplex-files-conformance-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    contract_version: u8,
    valid: Vec<ValidCase>,
    invalid: Vec<InvalidCase>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CapabilityCorpus {
    contract_version: u8,
    capability: String,
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
    ViewerRecordReady {
        viewer_id: String,
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
                "stopping" => StreamCloseReason::Stopping,
                "outcome-unknown" => StreamCloseReason::OutcomeUnknown,
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
        Message::ViewerRecordReady { viewer_id } => MultiplexMessage::ViewerRecordReady {
            viewer_id: MultiplexViewerId::new(viewer_id.parse().unwrap()).unwrap(),
        },
    }
}
fn decode(frame: &[u8]) -> std::io::Result<Option<MultiplexMessage>> {
    decode_with(MultiplexDecoder::default(), frame)
}
fn decode_with(
    mut decoder: MultiplexDecoder,
    frame: &[u8],
) -> std::io::Result<Option<MultiplexMessage>> {
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

/// A capability's values decode only where its answer admits them, and a decoder without it keeps
/// refusing them as the frozen corpus says.
fn decode_only_under_the_capability(
    corpus: &str,
    capability: &str,
    admitted: MultiplexCapabilities,
    without: MultiplexCapabilities,
) {
    let corpus: CapabilityCorpus = serde_json::from_str(corpus).unwrap();
    assert_eq!(corpus.contract_version, 1);
    assert_eq!(corpus.capability, capability);
    assert!(!corpus.valid.is_empty() && !corpus.invalid.is_empty());
    let admitted = || MultiplexDecoder::admitting(admitted);
    for case in &corpus.valid {
        let frame = bytes(&case.hex);
        let message = expected(&case.message);
        assert_eq!(
            decode_with(admitted(), &frame).unwrap().as_ref(),
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
        assert!(
            decode_with(MultiplexDecoder::admitting(without), &frame).is_err(),
            "{} without the capability",
            case.name
        );
    }
    for case in &corpus.invalid {
        assert!(
            decode_with(admitted(), &bytes(&case.hex)).is_err(),
            "{}",
            case.name
        );
    }
}

#[test]
fn outcome_unknown_closes_decode_only_under_their_capability() {
    decode_only_under_the_capability(
        OUTCOME_UNKNOWN_CORPUS,
        "stream-outcome-unknown",
        MultiplexCapabilities {
            stream_outcome_unknown: true,
            ..MultiplexCapabilities::default()
        },
        MultiplexCapabilities::default(),
    );
}

/// `files` requires `stream-outcome-unknown`, so its records are refused without `files` even
/// where the carrier admits that close.
#[test]
fn files_records_decode_only_under_their_capability() {
    let outcome_unknown = MultiplexCapabilities {
        stream_outcome_unknown: true,
        ..MultiplexCapabilities::default()
    };
    decode_only_under_the_capability(
        FILES_CORPUS,
        "files",
        MultiplexCapabilities {
            files: true,
            ..outcome_unknown
        },
        outcome_unknown,
    );
}
