use std::{error::Error, io};

use orchbee_terminal_protocol::{
    EncodedResizeControl, Envelope, MAXIMUM_CONTROL_JSON_BYTES, MessageRole, PayloadEncoding,
    ProtocolError, ProtocolErrorCode, ResizeControl, TerminalPixelGeometry, decode_resize_control,
    decode_transport_envelope, encode_resize_control, encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.25.json");

#[test]
fn canonical_resize_payloads_decode_and_reencode_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    assert_canonical_round_trip(
        32,
        MessageRole::Request,
        fixture_string(&fixture, "/commands/resize_terminal")?,
    )?;
    assert_canonical_round_trip(
        33,
        MessageRole::Response,
        fixture_string(&fixture, "/outputs/resize_accepted")?,
    )?;
    Ok(())
}

#[test]
fn canonical_payloads_retain_complete_resize_facts() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let expected_geometry = TerminalPixelGeometry {
        cell_pixel_height: 16,
        cell_pixel_width: 8,
        columns: 100,
        rows: 30,
    };

    let request = decode_resize_control(
        32,
        MessageRole::Request,
        fixture_string(&fixture, "/commands/resize_terminal")?.as_bytes(),
    )?;
    let ResizeControl::ResizeTerminal(request) = request else {
        return Err(fixture_error("resize request decoded to the wrong variant").into());
    };
    assert_eq!(request.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(request.geometry, expected_geometry);

    let response = decode_resize_control(
        33,
        MessageRole::Response,
        fixture_string(&fixture, "/outputs/resize_accepted")?.as_bytes(),
    )?;
    let ResizeControl::ResizeAccepted(response) = response else {
        return Err(fixture_error("resize response decoded to the wrong variant").into());
    };
    assert_eq!(response.attachment_id, request.attachment_id);
    assert_eq!(response.geometry, expected_geometry);
    assert_eq!(response.revision, 43);
    Ok(())
}

