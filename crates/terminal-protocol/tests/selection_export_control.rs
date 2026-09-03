use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    AttachmentId, CorrelationId, EncodedSelectionExportControl, Envelope, HostEpoch,
    MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_SELECTION_EXPORT_BYTES, MessageRole, PayloadEncoding,
    ProtocolError, ProtocolErrorCode, SelectionExport, SelectionExportCancel,
    SelectionExportCancelReason, SelectionExportChunkApplied, SelectionExportControl,
    SelectionExportFinished, SelectionExportFormat, SelectionExportStarted, SelectionExportStatus,
    decode_selection_export_control, decode_transport_envelope, encode_selection_export_control,
    encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.23.json");

#[test]
fn canonical_export_controls_round_trip_in_every_legal_direction() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for payload in fixture_strings(&fixture, "/commands/selection_export")? {
        assert_canonical_round_trip(63, MessageRole::Request, payload)?;
    }
    for payload in fixture_strings(&fixture, "/outputs/selection_export_started")? {
        assert_canonical_round_trip(64, MessageRole::Response, payload)?;
    }

    let applied_command = fixture_string(&fixture, "/commands/selection_export_chunk_applied")?;
    let applied_output = fixture_string(&fixture, "/outputs/selection_export_chunk_applied")?;
    assert_eq!(applied_command, applied_output);
    assert_canonical_round_trip(66, MessageRole::Request, applied_command)?;
    assert_canonical_round_trip(66, MessageRole::Response, applied_output)?;

    for payload in fixture_strings(&fixture, "/outputs/selection_export_finished")? {
        assert_canonical_round_trip(67, MessageRole::Response, payload)?;
        assert_canonical_round_trip(67, MessageRole::Event, payload)?;
    }
    for payload in fixture_strings(&fixture, "/commands/selection_export_cancel")? {
        assert_canonical_round_trip(68, MessageRole::Request, payload)?;
    }
    Ok(())
}

