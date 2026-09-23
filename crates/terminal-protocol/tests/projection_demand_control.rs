use std::{error::Error, io};

use orchbee_terminal_protocol::{
    EncodedProjectionDemandControl, Envelope, MAXIMUM_CONTROL_JSON_BYTES, MessageRole,
    PayloadEncoding, ProjectionBlockReason, ProjectionBlocked, ProjectionDemand,
    ProjectionDemandAccepted, ProjectionDemandControl, ProtocolError, ProtocolErrorCode,
    SetProjectionDemand, decode_projection_demand_control, decode_transport_envelope,
    encode_projection_demand_control, encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.25.json");

#[test]
fn canonical_projection_demand_payloads_round_trip_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for payload in fixture_strings(&fixture, "/commands/projection_demand")? {
        assert_canonical_round_trip(47, MessageRole::Request, payload)?;
    }
    for payload in fixture_strings(&fixture, "/outputs/projection_demand_accepted")? {
        assert_canonical_round_trip(48, MessageRole::Response, payload)?;
    }
    for payload in fixture_strings(&fixture, "/outputs/projection_blocked")? {
        assert_canonical_round_trip(49, MessageRole::Event, payload)?;
    }
    Ok(())
}

#[test]
fn canonical_payloads_retain_complete_projection_demand_facts() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let commands = fixture_strings(&fixture, "/commands/projection_demand")?;
    let accepted = fixture_strings(&fixture, "/outputs/projection_demand_accepted")?;
    for ((command, response), expected) in commands
        .into_iter()
        .zip(accepted)
        .zip([ProjectionDemand::Active, ProjectionDemand::Paused])
    {
        let request =
            decode_projection_demand_control(47, MessageRole::Request, command.as_bytes())?;
        let ProjectionDemandControl::SetProjectionDemand(request) = request else {
            return Err(fixture_error("projection demand decoded to the wrong variant").into());
        };
        assert_eq!(request.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(request.demand, expected);

        let response =
            decode_projection_demand_control(48, MessageRole::Response, response.as_bytes())?;
        assert_eq!(
            response,
            ProjectionDemandControl::ProjectionDemandAccepted(ProjectionDemandAccepted {
                attachment_id: request.attachment_id,
                demand: request.demand,
            })
        );
    }

    let blocked = fixture_strings(&fixture, "/outputs/projection_blocked")?;
    for (payload, expected) in blocked.into_iter().zip([
        ProjectionBlockReason::CaptureAllocationFailed,
        ProjectionBlockReason::ProjectionTooLarge,
    ]) {
        let event = decode_projection_demand_control(49, MessageRole::Event, payload.as_bytes())?;
        let ProjectionDemandControl::ProjectionBlocked(event) = event else {
            return Err(fixture_error("projection block decoded to the wrong variant").into());
        };
        assert_eq!(event.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(event.reason, expected);
        assert_eq!(event.revision, 42);
    }
    Ok(())
}

#[test]
fn projection_demand_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_projection_demand_control(47, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(
        47,
        MessageRole::Request,
        "{",
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_projection_demand_control(47, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(
        47,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let command = fixture_string_at(&fixture, "/commands/projection_demand", 0)?;
    let unknown = command.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        47,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = command.replacen("\"demand\":", "\"demand\":\"paused\",\"demand\":", 1);
    assert_payload_code(
        47,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate =
        command.replacen("\"demand\":", "\"demand\":\"paused\",\"dem\\u0061nd\":", 1);
    assert_payload_code(
        47,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let blocked = fixture_string_at(&fixture, "/outputs/projection_blocked", 0)?;
    let mut missing: Value = serde_json::from_str(blocked)?;
    object_mut(&mut missing)?.remove("revision");
    assert_value_code(
        49,
        MessageRole::Event,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn attachment_and_blocked_revision_refinements_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let command = fixture_string_at(&fixture, "/commands/projection_demand", 0)?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(command)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(
            47,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let blocked = fixture_string_at(&fixture, "/outputs/projection_blocked", 0)?;
    for revision in ["00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(blocked)?;
        set_pointer(&mut value, "/revision", Value::String(revision.to_owned()))?;
        assert_value_code(
            49,
            MessageRole::Event,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    for revision in ["0".to_owned(), u64::MAX.to_string()] {
        let mut value: Value = serde_json::from_str(blocked)?;
        set_pointer(&mut value, "/revision", Value::String(revision))?;
        assert!(decode_value(49, MessageRole::Event, value).is_ok());
    }
    let mut numeric: Value = serde_json::from_str(blocked)?;
    set_pointer(&mut numeric, "/revision", Value::from(42))?;
    assert_value_code(
        49,
        MessageRole::Event,
        numeric,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn projection_demand_and_block_reason_vocabularies_are_closed() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let command = fixture_string_at(&fixture, "/commands/projection_demand", 0)?;
    for invalid in ["", "ACTIVE", "active ", "future"] {
        let mut value: Value = serde_json::from_str(command)?;
        set_pointer(&mut value, "/demand", Value::String(invalid.to_owned()))?;
        assert_value_code(
            47,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }

    let blocked = fixture_string_at(&fixture, "/outputs/projection_blocked", 0)?;
    for invalid in ["", "capture_allocation_failed", "projection-too-big"] {
        let mut value: Value = serde_json::from_str(blocked)?;
        set_pointer(&mut value, "/reason", Value::String(invalid.to_owned()))?;
        assert_value_code(
            49,
            MessageRole::Event,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn public_projection_demand_values_encode_every_closed_variant() -> Result<(), Box<dyn Error>> {
    let attachment_id = orchbee_terminal_protocol::AttachmentId::parse(&"22".repeat(16))?;
    for demand in [ProjectionDemand::Active, ProjectionDemand::Paused] {
        for value in [
            ProjectionDemandControl::SetProjectionDemand(SetProjectionDemand {
                attachment_id,
                demand,
            }),
            ProjectionDemandControl::ProjectionDemandAccepted(ProjectionDemandAccepted {
                attachment_id,
                demand,
            }),
        ] {
            let encoded = encode_projection_demand_control(&value)?;
            assert_eq!(
                decode_projection_demand_control(encoded.kind, encoded.role, &encoded.payload)?,
                value
            );
        }
    }
    for reason in [
        ProjectionBlockReason::CaptureAllocationFailed,
        ProjectionBlockReason::ProjectionTooLarge,
    ] {
        let value = ProjectionDemandControl::ProjectionBlocked(ProjectionBlocked {
            attachment_id,
            reason,
            revision: u64::MAX,
        });
        let encoded = encode_projection_demand_control(&value)?;
        assert_eq!(
            decode_projection_demand_control(encoded.kind, encoded.role, &encoded.payload)?,
            value
        );
    }
    Ok(())
}

#[test]
fn projection_demand_kind_role_and_json_boundaries_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let command = fixture_string_at(&fixture, "/commands/projection_demand", 0)?;
    for (kind, role) in [
        (46, MessageRole::Response),
        (47, MessageRole::Response),
        (47, MessageRole::Event),
        (48, MessageRole::Request),
        (48, MessageRole::Event),
        (49, MessageRole::Request),
        (49, MessageRole::Response),
        (50, MessageRole::Request),
    ] {
        assert_payload_code(kind, role, command, ProtocolErrorCode::InvalidField);
    }

    let prefix_length = MAXIMUM_CONTROL_JSON_BYTES
        .checked_sub(command.len())
        .ok_or_else(|| fixture_error("canonical demand fixture exceeds the control bound"))?;
    let mut exact = vec![b' '; prefix_length];
    exact.extend_from_slice(command.as_bytes());
    assert!(decode_projection_demand_control(47, MessageRole::Request, &exact).is_ok());
    exact.insert(0, b' ');
    assert_code(
        decode_projection_demand_control(47, MessageRole::Request, &exact),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn projection_demand_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, payload) in [
        (
            47,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/projection_demand", 0)?,
        ),
        (
            48,
            MessageRole::Response,
            fixture_string_at(&fixture, "/outputs/projection_demand_accepted", 1)?,
        ),
        (
            49,
            MessageRole::Event,
            fixture_string_at(&fixture, "/outputs/projection_blocked", 0)?,
        ),
    ] {
        let value = decode_projection_demand_control(kind, role, payload.as_bytes())?;
        let EncodedProjectionDemandControl {
            kind,
            payload,
            role,
        } = encode_projection_demand_control(&value)?;
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
            decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)?,
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
    let decoded = decode_projection_demand_control(kind, role, payload.as_bytes())?;
    let encoded = encode_projection_demand_control(&decoded)?;
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
) -> Result<ProjectionDemandControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_projection_demand_control(kind, role, &payload)
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_projection_demand_control(kind, role, payload.as_bytes()),
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