#[test]
fn resize_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_resize_control(32, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_code(
        decode_resize_control(32, MessageRole::Request, b"{"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_resize_control(32, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );

    let fixture = read_fixture()?;
    let request = fixture_string(&fixture, "/commands/resize_terminal")?;
    assert_payload_code(
        32,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );
    let unknown = request.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        32,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = request.replacen("\"columns\":", "\"columns\":80,\"columns\":", 1);
    assert_payload_code(
        32,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate =
        request.replacen("\"columns\":", "\"columns\":80,\"col\\u0075mns\":", 1);
    assert_payload_code(
        32,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let response = fixture_string(&fixture, "/outputs/resize_accepted")?;
    let mut missing: Value = serde_json::from_str(response)?;
    object_mut(&mut missing)?.remove("revision");
    assert_value_code(
        33,
        MessageRole::Response,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn resize_identity_and_revision_obey_shared_refinements() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let request = fixture_string(&fixture, "/commands/resize_terminal")?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(request)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(
            32,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let response = fixture_string(&fixture, "/outputs/resize_accepted")?;
    for revision in ["00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(response)?;
        set_pointer(&mut value, "/revision", Value::String(revision.to_owned()))?;
        assert_value_code(
            33,
            MessageRole::Response,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    for revision in ["0".to_owned(), u64::MAX.to_string()] {
        let mut value: Value = serde_json::from_str(response)?;
        set_pointer(&mut value, "/revision", Value::String(revision))?;
        assert!(decode_value(33, MessageRole::Response, value).is_ok());
    }
    let mut numeric: Value = serde_json::from_str(response)?;
    set_pointer(&mut numeric, "/revision", Value::from(43))?;
    assert_value_code(
        33,
        MessageRole::Response,
        numeric,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn resize_dimensions_obey_binary64_integer_and_field_bounds() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let request = fixture_string(&fixture, "/commands/resize_terminal")?;
    let rounded = request.replacen("\"columns\":100", "\"columns\":1.0000000000000001", 1);
    let decoded = decode_resize_control(32, MessageRole::Request, rounded.as_bytes())?;
    assert!(matches!(
        decoded,
        ResizeControl::ResizeTerminal(value) if value.geometry.columns == 1
    ));

    for (pointer, replacement) in [
        ("/cellPixelHeight", Value::from(0)),
        ("/cellPixelWidth", Value::from(4097)),
        ("/columns", Value::from(0)),
        ("/rows", Value::from(4097)),
        ("/columns", Value::from(9_007_199_254_740_992_u64)),
    ] {
        let mut value: Value = serde_json::from_str(request)?;
        set_pointer(&mut value, pointer, replacement)?;
        assert_value_code(
            32,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    for token in ["1.5", "-1", "1e4000"] {
        let payload = request.replacen("\"columns\":100", &format!("\"columns\":{token}"), 1);
        assert_payload_code(
            32,
            MessageRole::Request,
            &payload,
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn resize_requests_enforce_grid_and_pixel_extent_bounds() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    assert_request_geometry(&fixture, 4096, 64, 1, 1, true)?;
    assert_request_geometry(&fixture, 4096, 65, 1, 1, false)?;
    assert_request_geometry(&fixture, 255, 257, 257, 255, true)?;
    assert_request_geometry(&fixture, 4096, 1, 16, 1, false)?;
    assert_request_geometry(&fixture, 1, 4096, 1, 16, false)?;
    assert_request_geometry(&fixture, 80, 24, 0, 0, true)?;
    assert_request_geometry(&fixture, 80, 24, 0, 16, false)?;
    assert_request_geometry(&fixture, 80, 24, 8, 0, false)?;
    Ok(())
}

#[test]
fn resize_requests_accept_a_paired_zero_cell_pixel_geometry() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let request = fixture_string(&fixture, "/commands/resize_terminal")?;
    let mut value: Value = serde_json::from_str(request)?;
    set_pointer(&mut value, "/cellPixelWidth", Value::from(0))?;
    set_pointer(&mut value, "/cellPixelHeight", Value::from(0))?;
    let decoded = decode_value(32, MessageRole::Request, value)?;
    assert!(matches!(
        decoded,
        ResizeControl::ResizeTerminal(geometry)
            if geometry.geometry.cell_pixel_width == 0 && geometry.geometry.cell_pixel_height == 0
    ));
    Ok(())
}

#[test]
fn response_decode_preserves_legacy_dimension_bounds_and_rejects_mixed_pixel_pairs()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let response = fixture_string(&fixture, "/outputs/resize_accepted")?;
    for (columns, rows, cell_pixel_width, cell_pixel_height) in
        [(4096, 65, 1, 1), (4096, 1, 16, 1), (1, 4096, 1, 16)]
    {
        let value = geometry_value(response, columns, rows, cell_pixel_width, cell_pixel_height)?;
        let decoded = decode_value(33, MessageRole::Response, value)?;
        assert_code(
            encode_resize_control(&decoded),
            ProtocolErrorCode::InvalidField,
        );
    }

    let unknown = geometry_value(response, 80, 24, 0, 0)?;
    let decoded = decode_value(33, MessageRole::Response, unknown)?;
    encode_resize_control(&decoded)?;

    for (cell_pixel_width, cell_pixel_height) in [(0, 16), (8, 0)] {
        let value = geometry_value(response, 80, 24, cell_pixel_width, cell_pixel_height)?;
        assert_code(
            decode_value(33, MessageRole::Response, value),
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn public_resize_values_are_revalidated_before_encoding() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let request = decode_resize_control(
        32,
        MessageRole::Request,
        fixture_string(&fixture, "/commands/resize_terminal")?.as_bytes(),
    )?;
    let ResizeControl::ResizeTerminal(mut request) = request else {
        return Err(fixture_error("resize request decoded to the wrong variant").into());
    };
    request.geometry.columns = 0;
    assert_code(
        encode_resize_control(&ResizeControl::ResizeTerminal(request)),
        ProtocolErrorCode::InvalidField,
    );

    let response = decode_resize_control(
        33,
        MessageRole::Response,
        fixture_string(&fixture, "/outputs/resize_accepted")?.as_bytes(),
    )?;
    let ResizeControl::ResizeAccepted(mut response) = response else {
        return Err(fixture_error("resize response decoded to the wrong variant").into());
    };
    response.geometry.rows = 0;
    assert_code(
        encode_resize_control(&ResizeControl::ResizeAccepted(response)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn resize_kind_role_and_payload_bounds_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let request = fixture_string(&fixture, "/commands/resize_terminal")?;
    for (kind, role) in [
        (31, MessageRole::Response),
        (32, MessageRole::Response),
        (32, MessageRole::Event),
        (33, MessageRole::Request),
        (33, MessageRole::Event),
        (34, MessageRole::Request),
    ] {
        assert_payload_code(kind, role, request, ProtocolErrorCode::InvalidField);
    }
    let oversized = vec![b' '; MAXIMUM_CONTROL_JSON_BYTES + 1];
    assert_code(
        decode_resize_control(32, MessageRole::Request, &oversized),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn resize_control_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, payload) in [
        (
            32,
            MessageRole::Request,
            fixture_string(&fixture, "/commands/resize_terminal")?,
        ),
        (
            33,
            MessageRole::Response,
            fixture_string(&fixture, "/outputs/resize_accepted")?,
        ),
    ] {
        let value = decode_resize_control(kind, role, payload.as_bytes())?;
        let EncodedResizeControl {
            kind,
            payload,
            role,
        } = encode_resize_control(&value)?;
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
        assert_eq!(
            decode_resize_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn assert_canonical_round_trip(
    kind: u16,
    role: MessageRole,
    payload: &str,
) -> Result<(), Box<dyn Error>> {
    let decoded = decode_resize_control(kind, role, payload.as_bytes())?;
    let encoded = encode_resize_control(&decoded)?;
    assert_eq!(encoded.kind, kind);
    assert_eq!(encoded.role, role);
    assert_eq!(encoded.payload, payload.as_bytes());
    Ok(())
}

fn assert_request_geometry(
    fixture: &Value,
    columns: u64,
    rows: u64,
    cell_pixel_width: u64,
    cell_pixel_height: u64,
    valid: bool,
) -> Result<(), Box<dyn Error>> {
    let value = geometry_value(
        fixture_string(fixture, "/commands/resize_terminal")?,
        columns,
        rows,
        cell_pixel_width,
        cell_pixel_height,
    )?;
    let result = decode_value(32, MessageRole::Request, value);
    if valid {
        result?;
    } else {
        assert_code(result, ProtocolErrorCode::InvalidField);
    }
    Ok(())
}

fn geometry_value(
    source: &str,
    columns: u64,
    rows: u64,
    cell_pixel_width: u64,
    cell_pixel_height: u64,
) -> Result<Value, Box<dyn Error>> {
    let mut value: Value = serde_json::from_str(source)?;
    set_pointer(&mut value, "/columns", Value::from(columns))?;
    set_pointer(&mut value, "/rows", Value::from(rows))?;
    set_pointer(&mut value, "/cellPixelWidth", Value::from(cell_pixel_width))?;
    set_pointer(
        &mut value,
        "/cellPixelHeight",
        Value::from(cell_pixel_height),
    )?;
    Ok(value)
}

fn read_fixture() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONTROL_FIXTURE)
}

fn fixture_string<'value>(value: &'value Value, pointer: &str) -> Result<&'value str, io::Error> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| fixture_error(&format!("fixture string {pointer} is missing")))
}

fn decode_value(
    kind: u16,
    role: MessageRole,
    value: Value,
) -> Result<ResizeControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_resize_control(kind, role, &payload)
}

fn object_mut(value: &mut Value) -> Result<&mut serde_json::Map<String, Value>, io::Error> {
    value
        .as_object_mut()
        .ok_or_else(|| fixture_error("fixture value is not an object"))
}

fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| fixture_error(&format!("fixture pointer {pointer} is missing")))?;
    *target = replacement;
    Ok(())
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_resize_control(kind, role, payload.as_bytes()),
        expected,
    );
}

fn fixture_error(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}

fn assert_code<ValueType>(result: Result<ValueType, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Ok(_value) => panic!("expected protocol error {expected}"),
        Err(error) => assert_eq!(error.code(), expected),
    }
}
