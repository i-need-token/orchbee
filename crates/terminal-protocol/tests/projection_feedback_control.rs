use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    EncodedProjectionFeedbackControl, Envelope, FrameApplied, MAXIMUM_CONTROL_JSON_BYTES,
    MessageRole, NeedSnapshot, PayloadEncoding, ProjectionFeedbackControl, ProtocolError,
    ProtocolErrorCode, SnapshotReason, decode_projection_feedback_control,
    decode_transport_envelope, encode_projection_feedback_control, encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance.json");

#[test]
fn canonical_projection_feedback_decodes_and_reencodes_byte_for_byte() -> Result<(), Box<dyn Error>>
{
    let fixture = read_fixture()?;
    let frame_command = fixture_string(&fixture, "/commands/frame_applied")?;
    let frame_output = fixture_string(&fixture, "/outputs/frame_applied")?;
    assert_eq!(frame_command, frame_output);
    assert_canonical_round_trip(43, MessageRole::Request, frame_command)?;
    assert_canonical_round_trip(43, MessageRole::Response, frame_output)?;

    let command_reasons = fixture_strings(&fixture, "/commands/need_snapshot")?;
    let output_reasons = fixture_strings(&fixture, "/outputs/need_snapshot")?;
    assert_eq!(command_reasons, output_reasons);
    for payload in command_reasons {
        assert_canonical_round_trip(44, MessageRole::Request, payload)?;
        assert_canonical_round_trip(44, MessageRole::Response, payload)?;
    }
    Ok(())
}

#[test]
fn canonical_payloads_retain_complete_feedback_facts_and_direction() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let frame_payload = fixture_string(&fixture, "/commands/frame_applied")?;
    let frame =
        decode_projection_feedback_control(43, MessageRole::Request, frame_payload.as_bytes())?;
    let ProjectionFeedbackControl::FrameAppliedRequest(frame) = frame else {
        return Err(fixture_error("frame request decoded to the wrong variant").into());
    };
    assert_eq!(frame.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(frame.generation.get(), 3);
    assert_eq!(frame.revision, 42);
    assert_eq!(frame.sequence.get(), 7);

    let response =
        decode_projection_feedback_control(43, MessageRole::Response, frame_payload.as_bytes())?;
    assert_eq!(
        response,
        ProjectionFeedbackControl::FrameAppliedResponse(frame)
    );

    let reason_payloads = fixture_strings(&fixture, "/commands/need_snapshot")?;
    for (payload, expected_reason) in reason_payloads.into_iter().zip(snapshot_reasons()) {
        let request =
            decode_projection_feedback_control(44, MessageRole::Request, payload.as_bytes())?;
        let ProjectionFeedbackControl::NeedSnapshotRequest(request) = request else {
            return Err(fixture_error("snapshot request decoded to the wrong variant").into());
        };
        assert_eq!(request.attachment_id, frame.attachment_id);
        assert_eq!(request.reason, expected_reason);
        assert!(request.sequence.get() > 0);

        let response =
            decode_projection_feedback_control(44, MessageRole::Response, payload.as_bytes())?;
        assert_eq!(
            response,
            ProjectionFeedbackControl::NeedSnapshotResponse(request)
        );
    }
    Ok(())
}

