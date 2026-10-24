use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    AttachmentId, CorrelationId, EncodedSelectionControl, Envelope, HostEpoch,
    MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_SELECTION_PREVIEW_BYTES, MessageRole, PayloadEncoding,
    ProtocolError, ProtocolErrorCode, SelectionChanged, SelectionChangedState, SelectionCommand,
    SelectionCommandKind, SelectionControl, SelectionGesture, SelectionGestureAction,
    SelectionGestureGeometry, SelectionGesturePosition, SelectionPoint, SelectionViewportPoint,
    decode_selection_control, decode_transport_envelope, encode_selection_control,
    encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance.json");

#[test]
fn canonical_selection_payloads_round_trip_in_every_legal_direction() -> Result<(), Box<dyn Error>>
{
    let fixture = read_fixture()?;
    for payload in fixture_strings(&fixture, "/commands/selection_gesture")? {
        assert_canonical_round_trip(60, MessageRole::Request, payload)?;
    }
    for payload in fixture_strings(&fixture, "/commands/selection_command")? {
        assert_canonical_round_trip(61, MessageRole::Request, payload)?;
    }
    for payload in fixture_strings(&fixture, "/outputs/selection_changed")? {
        assert_canonical_round_trip(62, MessageRole::Response, payload)?;
        assert_canonical_round_trip(62, MessageRole::Event, payload)?;
    }
    Ok(())
}

#[test]
fn canonical_gestures_retain_every_action_specific_fact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let payloads = fixture_strings(&fixture, "/commands/selection_gesture")?;
    let expected = [
        SelectionGestureAction::Start {
            click_count: 2,
            point: SelectionViewportPoint { column: 4, row: 10 },
            position: SelectionGesturePosition { x: 36, y: 20 },
            select_output: false,
        },
        SelectionGestureAction::Update {
            geometry: SelectionGestureGeometry {
                cell_width: 8,
                columns: 120,
                padding_left: 0,
                screen_height: 640,
            },
            point: SelectionViewportPoint { column: 4, row: 10 },
            position: SelectionGesturePosition { x: 44, y: -1 },
            rectangle: true,
        },
        SelectionGestureAction::End {
            point: SelectionViewportPoint { column: 4, row: 10 },
        },
        SelectionGestureAction::Cancel,
    ];

    for (payload, expected_action) in payloads.into_iter().zip(expected) {
        let decoded = decode_selection_control(60, MessageRole::Request, payload.as_bytes())?;
        let SelectionControl::Gesture(gesture) = decoded else {
            return Err(fixture_error("gesture decoded to the wrong variant").into());
        };
        assert_eq!(gesture.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(gesture.generation.get(), 3);
        assert_eq!(gesture.action, expected_action);
    }
    Ok(())
}

