use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    EncodedRowAccessControl, Envelope, MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_ROW_WINDOW_BYTES,
    MAXIMUM_ROW_WINDOW_ROWS, MessageRole, PayloadEncoding, ProtocolError, ProtocolErrorCode,
    ReadRows, RowAccessControl, RowRevisionPolicy, RowWindowDetail, RowWindowOrigin,
    RowWindowPurpose, RowsApplied, decode_row_access_control, decode_transport_envelope,
    encode_row_access_control, encode_transport_envelope,
};
use serde_json::{Value, json};

const CONTROL_FIXTURE: &str =
    include_str!("../../../schemas/terminal/v1/fixtures/control-conformance.json");

#[test]
fn canonical_row_access_payloads_round_trip_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for payload in fixture_strings(&fixture, "/commands/read_rows")? {
        assert_canonical_round_trip(45, MessageRole::Request, payload)?;
    }

    let applied = fixture_string(&fixture, "/commands/rows_applied")?;
    assert_canonical_round_trip(50, MessageRole::Request, applied)?;
    assert_canonical_round_trip(50, MessageRole::Response, applied)?;
    Ok(())
}

#[test]
fn canonical_payloads_retain_every_origin_fact_and_acknowledgement_direction()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let reads = fixture_strings(&fixture, "/commands/read_rows")?;
    let expected = [
        (
            RowWindowDetail::Text,
            RowWindowOrigin::Absolute { row: 10 },
            RowWindowPurpose::Buffer,
            RowRevisionPolicy::Exact,
        ),
        (
            RowWindowDetail::Render,
            RowWindowOrigin::Oldest { offset: 2 },
            RowWindowPurpose::Search,
            RowRevisionPolicy::Exact,
        ),
        (
            RowWindowDetail::Text,
            RowWindowOrigin::Committed { delta: -1 },
            RowWindowPurpose::Viewport,
            RowRevisionPolicy::CurrentView,
        ),
        (
            RowWindowDetail::Render,
            RowWindowOrigin::Tail { rows_before: 3 },
            RowWindowPurpose::Buffer,
            RowRevisionPolicy::Exact,
        ),
    ];

    for ((payload, (detail, origin, purpose, revision_policy)), expected_sequence) in
        reads.into_iter().zip(expected).zip(1_u64..=4)
    {
        let decoded = decode_row_access_control(45, MessageRole::Request, payload.as_bytes())?;
        let RowAccessControl::ReadRows(read) = decoded else {
            return Err(fixture_error("row read decoded to the wrong variant").into());
        };
        assert_eq!(read.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(read.detail, detail);
        assert_eq!(read.generation.get(), 3);
        assert_eq!(read.maximum_bytes, 65_536);
        assert_eq!(read.origin, origin);
        assert_eq!(read.purpose, purpose);
        assert_eq!(read.request_sequence.get(), expected_sequence);
        assert_eq!(read.revision, 42);
        assert_eq!(read.revision_policy, revision_policy);
        assert_eq!(read.row_count, 20);
    }

    let applied = fixture_string(&fixture, "/commands/rows_applied")?;
    let request = decode_row_access_control(50, MessageRole::Request, applied.as_bytes())?;
    let RowAccessControl::RowsAppliedRequest(applied_request) = request else {
        return Err(fixture_error("rows-applied decoded to the wrong request variant").into());
    };
    assert_eq!(applied_request.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(applied_request.generation.get(), 3);
    assert_eq!(applied_request.request_sequence.get(), 8);
    assert_eq!(applied_request.revision, 42);
    assert_eq!(
        decode_row_access_control(50, MessageRole::Response, applied.as_bytes())?,
        RowAccessControl::RowsAppliedResponse(applied_request)
    );
    Ok(())
}

#[test]
fn row_access_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_row_access_control(45, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(
        45,
        MessageRole::Request,
        "{",
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_row_access_control(45, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(
        45,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let read = fixture_string_at(&fixture, "/commands/read_rows", 0)?;
    let unknown = read.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        45,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = read.replacen("\"purpose\":", "\"purpose\":\"search\",\"purpose\":", 1);
    assert_payload_code(
        45,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = read.replacen(
        "\"purpose\":",
        "\"purpose\":\"search\",\"purp\\u006fse\":",
        1,
    );
    assert_payload_code(
        45,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let mut unknown_origin: Value = serde_json::from_str(read)?;
    object_at_mut(&mut unknown_origin, "/origin")?.insert("extra".to_owned(), Value::Bool(true));
    assert_value_code(
        45,
        MessageRole::Request,
        unknown_origin,
        ProtocolErrorCode::InvalidField,
    );

    let nested_duplicate = read.replacen("\"row\":\"10\"", "\"row\":\"9\",\"r\\u006fw\":\"10\"", 1);
    assert_payload_code(
        45,
        MessageRole::Request,
        &nested_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let mut missing: Value = serde_json::from_str(read)?;
    object_at_mut(&mut missing, "/origin")?.remove("row");
    assert_value_code(
        45,
        MessageRole::Request,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn row_access_identity_and_decimal_uint64_refinements_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let read = fixture_string_at(&fixture, "/commands/read_rows", 0)?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(read)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(
            45,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    for pointer in ["/generation", "/requestSequence"] {
        for invalid in ["0", "00", "-1", "+1", "18446744073709551616", " 1"] {
            let mut value: Value = serde_json::from_str(read)?;
            set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
            assert_value_code(
                45,
                MessageRole::Request,
                value,
                ProtocolErrorCode::InvalidField,
            );
        }
    }

    for pointer in ["/revision", "/origin/row"] {
        for invalid in ["00", "-1", "+1", "18446744073709551616", " 1"] {
            let mut value: Value = serde_json::from_str(read)?;
            set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
            assert_value_code(
                45,
                MessageRole::Request,
                value,
                ProtocolErrorCode::InvalidField,
            );
        }
    }

    let maximum = u64::MAX.to_string();
    let mut boundary: Value = serde_json::from_str(read)?;
    for pointer in [
        "/generation",
        "/requestSequence",
        "/revision",
        "/origin/row",
    ] {
        set_pointer(&mut boundary, pointer, Value::String(maximum.clone()))?;
    }
    assert!(decode_value(45, MessageRole::Request, boundary).is_ok());

    let mut zeroes: Value = serde_json::from_str(read)?;
    for pointer in ["/revision", "/origin/row"] {
        set_pointer(&mut zeroes, pointer, Value::String("0".to_owned()))?;
    }
    assert!(decode_value(45, MessageRole::Request, zeroes).is_ok());

    let mut numeric: Value = serde_json::from_str(read)?;
    set_pointer(&mut numeric, "/revision", Value::from(42))?;
    assert_value_code(
        45,
        MessageRole::Request,
        numeric,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn row_access_vocabularies_are_closed() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let read = fixture_string_at(&fixture, "/commands/read_rows", 0)?;
    for (pointer, invalid_values) in [
        ("/detail", ["", "Render", "plain", "future"]),
        ("/purpose", ["", "VIEWPORT", "history", "future"]),
        ("/revisionPolicy", ["", "current_view", "latest", "future"]),
        ("/origin/kind", ["", "Absolute", "relative", "future"]),
    ] {
        for invalid in invalid_values {
            let mut value: Value = serde_json::from_str(read)?;
            set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
            assert_value_code(
                45,
                MessageRole::Request,
                value,
                ProtocolErrorCode::InvalidField,
            );
        }
    }
    Ok(())
}

#[test]
fn row_access_json_integer_bounds_and_normalization_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let read = fixture_string_at(&fixture, "/commands/read_rows", 0)?;
    let maximum_bytes = u64::try_from(MAXIMUM_ROW_WINDOW_BYTES)
        .map_err(|_error| fixture_error("row-window byte limit does not fit u64"))?;
    let maximum_rows = u64::try_from(MAXIMUM_ROW_WINDOW_ROWS)
        .map_err(|_error| fixture_error("row-window row limit does not fit u64"))?;

    for (pointer, admitted) in [
        ("/maximumBytes", [1, maximum_bytes]),
        ("/rowCount", [1, maximum_rows]),
    ] {
        for number in admitted {
            let mut value: Value = serde_json::from_str(read)?;
            set_pointer(&mut value, pointer, Value::from(number))?;
            assert!(decode_value(45, MessageRole::Request, value).is_ok());
        }
    }
    for (pointer, rejected) in [
        ("/maximumBytes", [0, maximum_bytes + 1]),
        ("/rowCount", [0, maximum_rows + 1]),
    ] {
        for number in rejected {
            let mut value: Value = serde_json::from_str(read)?;
            set_pointer(&mut value, pointer, Value::from(number))?;
            assert_value_code(
                45,
                MessageRole::Request,
                value,
                ProtocolErrorCode::InvalidField,
            );
        }
    }

    for pointer in ["/maximumBytes", "/rowCount"] {
        let mut value: Value = serde_json::from_str(read)?;
        set_pointer(&mut value, pointer, Value::from(1.5))?;
        assert_value_code(
            45,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }

    let oldest = fixture_string_at(&fixture, "/commands/read_rows", 1)?;
    for offset in [0_u64, u64::from(u32::MAX)] {
        let mut value: Value = serde_json::from_str(oldest)?;
        set_pointer(&mut value, "/origin/offset", Value::from(offset))?;
        assert!(decode_value(45, MessageRole::Request, value).is_ok());
    }
    let mut excessive_offset: Value = serde_json::from_str(oldest)?;
    set_pointer(
        &mut excessive_offset,
        "/origin/offset",
        Value::from(u64::from(u32::MAX) + 1),
    )?;
    assert_value_code(
        45,
        MessageRole::Request,
        excessive_offset,
        ProtocolErrorCode::InvalidField,
    );

    let committed = fixture_string_at(&fixture, "/commands/read_rows", 2)?;
    for delta in [-4_096, 0, 4_096] {
        let mut value: Value = serde_json::from_str(committed)?;
        set_pointer(&mut value, "/origin/delta", Value::from(delta))?;
        assert!(decode_value(45, MessageRole::Request, value).is_ok());
    }
    for delta in [-4_097, 4_097] {
        let mut value: Value = serde_json::from_str(committed)?;
        set_pointer(&mut value, "/origin/delta", Value::from(delta))?;
        assert_value_code(
            45,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }

    let exponential = read.replacen("\"maximumBytes\":65536", "\"maximumBytes\":1e3", 1);
    let RowAccessControl::ReadRows(exponential) =
        decode_row_access_control(45, MessageRole::Request, exponential.as_bytes())?
    else {
        return Err(fixture_error("exponential read decoded to the wrong variant").into());
    };
    assert_eq!(exponential.maximum_bytes, 1_000);

    let decimal_delta = committed.replacen("\"delta\":-1", "\"delta\":-1.0", 1);
    let RowAccessControl::ReadRows(decimal_delta) =
        decode_row_access_control(45, MessageRole::Request, decimal_delta.as_bytes())?
    else {
        return Err(fixture_error("decimal delta decoded to the wrong variant").into());
    };
    assert_eq!(
        decimal_delta.origin,
        RowWindowOrigin::Committed { delta: -1 }
    );
    Ok(())
}

#[test]
fn current_view_revision_policy_admission_matrix_is_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let base = fixture_string_at(&fixture, "/commands/read_rows", 0)?;
    let origins = [
        ("absolute", json!({ "kind": "absolute", "row": "0" })),
        ("oldest", json!({ "kind": "oldest", "offset": 0 })),
        ("committed", json!({ "delta": 0, "kind": "committed" })),
        ("tail", json!({ "kind": "tail", "rowsBefore": 0 })),
    ];

    for purpose in ["buffer", "search", "viewport"] {
        for (origin_kind, origin) in origins.clone() {
            let mut current: Value = serde_json::from_str(base)?;
            set_pointer(&mut current, "/purpose", Value::String(purpose.to_owned()))?;
            set_pointer(
                &mut current,
                "/revisionPolicy",
                Value::String("current-view".to_owned()),
            )?;
            set_pointer(&mut current, "/origin", origin.clone())?;
            let admitted = purpose == "viewport" && matches!(origin_kind, "committed" | "tail");
            assert_eq!(
                decode_value(45, MessageRole::Request, current).is_ok(),
                admitted,
                "current-view matrix mismatch for {purpose}/{origin_kind}"
            );

            let mut exact: Value = serde_json::from_str(base)?;
            set_pointer(&mut exact, "/purpose", Value::String(purpose.to_owned()))?;
            set_pointer(
                &mut exact,
                "/revisionPolicy",
                Value::String("exact".to_owned()),
            )?;
            set_pointer(&mut exact, "/origin", origin)?;
            assert!(
                decode_value(45, MessageRole::Request, exact).is_ok(),
                "exact policy rejected {purpose}/{origin_kind}"
            );
        }
    }
    Ok(())
}

#[test]
fn public_row_access_values_encode_all_variants_and_revalidate_constraints()
-> Result<(), Box<dyn Error>> {
    let attachment_id = orchbee_terminal_protocol::AttachmentId::parse(&"22".repeat(16))?;
    let one = nonzero(1)?;
    let maximum = nonzero(u64::MAX)?;
    let maximum_bytes = u32::try_from(MAXIMUM_ROW_WINDOW_BYTES)
        .map_err(|_error| fixture_error("row-window byte limit does not fit u32"))?;
    let maximum_rows = u16::try_from(MAXIMUM_ROW_WINDOW_ROWS)
        .map_err(|_error| fixture_error("row-window row limit does not fit u16"))?;

    for origin in [
        RowWindowOrigin::Absolute { row: u64::MAX },
        RowWindowOrigin::Oldest { offset: u32::MAX },
        RowWindowOrigin::Committed { delta: -4_096 },
        RowWindowOrigin::Committed { delta: 4_096 },
        RowWindowOrigin::Tail {
            rows_before: u32::MAX,
        },
    ] {
        assert_public_round_trip(RowAccessControl::ReadRows(ReadRows {
            attachment_id,
            detail: RowWindowDetail::Render,
            generation: maximum,
            maximum_bytes,
            origin,
            purpose: RowWindowPurpose::Buffer,
            request_sequence: maximum,
            revision: u64::MAX,
            revision_policy: RowRevisionPolicy::Exact,
            row_count: maximum_rows,
        }))?;
    }

    for detail in [RowWindowDetail::Render, RowWindowDetail::Text] {
        for purpose in [
            RowWindowPurpose::Buffer,
            RowWindowPurpose::Search,
            RowWindowPurpose::Viewport,
        ] {
            assert_public_round_trip(RowAccessControl::ReadRows(ReadRows {
                attachment_id,
                detail,
                generation: one,
                maximum_bytes: 1,
                origin: RowWindowOrigin::Tail { rows_before: 0 },
                purpose,
                request_sequence: one,
                revision: 0,
                revision_policy: RowRevisionPolicy::Exact,
                row_count: 1,
            }))?;
        }
    }

    assert_public_round_trip(RowAccessControl::ReadRows(ReadRows {
        attachment_id,
        detail: RowWindowDetail::Text,
        generation: one,
        maximum_bytes: 1,
        origin: RowWindowOrigin::Committed { delta: 0 },
        purpose: RowWindowPurpose::Viewport,
        request_sequence: one,
        revision: 0,
        revision_policy: RowRevisionPolicy::CurrentView,
        row_count: 1,
    }))?;

    let applied = RowsApplied {
        attachment_id,
        generation: maximum,
        request_sequence: maximum,
        revision: u64::MAX,
    };
    assert_public_round_trip(RowAccessControl::RowsAppliedRequest(applied))?;
    assert_public_round_trip(RowAccessControl::RowsAppliedResponse(applied))?;

    for invalid in [
        ReadRows {
            maximum_bytes: 0,
            ..minimum_read(attachment_id, one)
        },
        ReadRows {
            maximum_bytes: maximum_bytes + 1,
            ..minimum_read(attachment_id, one)
        },
        ReadRows {
            row_count: 0,
            ..minimum_read(attachment_id, one)
        },
        ReadRows {
            row_count: maximum_rows + 1,
            ..minimum_read(attachment_id, one)
        },
        ReadRows {
            origin: RowWindowOrigin::Committed { delta: 4_097 },
            ..minimum_read(attachment_id, one)
        },
        ReadRows {
            origin: RowWindowOrigin::Absolute { row: 0 },
            purpose: RowWindowPurpose::Viewport,
            revision_policy: RowRevisionPolicy::CurrentView,
            ..minimum_read(attachment_id, one)
        },
    ] {
        assert_code(
            encode_row_access_control(&RowAccessControl::ReadRows(invalid)),
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn row_access_kind_role_and_control_json_boundaries_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let read = fixture_string_at(&fixture, "/commands/read_rows", 0)?;
    for (kind, role) in [
        (44, MessageRole::Request),
        (45, MessageRole::Response),
        (45, MessageRole::Event),
        (46, MessageRole::Response),
        (49, MessageRole::Event),
        (50, MessageRole::Event),
        (51, MessageRole::Request),
    ] {
        assert_payload_code(kind, role, read, ProtocolErrorCode::InvalidField);
    }

    let prefix_length = MAXIMUM_CONTROL_JSON_BYTES
        .checked_sub(read.len())
        .ok_or_else(|| fixture_error("canonical row-access fixture exceeds the control bound"))?;
    let mut exact = vec![b' '; prefix_length];
    exact.extend_from_slice(read.as_bytes());
    assert!(decode_row_access_control(45, MessageRole::Request, &exact).is_ok());
    exact.insert(0, b' ');
    assert_code(
        decode_row_access_control(45, MessageRole::Request, &exact),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn row_access_control_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let applied = fixture_string(&fixture, "/commands/rows_applied")?;
    for (kind, role, payload) in [
        (
            45,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/read_rows", 2)?,
        ),
        (50, MessageRole::Request, applied),
        (50, MessageRole::Response, applied),
    ] {
        let value = decode_row_access_control(kind, role, payload.as_bytes())?;
        let EncodedRowAccessControl {
            kind,
            payload,
            role,
        } = encode_row_access_control(&value)?;
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
            decode_row_access_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn minimum_read(
    attachment_id: orchbee_terminal_protocol::AttachmentId,
    one: NonZeroU64,
) -> ReadRows {
    ReadRows {
        attachment_id,
        detail: RowWindowDetail::Text,
        generation: one,
        maximum_bytes: 1,
        origin: RowWindowOrigin::Tail { rows_before: 0 },
        purpose: RowWindowPurpose::Viewport,
        request_sequence: one,
        revision: 0,
        revision_policy: RowRevisionPolicy::CurrentView,
        row_count: 1,
    }
}

fn nonzero(value: u64) -> Result<NonZeroU64, io::Error> {
    NonZeroU64::new(value).ok_or_else(|| fixture_error("test expected a positive integer"))
}

fn assert_public_round_trip(value: RowAccessControl) -> Result<(), ProtocolError> {
    let encoded = encode_row_access_control(&value)?;
    assert_eq!(
        decode_row_access_control(encoded.kind, encoded.role, &encoded.payload)?,
        value
    );
    Ok(())
}

fn assert_canonical_round_trip(
    kind: u16,
    role: MessageRole,
    payload: &str,
) -> Result<(), Box<dyn Error>> {
    let decoded = decode_row_access_control(kind, role, payload.as_bytes())?;
    let encoded = encode_row_access_control(&decoded)?;
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
) -> Result<RowAccessControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_row_access_control(kind, role, &payload)
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_row_access_control(kind, role, payload.as_bytes()),
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

fn object_at_mut<'value>(
    value: &'value mut Value,
    pointer: &str,
) -> Result<&'value mut serde_json::Map<String, Value>, io::Error> {
    value
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| fixture_error("test JSON pointer is not an object"))
}

fn fixture_error(detail: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, detail)
}