#[test]
fn canonical_export_controls_retain_complete_facts_and_direction() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let export = fixture_string_at(&fixture, "/commands/selection_export", 0)?;
    let SelectionExportControl::ExportRequest(export) =
        decode_selection_export_control(63, MessageRole::Request, export.as_bytes())?
    else {
        return Err(fixture_error("export request decoded to the wrong variant").into());
    };
    assert_eq!(export.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(export.export_sequence.get(), 1);
    assert_eq!(export.format, SelectionExportFormat::Plain);

    let started = fixture_string_at(&fixture, "/outputs/selection_export_started", 0)?;
    let SelectionExportControl::StartedResponse(started) =
        decode_selection_export_control(64, MessageRole::Response, started.as_bytes())?
    else {
        return Err(fixture_error("started response decoded to the wrong variant").into());
    };
    assert_eq!(started.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(started.byte_length, 10);
    assert_eq!(started.export_sequence.get(), 1);
    assert_eq!(started.format, SelectionExportFormat::Plain);

    let applied = fixture_string(&fixture, "/commands/selection_export_chunk_applied")?;
    let SelectionExportControl::ChunkAppliedRequest(applied_value) =
        decode_selection_export_control(66, MessageRole::Request, applied.as_bytes())?
    else {
        return Err(fixture_error("chunk acknowledgement decoded to the wrong variant").into());
    };
    assert_eq!(applied_value.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(applied_value.chunk_sequence, 2);
    assert_eq!(applied_value.export_sequence.get(), 9);
    assert_eq!(
        decode_selection_export_control(66, MessageRole::Response, applied.as_bytes())?,
        SelectionExportControl::ChunkAppliedResponse(applied_value)
    );

    for (payload, status) in fixture_strings(&fixture, "/outputs/selection_export_finished")?
        .into_iter()
        .zip([
            SelectionExportStatus::Cancelled,
            SelectionExportStatus::Completed,
        ])
    {
        let SelectionExportControl::FinishedResponse(finished) =
            decode_selection_export_control(67, MessageRole::Response, payload.as_bytes())?
        else {
            return Err(fixture_error("finished response decoded to the wrong variant").into());
        };
        assert_eq!(finished.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(finished.byte_length, 12);
        assert_eq!(finished.export_sequence.get(), 9);
        assert_eq!(finished.status, status);
        assert_eq!(
            decode_selection_export_control(67, MessageRole::Event, payload.as_bytes())?,
            SelectionExportControl::FinishedEvent(finished)
        );
    }

    for (payload, reason) in fixture_strings(&fixture, "/commands/selection_export_cancel")?
        .into_iter()
        .zip([
            SelectionExportCancelReason::ClipboardFailed,
            SelectionExportCancelReason::Disposed,
            SelectionExportCancelReason::Replaced,
            SelectionExportCancelReason::User,
        ])
    {
        let SelectionExportControl::CancelRequest(cancel) =
            decode_selection_export_control(68, MessageRole::Request, payload.as_bytes())?
        else {
            return Err(fixture_error("cancel request decoded to the wrong variant").into());
        };
        assert_eq!(cancel.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(cancel.reason, reason);
    }
    Ok(())
}

#[test]
fn export_control_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_selection_export_control(63, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(
        63,
        MessageRole::Request,
        "{",
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_selection_export_control(63, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(
        63,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let export = fixture_string_at(&fixture, "/commands/selection_export", 0)?;
    let unknown = export.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        63,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = export.replacen(
        "\"exportSequence\":",
        "\"exportSequence\":\"2\",\"exportSequence\":",
        1,
    );
    assert_payload_code(
        63,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate =
        export.replacen("\"format\":", "\"format\":\"plain\",\"form\\u0061t\":", 1);
    assert_payload_code(
        63,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let mut missing: Value = serde_json::from_str(export)?;
    object_mut(&mut missing)?.remove("format");
    assert_value_code(
        63,
        MessageRole::Request,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn export_identity_and_positive_decimal_sequence_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let export = fixture_string_at(&fixture, "/commands/selection_export", 0)?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(export)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(
            63,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }
    for invalid in ["0", "00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(export)?;
        set_pointer(
            &mut value,
            "/exportSequence",
            Value::String(invalid.to_owned()),
        )?;
        assert_value_code(
            63,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    let mut maximum: Value = serde_json::from_str(export)?;
    set_pointer(
        &mut maximum,
        "/exportSequence",
        Value::String(u64::MAX.to_string()),
    )?;
    assert!(decode_value(63, MessageRole::Request, maximum).is_ok());

    let mut numeric: Value = serde_json::from_str(export)?;
    set_pointer(&mut numeric, "/exportSequence", Value::from(1))?;
    assert_value_code(
        63,
        MessageRole::Request,
        numeric,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn export_numeric_fields_normalize_safely_and_obey_exact_bounds() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let started = fixture_string_at(&fixture, "/outputs/selection_export_started", 0)?;
    assert_numeric_partition(
        64,
        MessageRole::Response,
        started,
        "/byteLength",
        &[0, MAXIMUM_SELECTION_EXPORT_BYTES as u64],
        &[MAXIMUM_SELECTION_EXPORT_BYTES as u64 + 1],
    )?;
    let mut negative: Value = serde_json::from_str(started)?;
    set_pointer(&mut negative, "/byteLength", Value::from(-1))?;
    assert_value_code(
        64,
        MessageRole::Response,
        negative,
        ProtocolErrorCode::InvalidField,
    );

    let applied = fixture_string(&fixture, "/commands/selection_export_chunk_applied")?;
    assert_numeric_partition(
        66,
        MessageRole::Request,
        applied,
        "/chunkSequence",
        &[0, u64::from(u32::MAX)],
        &[u64::from(u32::MAX) + 1],
    )?;
    let mut negative: Value = serde_json::from_str(applied)?;
    set_pointer(&mut negative, "/chunkSequence", Value::from(-1))?;
    assert_value_code(
        66,
        MessageRole::Request,
        negative,
        ProtocolErrorCode::InvalidField,
    );

    for (kind, role, payload, member) in [
        (64, MessageRole::Response, started, "byteLength"),
        (66, MessageRole::Request, applied, "chunkSequence"),
    ] {
        for invalid in ["1.5", "9007199254740992"] {
            let candidate = replace_numeric_member(payload, member, invalid)?;
            assert_payload_code(kind, role, &candidate, ProtocolErrorCode::InvalidField);
        }
        for normalized in ["1.0", "1e0"] {
            let candidate = replace_numeric_member(payload, member, normalized)?;
            assert!(decode_selection_export_control(kind, role, candidate.as_bytes()).is_ok());
        }
    }
    Ok(())
}

#[test]
fn export_format_status_and_cancel_reason_vocabularies_are_closed() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, payload, pointer) in [
        (
            63,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/selection_export", 0)?,
            "/format",
        ),
        (
            64,
            MessageRole::Response,
            fixture_string_at(&fixture, "/outputs/selection_export_started", 0)?,
            "/format",
        ),
        (
            67,
            MessageRole::Event,
            fixture_string_at(&fixture, "/outputs/selection_export_finished", 0)?,
            "/status",
        ),
        (
            68,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/selection_export_cancel", 0)?,
            "/reason",
        ),
    ] {
        for invalid in ["", "PLAIN", "plain ", "future"] {
            let mut value: Value = serde_json::from_str(payload)?;
            set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
            assert_value_code(kind, role, value, ProtocolErrorCode::InvalidField);
        }
    }
    Ok(())
}

#[test]
fn public_export_controls_encode_every_direction_and_revalidate_byte_length()
-> Result<(), Box<dyn Error>> {
    let attachment_id = example_attachment()?;
    let export_sequence = nonzero(u64::MAX)?;
    let export = SelectionExport {
        attachment_id,
        export_sequence,
        format: SelectionExportFormat::Plain,
    };
    assert_public_round_trip(SelectionExportControl::ExportRequest(export))?;
    assert_public_round_trip(SelectionExportControl::StartedResponse(
        SelectionExportStarted {
            attachment_id,
            byte_length: MAXIMUM_SELECTION_EXPORT_BYTES as u32,
            export_sequence,
            format: SelectionExportFormat::Plain,
        },
    ))?;

    let applied = SelectionExportChunkApplied {
        attachment_id,
        chunk_sequence: u32::MAX,
        export_sequence,
    };
    assert_public_round_trip(SelectionExportControl::ChunkAppliedRequest(applied))?;
    assert_public_round_trip(SelectionExportControl::ChunkAppliedResponse(applied))?;

    for status in [
        SelectionExportStatus::Cancelled,
        SelectionExportStatus::Completed,
    ] {
        let finished = SelectionExportFinished {
            attachment_id,
            byte_length: 0,
            export_sequence,
            status,
        };
        assert_public_round_trip(SelectionExportControl::FinishedResponse(finished))?;
        assert_public_round_trip(SelectionExportControl::FinishedEvent(finished))?;
    }
    for reason in [
        SelectionExportCancelReason::ClipboardFailed,
        SelectionExportCancelReason::Disposed,
        SelectionExportCancelReason::Replaced,
        SelectionExportCancelReason::User,
    ] {
        assert_public_round_trip(SelectionExportControl::CancelRequest(
            SelectionExportCancel {
                attachment_id,
                export_sequence,
                reason,
            },
        ))?;
    }

    assert_code(
        encode_selection_export_control(&SelectionExportControl::StartedResponse(
            SelectionExportStarted {
                attachment_id,
                byte_length: MAXIMUM_SELECTION_EXPORT_BYTES as u32 + 1,
                export_sequence,
                format: SelectionExportFormat::Plain,
            },
        )),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn export_control_kind_and_role_selection_is_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let cases = [
        (
            63,
            fixture_string_at(&fixture, "/commands/selection_export", 0)?,
            MessageRole::Request,
        ),
        (
            64,
            fixture_string_at(&fixture, "/outputs/selection_export_started", 0)?,
            MessageRole::Response,
        ),
        (
            66,
            fixture_string(&fixture, "/commands/selection_export_chunk_applied")?,
            MessageRole::Request,
        ),
        (
            67,
            fixture_string_at(&fixture, "/outputs/selection_export_finished", 0)?,
            MessageRole::Event,
        ),
        (
            68,
            fixture_string_at(&fixture, "/commands/selection_export_cancel", 0)?,
            MessageRole::Request,
        ),
    ];
    for (kind, payload, legal_role) in cases {
        for role in [
            MessageRole::Request,
            MessageRole::Response,
            MessageRole::Event,
        ] {
            if (kind == 66 && matches!(role, MessageRole::Request | MessageRole::Response))
                || (kind == 67 && matches!(role, MessageRole::Response | MessageRole::Event))
                || role == legal_role
            {
                assert!(decode_selection_export_control(kind, role, payload.as_bytes()).is_ok());
            } else {
                assert_payload_code(kind, role, payload, ProtocolErrorCode::InvalidField);
            }
        }
    }
    assert_payload_code(
        65,
        MessageRole::Event,
        "{}",
        ProtocolErrorCode::InvalidField,
    );
    assert_payload_code(
        69,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn export_control_json_aggregate_boundary_is_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let export = fixture_string_at(&fixture, "/commands/selection_export", 0)?;
    let exact = format!(
        "{export}{}",
        " ".repeat(MAXIMUM_CONTROL_JSON_BYTES - export.len())
    );
    assert_eq!(exact.len(), MAXIMUM_CONTROL_JSON_BYTES);
    assert!(decode_selection_export_control(63, MessageRole::Request, exact.as_bytes()).is_ok());
    let over = format!("{exact} ");
    assert_code(
        decode_selection_export_control(63, MessageRole::Request, over.as_bytes()),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn export_controls_compose_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let values = [
        decode_selection_export_control(
            63,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/selection_export", 0)?.as_bytes(),
        )?,
        decode_selection_export_control(
            64,
            MessageRole::Response,
            fixture_string_at(&fixture, "/outputs/selection_export_started", 0)?.as_bytes(),
        )?,
        decode_selection_export_control(
            66,
            MessageRole::Request,
            fixture_string(&fixture, "/commands/selection_export_chunk_applied")?.as_bytes(),
        )?,
        decode_selection_export_control(
            66,
            MessageRole::Response,
            fixture_string(&fixture, "/outputs/selection_export_chunk_applied")?.as_bytes(),
        )?,
        decode_selection_export_control(
            67,
            MessageRole::Response,
            fixture_string_at(&fixture, "/outputs/selection_export_finished", 1)?.as_bytes(),
        )?,
        decode_selection_export_control(
            67,
            MessageRole::Event,
            fixture_string_at(&fixture, "/outputs/selection_export_finished", 0)?.as_bytes(),
        )?,
        decode_selection_export_control(
            68,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/selection_export_cancel", 3)?.as_bytes(),
        )?,
    ];

    for value in values {
        let encoded = encode_selection_export_control(&value)?;
        let correlation_id = if encoded.role == MessageRole::Event {
            None
        } else {
            Some(CorrelationId::parse(&"44".repeat(16))?)
        };
        let record = encode_transport_envelope(&Envelope {
            correlation_id,
            host_epoch: Some(HostEpoch::parse(&"11".repeat(16))?),
            kind: encoded.kind,
            optional_message: false,
            payload: &encoded.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
            role: encoded.role,
        })?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(
            decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn assert_numeric_partition(
    kind: u16,
    role: MessageRole,
    payload: &str,
    pointer: &str,
    accepted: &[u64],
    rejected: &[u64],
) -> Result<(), Box<dyn Error>> {
    for value in accepted {
        let mut candidate: Value = serde_json::from_str(payload)?;
        set_pointer(&mut candidate, pointer, Value::from(*value))?;
        assert!(decode_value(kind, role, candidate).is_ok());
    }
    for value in rejected {
        let mut candidate: Value = serde_json::from_str(payload)?;
        set_pointer(&mut candidate, pointer, Value::from(*value))?;
        assert_value_code(kind, role, candidate, ProtocolErrorCode::InvalidField);
    }
    Ok(())
}

fn replace_numeric_member(
    payload: &str,
    member: &str,
    replacement: &str,
) -> Result<String, io::Error> {
    let marker = format!("\"{member}\":");
    let start = payload
        .find(&marker)
        .ok_or_else(|| fixture_error("numeric member is absent"))?
        + marker.len();
    let end = payload[start..]
        .find([',', '}'])
        .map(|offset| start + offset)
        .ok_or_else(|| fixture_error("numeric member has no terminator"))?;
    let mut result = payload.to_owned();
    result.replace_range(start..end, replacement);
    Ok(result)
}

fn example_attachment() -> Result<AttachmentId, orchbee_terminal_protocol::IdentityError> {
    AttachmentId::parse(&"22".repeat(16))
}

fn nonzero(value: u64) -> Result<NonZeroU64, io::Error> {
    NonZeroU64::new(value).ok_or_else(|| fixture_error("expected a nonzero test value"))
}

fn assert_public_round_trip(value: SelectionExportControl) -> Result<(), ProtocolError> {
    let encoded = encode_selection_export_control(&value)?;
    assert_eq!(
        decode_selection_export_control(encoded.kind, encoded.role, &encoded.payload)?,
        value
    );
    Ok(())
}

fn assert_canonical_round_trip(
    kind: u16,
    role: MessageRole,
    payload: &str,
) -> Result<(), Box<dyn Error>> {
    let decoded = decode_selection_export_control(kind, role, payload.as_bytes())?;
    let encoded: EncodedSelectionExportControl = encode_selection_export_control(&decoded)?;
    assert_eq!(encoded.kind, kind);
    assert_eq!(encoded.role, role);
    assert_eq!(encoded.payload, payload.as_bytes());
    Ok(())
}

fn read_fixture() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONTROL_FIXTURE)
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

fn fixture_string<'fixture>(
    fixture: &'fixture Value,
    pointer: &str,
) -> Result<&'fixture str, io::Error> {
    fixture
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| fixture_error("fixture pointer is not a string"))
}

fn decode_value(
    kind: u16,
    role: MessageRole,
    value: Value,
) -> Result<SelectionExportControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_selection_export_control(kind, role, &payload)
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_selection_export_control(kind, role, payload.as_bytes()),
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