#[test]
fn feedback_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_projection_feedback_control(43, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(
        43,
        MessageRole::Request,
        "{",
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_projection_feedback_control(43, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(
        43,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let frame = fixture_string(&fixture, "/commands/frame_applied")?;
    let unknown = frame.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        43,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = frame.replacen("\"sequence\":", "\"sequence\":\"1\",\"sequence\":", 1);
    assert_payload_code(
        43,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate =
        frame.replacen("\"sequence\":", "\"sequence\":\"1\",\"sequen\\u0063e\":", 1);
    assert_payload_code(
        43,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let need_snapshot = fixture_string_at(&fixture, "/commands/need_snapshot", 0)?;
    let mut missing: Value = serde_json::from_str(need_snapshot)?;
    object_mut(&mut missing)?.remove("reason");
    assert_value_code(
        44,
        MessageRole::Request,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn attachment_and_decimal_uint64_refinements_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let frame = fixture_string(&fixture, "/commands/frame_applied")?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(frame)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(
            43,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    for pointer in ["/generation", "/sequence"] {
        for invalid in ["0", "00", "-1", "+1", "18446744073709551616", " 1"] {
            let mut value: Value = serde_json::from_str(frame)?;
            set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
            assert_value_code(
                43,
                MessageRole::Request,
                value,
                ProtocolErrorCode::InvalidField,
            );
        }
    }
    for invalid in ["00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(frame)?;
        set_pointer(&mut value, "/revision", Value::String(invalid.to_owned()))?;
        assert_value_code(
            43,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }

    let maximum = u64::MAX.to_string();
    let mut boundary: Value = serde_json::from_str(frame)?;
    for pointer in ["/generation", "/revision", "/sequence"] {
        set_pointer(&mut boundary, pointer, Value::String(maximum.clone()))?;
    }
    let decoded = decode_value(43, MessageRole::Request, boundary)?;
    assert!(encode_projection_feedback_control(&decoded).is_ok());

    let mut zero_revision: Value = serde_json::from_str(frame)?;
    set_pointer(
        &mut zero_revision,
        "/revision",
        Value::String("0".to_owned()),
    )?;
    assert!(decode_value(43, MessageRole::Request, zero_revision).is_ok());

    let mut numeric: Value = serde_json::from_str(frame)?;
    set_pointer(&mut numeric, "/revision", Value::from(42))?;
    assert_value_code(
        43,
        MessageRole::Request,
        numeric,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn snapshot_reason_vocabulary_is_closed_and_complete() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let payloads = fixture_strings(&fixture, "/commands/need_snapshot")?;
    assert_eq!(payloads.len(), snapshot_reasons().len());

    let baseline = payloads
        .first()
        .ok_or_else(|| fixture_error("snapshot fixture array is empty"))?;
    for invalid in ["", "activity_stale", "future-reason", "STORE-RESET"] {
        let mut value: Value = serde_json::from_str(baseline)?;
        set_pointer(&mut value, "/reason", Value::String(invalid.to_owned()))?;
        assert_value_code(
            44,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn public_feedback_values_encode_every_closed_variant() -> Result<(), Box<dyn Error>> {
    let attachment_id = orchbee_terminal_protocol::AttachmentId::parse(&"22".repeat(16))?;
    let maximum = NonZeroU64::new(u64::MAX)
        .ok_or_else(|| fixture_error("maximum unsigned integer is unexpectedly zero"))?;
    let frame = FrameApplied {
        attachment_id,
        generation: maximum,
        revision: u64::MAX,
        sequence: maximum,
    };
    for value in [
        ProjectionFeedbackControl::FrameAppliedRequest(frame),
        ProjectionFeedbackControl::FrameAppliedResponse(frame),
    ] {
        let encoded = encode_projection_feedback_control(&value)?;
        assert_eq!(
            decode_projection_feedback_control(encoded.kind, encoded.role, &encoded.payload)?,
            value
        );
    }

    for reason in snapshot_reasons() {
        let need = NeedSnapshot {
            attachment_id,
            reason,
            sequence: maximum,
        };
        for value in [
            ProjectionFeedbackControl::NeedSnapshotRequest(need),
            ProjectionFeedbackControl::NeedSnapshotResponse(need),
        ] {
            let encoded = encode_projection_feedback_control(&value)?;
            assert_eq!(
                decode_projection_feedback_control(encoded.kind, encoded.role, &encoded.payload)?,
                value
            );
        }
    }
    Ok(())
}

#[test]
fn feedback_kind_role_and_control_json_boundaries_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let frame = fixture_string(&fixture, "/commands/frame_applied")?;
    for (kind, role) in [
        (42, MessageRole::Event),
        (43, MessageRole::Event),
        (44, MessageRole::Event),
        (45, MessageRole::Request),
    ] {
        assert_payload_code(kind, role, frame, ProtocolErrorCode::InvalidField);
    }

    let prefix_length = MAXIMUM_CONTROL_JSON_BYTES
        .checked_sub(frame.len())
        .ok_or_else(|| fixture_error("canonical feedback fixture exceeds the control bound"))?;
    let mut exact = vec![b' '; prefix_length];
    exact.extend_from_slice(frame.as_bytes());
    assert!(decode_projection_feedback_control(43, MessageRole::Request, &exact).is_ok());
    exact.insert(0, b' ');
    assert_code(
        decode_projection_feedback_control(43, MessageRole::Request, &exact),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn projection_feedback_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, payload) in [
        (
            43,
            MessageRole::Request,
            fixture_string(&fixture, "/commands/frame_applied")?,
        ),
        (
            43,
            MessageRole::Response,
            fixture_string(&fixture, "/outputs/frame_applied")?,
        ),
        (
            44,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/need_snapshot", 0)?,
        ),
        (
            44,
            MessageRole::Response,
            fixture_string_at(&fixture, "/outputs/need_snapshot", 7)?,
        ),
    ] {
        let value = decode_projection_feedback_control(kind, role, payload.as_bytes())?;
        let EncodedProjectionFeedbackControl {
            kind,
            payload,
            role,
        } = encode_projection_feedback_control(&value)?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: None,
            kind,
            optional_message: false,
            payload: &payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
            role,
        })?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(envelope.payload_encoding, PayloadEncoding::ControlJson);
        assert_eq!(
            decode_projection_feedback_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn snapshot_reasons() -> [SnapshotReason; 8] {
    [
        SnapshotReason::ActivityStale,
        SnapshotReason::BaseMismatch,
        SnapshotReason::DecoderRejected,
        SnapshotReason::DimensionsMismatch,
        SnapshotReason::GenerationMismatch,
        SnapshotReason::IdentityMismatch,
        SnapshotReason::SequenceGap,
        SnapshotReason::StoreReset,
    ]
}

fn assert_canonical_round_trip(
    kind: u16,
    role: MessageRole,
    payload: &str,
) -> Result<(), Box<dyn Error>> {
    let decoded = decode_projection_feedback_control(kind, role, payload.as_bytes())?;
    let encoded = encode_projection_feedback_control(&decoded)?;
    assert_eq!(encoded.kind, kind);
    assert_eq!(encoded.role, role);
    assert_eq!(encoded.payload, payload.as_bytes());
    Ok(())
}

fn read_fixture() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONTROL_FIXTURE)
}

fn fixture_string<'fixture>(
    fixture: &'fixture Value,
    pointer: &str,
) -> Result<&'fixture str, io::Error> {
    fixture
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| fixture_error("fixture pointer is not a string"))
}

fn fixture_strings<'fixture>(
    fixture: &'fixture Value,
    pointer: &str,
) -> Result<Vec<&'fixture str>, io::Error> {
    fixture
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| fixture_error("fixture pointer is not an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| fixture_error("fixture array value is not a string"))
        })
        .collect()
}

fn fixture_string_at<'fixture>(
    fixture: &'fixture Value,
    pointer: &str,
    index: usize,
) -> Result<&'fixture str, io::Error> {
    fixture_strings(fixture, pointer)?
        .get(index)
        .copied()
        .ok_or_else(|| fixture_error("fixture array index is absent"))
}

fn decode_value(
    kind: u16,
    role: MessageRole,
    value: Value,
) -> Result<ProjectionFeedbackControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_projection_feedback_control(kind, role, &payload)
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_projection_feedback_control(kind, role, payload.as_bytes()),
        expected,
    );
}

fn assert_code<T>(result: Result<T, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Err(error) => assert_eq!(error.code(), expected),
        Ok(_) => panic!("payload unexpectedly succeeded"),
    }
}

fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| fixture_error("test JSON pointer is absent"))?;
    *target = replacement;
    Ok(())
}

fn object_mut(value: &mut Value) -> Result<&mut serde_json::Map<String, Value>, io::Error> {
    value
        .as_object_mut()
        .ok_or_else(|| fixture_error("test JSON value is not an object"))
}

fn fixture_error(detail: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, detail)
}