#[test]
fn canonical_commands_retain_absolute_points_revision_and_generation() -> Result<(), Box<dyn Error>>
{
    let fixture = read_fixture()?;
    let payloads = fixture_strings(&fixture, "/commands/selection_command")?;
    let expected = [
        SelectionCommandKind::Clear,
        SelectionCommandKind::SelectAll,
        SelectionCommandKind::SetRange {
            end: SelectionPoint { column: 9, row: 43 },
            rectangle: true,
            start: SelectionPoint {
                column: 4,
                row: 100,
            },
        },
        SelectionCommandKind::SelectOutput {
            point: SelectionPoint {
                column: 4,
                row: 100,
            },
        },
    ];

    for (payload, expected_command) in payloads.into_iter().zip(expected) {
        let decoded = decode_selection_control(61, MessageRole::Request, payload.as_bytes())?;
        let SelectionControl::Command(command) = decoded else {
            return Err(fixture_error("command decoded to the wrong variant").into());
        };
        assert_eq!(command.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(command.generation.get(), 3);
        assert_eq!(command.revision, 42);
        assert_eq!(command.command, expected_command);
    }
    Ok(())
}

#[test]
fn canonical_changed_facts_preserve_state_nullability_and_direction() -> Result<(), Box<dyn Error>>
{
    let fixture = read_fixture()?;
    let payloads = fixture_strings(&fixture, "/outputs/selection_changed")?;
    let expected = [
        SelectionChangedState::None,
        SelectionChangedState::Evicted,
        SelectionChangedState::Active {
            end: SelectionPoint {
                column: 7,
                row: 102,
            },
            preview: None,
            rectangle: true,
            start: SelectionPoint {
                column: 4,
                row: 100,
            },
        },
    ];

    for (payload, expected_state) in payloads.into_iter().zip(expected) {
        let response = decode_selection_control(62, MessageRole::Response, payload.as_bytes())?;
        let SelectionControl::ChangedResponse(changed) = response else {
            return Err(fixture_error("changed response decoded to the wrong variant").into());
        };
        assert_eq!(changed.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(changed.state, expected_state);
        assert_eq!(
            decode_selection_control(62, MessageRole::Event, payload.as_bytes())?,
            SelectionControl::ChangedEvent(changed)
        );
    }
    Ok(())
}

#[test]
fn selection_json_is_utf8_unique_complete_and_closed_at_every_depth() -> Result<(), Box<dyn Error>>
{
    assert_code(
        decode_selection_control(60, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(
        60,
        MessageRole::Request,
        "{",
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_selection_control(60, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(
        60,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let start = fixture_string_at(&fixture, "/commands/selection_gesture", 0)?;
    let unknown = start.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        60,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = start.replacen("\"action\":", "\"action\":\"end\",\"action\":", 1);
    assert_payload_code(
        60,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = start.replacen(
        "\"clickCount\":",
        "\"clickCount\":1,\"clickC\\u006funt\":",
        1,
    );
    assert_payload_code(
        60,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let nested_duplicate = start.replacen("\"column\":4", "\"column\":3,\"col\\u0075mn\":4", 1);
    assert_payload_code(
        60,
        MessageRole::Request,
        &nested_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let mut missing: Value = serde_json::from_str(start)?;
    object_at_mut(&mut missing, "/point")?.remove("row");
    assert_value_code(
        60,
        MessageRole::Request,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn selection_identity_and_decimal_uint64_refinements_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let gesture = fixture_string_at(&fixture, "/commands/selection_gesture", 0)?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(gesture)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(
            60,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }
    for invalid in ["0", "00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(gesture)?;
        set_pointer(&mut value, "/generation", Value::String(invalid.to_owned()))?;
        assert_value_code(
            60,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }

    let command = fixture_string_at(&fixture, "/commands/selection_command", 2)?;
    for pointer in ["/revision", "/start/row", "/end/row"] {
        for invalid in ["00", "-1", "+1", "18446744073709551616", " 1"] {
            let mut value: Value = serde_json::from_str(command)?;
            set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
            assert_value_code(
                61,
                MessageRole::Request,
                value,
                ProtocolErrorCode::InvalidField,
            );
        }
    }
    let mut boundaries: Value = serde_json::from_str(command)?;
    set_pointer(&mut boundaries, "/revision", Value::String("0".to_owned()))?;
    set_pointer(
        &mut boundaries,
        "/start/row",
        Value::String(u64::MAX.to_string()),
    )?;
    set_pointer(&mut boundaries, "/end/row", Value::String("0".to_owned()))?;
    assert!(decode_value(61, MessageRole::Request, boundaries).is_ok());
    Ok(())
}

#[test]
fn selection_discriminants_require_exact_variant_members() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (pointer, index, discriminant, invalid) in [
        ("/commands/selection_gesture", 0, "/action", "future"),
        ("/commands/selection_command", 0, "/command", "future"),
        ("/outputs/selection_changed", 0, "/state", "future"),
    ] {
        let payload = fixture_string_at(&fixture, pointer, index)?;
        let mut value: Value = serde_json::from_str(payload)?;
        set_pointer(&mut value, discriminant, Value::String(invalid.to_owned()))?;
        let (kind, role) = match pointer {
            "/commands/selection_gesture" => (60, MessageRole::Request),
            "/commands/selection_command" => (61, MessageRole::Request),
            _ => (62, MessageRole::Event),
        };
        assert_value_code(kind, role, value, ProtocolErrorCode::InvalidField);
    }

    for index in 0..4 {
        let payload = fixture_string_at(&fixture, "/commands/selection_gesture", index)?;
        let mut value: Value = serde_json::from_str(payload)?;
        object_mut(&mut value)?.insert("revision".to_owned(), Value::String("1".to_owned()));
        assert_value_code(
            60,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    for (index, name, value) in [
        (0, "point", serde_json::json!({"column": 1, "row": "2"})),
        (1, "rectangle", Value::Bool(false)),
        (2, "clickCount", Value::from(2)),
        (3, "point", serde_json::json!({"column": 1, "row": 2})),
    ] {
        let payload = fixture_string_at(&fixture, "/commands/selection_command", index)?;
        let mut candidate: Value = serde_json::from_str(payload)?;
        object_mut(&mut candidate)?.insert(name.to_owned(), value);
        assert_value_code(
            61,
            MessageRole::Request,
            candidate,
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn selection_numeric_fields_obey_safe_integer_and_exact_field_bounds() -> Result<(), Box<dyn Error>>
{
    let fixture = read_fixture()?;
    let start = fixture_string_at(&fixture, "/commands/selection_gesture", 0)?;
    for (pointer, accepted, rejected) in [
        ("/clickCount", vec![1_i64, 3], vec![0_i64, 4]),
        ("/point/column", vec![0, 4095], vec![-1, 4096]),
        ("/point/row", vec![0, 4095], vec![-1, 4096]),
        ("/position/x", vec![-65_535, 65_535], vec![-65_536, 65_536]),
        ("/position/y", vec![-65_535, 65_535], vec![-65_536, 65_536]),
    ] {
        assert_integer_partition(
            60,
            MessageRole::Request,
            start,
            pointer,
            &accepted,
            &rejected,
        )?;
    }

    let update = fixture_string_at(&fixture, "/commands/selection_gesture", 1)?;
    for (pointer, accepted, rejected) in [
        ("/geometry/cellWidth", vec![1_i64, 4096], vec![0_i64, 4097]),
        ("/geometry/columns", vec![1, 4096], vec![0, 4097]),
        ("/geometry/paddingLeft", vec![0, 65_535], vec![-1, 65_536]),
        ("/geometry/screenHeight", vec![1, 65_535], vec![0, 65_536]),
    ] {
        assert_integer_partition(
            60,
            MessageRole::Request,
            update,
            pointer,
            &accepted,
            &rejected,
        )?;
    }

    let range = fixture_string_at(&fixture, "/commands/selection_command", 2)?;
    for pointer in ["/start/column", "/end/column"] {
        assert_integer_partition(
            61,
            MessageRole::Request,
            range,
            pointer,
            &[0, 4095],
            &[-1, 4096],
        )?;
    }

    for (kind, role, payload, pointer) in [
        (60, MessageRole::Request, start, "/clickCount"),
        (60, MessageRole::Request, update, "/position/x"),
        (61, MessageRole::Request, range, "/start/column"),
    ] {
        let fractional = replace_numeric_member(payload, pointer_name(pointer), "1.5")?;
        assert_payload_code(kind, role, &fractional, ProtocolErrorCode::InvalidField);
        let unsafe_integer =
            replace_numeric_member(payload, pointer_name(pointer), "9007199254740992")?;
        assert_payload_code(kind, role, &unsafe_integer, ProtocolErrorCode::InvalidField);
        let normalized = replace_numeric_member(payload, pointer_name(pointer), "1.0")?;
        assert!(decode_selection_control(kind, role, normalized.as_bytes()).is_ok());
    }
    Ok(())
}

#[test]
fn active_preview_is_required_nullable_utf8_bounded_and_nul_preserving()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let active = fixture_string_at(&fixture, "/outputs/selection_changed", 2)?;
    for preview in [
        Value::Null,
        Value::String(String::new()),
        Value::String("x".repeat(MAXIMUM_SELECTION_PREVIEW_BYTES)),
        Value::String("é".repeat(MAXIMUM_SELECTION_PREVIEW_BYTES / 2)),
        Value::String("left\0right".to_owned()),
    ] {
        let mut value: Value = serde_json::from_str(active)?;
        set_pointer(&mut value, "/preview", preview)?;
        assert!(decode_value(62, MessageRole::Event, value).is_ok());
    }
    for preview in [
        "x".repeat(MAXIMUM_SELECTION_PREVIEW_BYTES + 1),
        "é".repeat(MAXIMUM_SELECTION_PREVIEW_BYTES / 2 + 1),
    ] {
        let mut value: Value = serde_json::from_str(active)?;
        set_pointer(&mut value, "/preview", Value::String(preview))?;
        assert_value_code(
            62,
            MessageRole::Event,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    let mut missing: Value = serde_json::from_str(active)?;
    object_mut(&mut missing)?.remove("preview");
    assert_value_code(
        62,
        MessageRole::Event,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn public_selection_values_encode_all_variants_and_revalidate_bounds() -> Result<(), Box<dyn Error>>
{
    let attachment_id = example_attachment()?;
    let generation = nonzero(3)?;
    let point = SelectionViewportPoint { column: 1, row: 2 };
    let absolute = SelectionPoint { column: 1, row: 2 };
    let gesture_actions = [
        SelectionGestureAction::Start {
            click_count: 1,
            point,
            position: SelectionGesturePosition {
                x: -65_535,
                y: 65_535,
            },
            select_output: true,
        },
        SelectionGestureAction::Update {
            geometry: SelectionGestureGeometry {
                cell_width: 1,
                columns: 4096,
                padding_left: u16::MAX,
                screen_height: u16::MAX,
            },
            point,
            position: SelectionGesturePosition { x: 0, y: 0 },
            rectangle: false,
        },
        SelectionGestureAction::End { point },
        SelectionGestureAction::Cancel,
    ];
    for action in gesture_actions {
        assert_public_round_trip(SelectionControl::Gesture(SelectionGesture {
            action,
            attachment_id,
            generation,
        }))?;
    }

    let commands = [
        SelectionCommandKind::Clear,
        SelectionCommandKind::SelectAll,
        SelectionCommandKind::SetRange {
            end: absolute,
            rectangle: true,
            start: absolute,
        },
        SelectionCommandKind::SelectOutput { point: absolute },
    ];
    for command in commands {
        assert_public_round_trip(SelectionControl::Command(SelectionCommand {
            attachment_id,
            command,
            generation,
            revision: 0,
        }))?;
    }

    for state in [
        SelectionChangedState::None,
        SelectionChangedState::Evicted,
        SelectionChangedState::Active {
            end: absolute,
            preview: Some("left\0right".to_owned()),
            rectangle: false,
            start: absolute,
        },
    ] {
        let changed = SelectionChanged {
            attachment_id,
            state,
        };
        assert_public_round_trip(SelectionControl::ChangedResponse(changed.clone()))?;
        assert_public_round_trip(SelectionControl::ChangedEvent(changed))?;
    }

    assert_code(
        encode_selection_control(&SelectionControl::Gesture(SelectionGesture {
            action: SelectionGestureAction::Start {
                click_count: 4,
                point,
                position: SelectionGesturePosition { x: 0, y: 0 },
                select_output: false,
            },
            attachment_id,
            generation,
        })),
        ProtocolErrorCode::InvalidField,
    );
    assert_code(
        encode_selection_control(&SelectionControl::ChangedEvent(SelectionChanged {
            attachment_id,
            state: SelectionChangedState::Active {
                end: absolute,
                preview: Some("x".repeat(MAXIMUM_SELECTION_PREVIEW_BYTES + 1)),
                rectangle: false,
                start: absolute,
            },
        })),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn selection_kind_role_and_control_json_boundaries_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let gesture = fixture_string_at(&fixture, "/commands/selection_gesture", 3)?;
    for role in [MessageRole::Response, MessageRole::Event] {
        assert_payload_code(60, role, gesture, ProtocolErrorCode::InvalidField);
    }
    let command = fixture_string_at(&fixture, "/commands/selection_command", 0)?;
    for role in [MessageRole::Response, MessageRole::Event] {
        assert_payload_code(61, role, command, ProtocolErrorCode::InvalidField);
    }
    let changed = fixture_string_at(&fixture, "/outputs/selection_changed", 0)?;
    assert_payload_code(
        62,
        MessageRole::Request,
        changed,
        ProtocolErrorCode::InvalidField,
    );
    assert_payload_code(
        59,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let exact = format!(
        "{gesture}{}",
        " ".repeat(MAXIMUM_CONTROL_JSON_BYTES - gesture.len())
    );
    assert_eq!(exact.len(), MAXIMUM_CONTROL_JSON_BYTES);
    assert!(decode_selection_control(60, MessageRole::Request, exact.as_bytes()).is_ok());
    let over = format!("{exact} ");
    assert_code(
        decode_selection_control(60, MessageRole::Request, over.as_bytes()),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn selection_controls_compose_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let values = [
        decode_selection_control(
            60,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/selection_gesture", 0)?.as_bytes(),
        )?,
        decode_selection_control(
            61,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/selection_command", 2)?.as_bytes(),
        )?,
        decode_selection_control(
            62,
            MessageRole::Response,
            fixture_string_at(&fixture, "/outputs/selection_changed", 2)?.as_bytes(),
        )?,
        decode_selection_control(
            62,
            MessageRole::Event,
            fixture_string_at(&fixture, "/outputs/selection_changed", 1)?.as_bytes(),
        )?,
    ];

    for value in values {
        let encoded = encode_selection_control(&value)?;
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
            decode_selection_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn assert_integer_partition(
    kind: u16,
    role: MessageRole,
    payload: &str,
    pointer: &str,
    accepted: &[i64],
    rejected: &[i64],
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

fn pointer_name(pointer: &str) -> &str {
    pointer.rsplit('/').next().unwrap_or(pointer)
}

fn example_attachment() -> Result<AttachmentId, orchbee_terminal_protocol::IdentityError> {
    AttachmentId::parse(&"22".repeat(16))
}

fn nonzero(value: u64) -> Result<NonZeroU64, io::Error> {
    NonZeroU64::new(value).ok_or_else(|| fixture_error("expected a nonzero test value"))
}

fn assert_public_round_trip(value: SelectionControl) -> Result<(), ProtocolError> {
    let encoded = encode_selection_control(&value)?;
    assert_eq!(
        decode_selection_control(encoded.kind, encoded.role, &encoded.payload)?,
        value
    );
    Ok(())
}

fn assert_canonical_round_trip(
    kind: u16,
    role: MessageRole,
    payload: &str,
) -> Result<(), Box<dyn Error>> {
    let decoded = decode_selection_control(kind, role, payload.as_bytes())?;
    let encoded: EncodedSelectionControl = encode_selection_control(&decoded)?;
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

fn decode_value(
    kind: u16,
    role: MessageRole,
    value: Value,
) -> Result<SelectionControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_selection_control(kind, role, &payload)
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_selection_control(kind, role, payload.as_bytes()),
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

fn object_mut(value: &mut Value) -> Result<&mut serde_json::Map<String, Value>, io::Error> {
    value
        .as_object_mut()
        .ok_or_else(|| fixture_error("test JSON value is not an object"))
}

fn fixture_error(detail: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, detail)
}
