use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    EncodedHistoryControl, Envelope, HistoryAnchorStatus, HistoryChanged, HistoryControl,
    HistoryDemand, HistoryDemandAccepted, HistoryObserved, MAXIMUM_CONTROL_JSON_BYTES, MessageRole,
    PayloadEncoding, ProtocolError, ProtocolErrorCode, SetHistoryDemand, decode_history_control,
    decode_transport_envelope, encode_history_control, encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.25.json");

#[test]
fn canonical_history_payloads_round_trip_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for payload in fixture_strings(&fixture, "/commands/history_demand")? {
        assert_canonical_round_trip(51, MessageRole::Request, payload)?;
    }
    for payload in fixture_strings(&fixture, "/outputs/history_demand_accepted")? {
        assert_canonical_round_trip(52, MessageRole::Response, payload)?;
    }
    for payload in fixture_strings(&fixture, "/outputs/history_changed")? {
        assert_canonical_round_trip(53, MessageRole::Event, payload)?;
    }

    let observed_command = fixture_string(&fixture, "/commands/history_observed")?;
    let observed_output = fixture_string(&fixture, "/outputs/history_observed")?;
    assert_eq!(observed_command, observed_output);
    assert_canonical_round_trip(54, MessageRole::Request, observed_command)?;
    assert_canonical_round_trip(54, MessageRole::Response, observed_output)?;
    Ok(())
}

#[test]
fn canonical_payloads_retain_complete_history_facts_and_direction() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let commands = fixture_strings(&fixture, "/commands/history_demand")?;
    let accepted = fixture_strings(&fixture, "/outputs/history_demand_accepted")?;
    for ((command, response), expected) in commands.into_iter().zip(accepted).zip([
        HistoryDemand::FollowingBottom,
        HistoryDemand::ViewingHistory,
    ]) {
        let request = decode_history_control(51, MessageRole::Request, command.as_bytes())?;
        let HistoryControl::SetHistoryDemand(request) = request else {
            return Err(fixture_error("history demand decoded to the wrong variant").into());
        };
        assert_eq!(request.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(request.demand, expected);

        let response = decode_history_control(52, MessageRole::Response, response.as_bytes())?;
        assert_eq!(
            response,
            HistoryControl::HistoryDemandAccepted(HistoryDemandAccepted {
                attachment_id: request.attachment_id,
                demand: request.demand,
            })
        );
    }

    let changed = fixture_strings(&fixture, "/outputs/history_changed")?;
    for ((payload, expected_status), expected_sequence) in changed
        .into_iter()
        .zip([
            HistoryAnchorStatus::Evicted,
            HistoryAnchorStatus::None,
            HistoryAnchorStatus::Valid,
        ])
        .zip(1_u64..=3)
    {
        let event = decode_history_control(53, MessageRole::Event, payload.as_bytes())?;
        let HistoryControl::HistoryChanged(event) = event else {
            return Err(fixture_error("history change decoded to the wrong variant").into());
        };
        assert_eq!(event.anchor_status, expected_status);
        assert_eq!(event.attachment_id.to_text(), "22".repeat(16));
        assert_eq!(event.generation.get(), 3);
        assert_eq!(event.revision, 42);
        assert_eq!(event.sequence.get(), expected_sequence);
        assert_eq!(event.total_rows, 125);
    }

    let observed = fixture_string(&fixture, "/commands/history_observed")?;
    let request = decode_history_control(54, MessageRole::Request, observed.as_bytes())?;
    let HistoryControl::HistoryObservedRequest(observation) = request else {
        return Err(
            fixture_error("history observation decoded to the wrong request variant").into(),
        );
    };
    assert_eq!(observation.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(observation.generation.get(), 3);
    assert_eq!(observation.revision, 42);
    assert_eq!(observation.sequence.get(), 5);
    assert_eq!(
        decode_history_control(54, MessageRole::Response, observed.as_bytes())?,
        HistoryControl::HistoryObservedResponse(observation)
    );
    Ok(())
}

#[test]
fn history_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_history_control(51, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(
        51,
        MessageRole::Request,
        "{",
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_history_control(51, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(
        51,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let demand = fixture_string_at(&fixture, "/commands/history_demand", 0)?;
    let unknown = demand.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        51,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = demand.replacen(
        "\"demand\":",
        "\"demand\":\"viewing-history\",\"demand\":",
        1,
    );
    assert_payload_code(
        51,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = demand.replacen(
        "\"demand\":",
        "\"demand\":\"viewing-history\",\"dem\\u0061nd\":",
        1,
    );
    assert_payload_code(
        51,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let changed = fixture_string_at(&fixture, "/outputs/history_changed", 0)?;
    let mut missing: Value = serde_json::from_str(changed)?;
    object_mut(&mut missing)?.remove("anchorStatus");
    assert_value_code(
        53,
        MessageRole::Event,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn history_identity_and_decimal_uint64_refinements_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let demand = fixture_string_at(&fixture, "/commands/history_demand", 0)?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(demand)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(
            51,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let changed = fixture_string_at(&fixture, "/outputs/history_changed", 0)?;
    for pointer in ["/generation", "/sequence"] {
        for invalid in ["0", "00", "-1", "+1", "18446744073709551616", " 1"] {
            let mut value: Value = serde_json::from_str(changed)?;
            set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
            assert_value_code(
                53,
                MessageRole::Event,
                value,
                ProtocolErrorCode::InvalidField,
            );
        }
    }
    for pointer in ["/revision", "/totalRows"] {
        for invalid in ["00", "-1", "+1", "18446744073709551616", " 1"] {
            let mut value: Value = serde_json::from_str(changed)?;
            set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
            assert_value_code(
                53,
                MessageRole::Event,
                value,
                ProtocolErrorCode::InvalidField,
            );
        }
    }

    let maximum = u64::MAX.to_string();
    let mut boundary: Value = serde_json::from_str(changed)?;
    for pointer in ["/generation", "/revision", "/sequence", "/totalRows"] {
        set_pointer(&mut boundary, pointer, Value::String(maximum.clone()))?;
    }
    assert!(decode_value(53, MessageRole::Event, boundary).is_ok());

    let mut zeroes: Value = serde_json::from_str(changed)?;
    for pointer in ["/revision", "/totalRows"] {
        set_pointer(&mut zeroes, pointer, Value::String("0".to_owned()))?;
    }
    assert!(decode_value(53, MessageRole::Event, zeroes).is_ok());

    let mut numeric: Value = serde_json::from_str(changed)?;
    set_pointer(&mut numeric, "/revision", Value::from(42))?;
    assert_value_code(
        53,
        MessageRole::Event,
        numeric,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn history_demand_and_anchor_status_vocabularies_are_closed() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let demand = fixture_string_at(&fixture, "/commands/history_demand", 0)?;
    for invalid in ["", "following_bottom", "VIEWING-HISTORY", "future"] {
        let mut value: Value = serde_json::from_str(demand)?;
        set_pointer(&mut value, "/demand", Value::String(invalid.to_owned()))?;
        assert_value_code(
            51,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }

    let changed = fixture_string_at(&fixture, "/outputs/history_changed", 0)?;
    for invalid in ["", "EVICTED", "valid ", "future"] {
        let mut value: Value = serde_json::from_str(changed)?;
        set_pointer(
            &mut value,
            "/anchorStatus",
            Value::String(invalid.to_owned()),
        )?;
        assert_value_code(
            53,
            MessageRole::Event,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn public_history_values_encode_every_closed_variant() -> Result<(), Box<dyn Error>> {
    let attachment_id = orchbee_terminal_protocol::AttachmentId::parse(&"22".repeat(16))?;
    let maximum = NonZeroU64::new(u64::MAX)
        .ok_or_else(|| fixture_error("maximum unsigned integer is unexpectedly zero"))?;

    for demand in [
        HistoryDemand::FollowingBottom,
        HistoryDemand::ViewingHistory,
    ] {
        for value in [
            HistoryControl::SetHistoryDemand(SetHistoryDemand {
                attachment_id,
                demand,
            }),
            HistoryControl::HistoryDemandAccepted(HistoryDemandAccepted {
                attachment_id,
                demand,
            }),
        ] {
            assert_public_round_trip(value)?;
        }
    }

    for anchor_status in [
        HistoryAnchorStatus::Evicted,
        HistoryAnchorStatus::None,
        HistoryAnchorStatus::Valid,
    ] {
        assert_public_round_trip(HistoryControl::HistoryChanged(HistoryChanged {
            anchor_status,
            attachment_id,
            generation: maximum,
            revision: u64::MAX,
            sequence: maximum,
            total_rows: u64::MAX,
        }))?;
    }

    let observed = HistoryObserved {
        attachment_id,
        generation: maximum,
        revision: u64::MAX,
        sequence: maximum,
    };
    for value in [
        HistoryControl::HistoryObservedRequest(observed),
        HistoryControl::HistoryObservedResponse(observed),
    ] {
        assert_public_round_trip(value)?;
    }
    Ok(())
}

#[test]
fn history_kind_role_and_control_json_boundaries_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let demand = fixture_string_at(&fixture, "/commands/history_demand", 0)?;
    for (kind, role) in [
        (50, MessageRole::Request),
        (51, MessageRole::Response),
        (51, MessageRole::Event),
        (52, MessageRole::Request),
        (52, MessageRole::Event),
        (53, MessageRole::Request),
        (53, MessageRole::Response),
        (54, MessageRole::Event),
        (55, MessageRole::Event),
    ] {
        assert_payload_code(kind, role, demand, ProtocolErrorCode::InvalidField);
    }

    let prefix_length = MAXIMUM_CONTROL_JSON_BYTES
        .checked_sub(demand.len())
        .ok_or_else(|| fixture_error("canonical history fixture exceeds the control bound"))?;
    let mut exact = vec![b' '; prefix_length];
    exact.extend_from_slice(demand.as_bytes());
    assert!(decode_history_control(51, MessageRole::Request, &exact).is_ok());
    exact.insert(0, b' ');
    assert_code(
        decode_history_control(51, MessageRole::Request, &exact),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn history_control_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, payload) in [
        (
            51,
            MessageRole::Request,
            fixture_string_at(&fixture, "/commands/history_demand", 0)?,
        ),
        (
            52,
            MessageRole::Response,
            fixture_string_at(&fixture, "/outputs/history_demand_accepted", 1)?,
        ),
        (
            53,
            MessageRole::Event,
            fixture_string_at(&fixture, "/outputs/history_changed", 2)?,
        ),
        (
            54,
            MessageRole::Request,
            fixture_string(&fixture, "/commands/history_observed")?,
        ),
        (
            54,
            MessageRole::Response,
            fixture_string(&fixture, "/outputs/history_observed")?,
        ),
    ] {
        let value = decode_history_control(kind, role, payload.as_bytes())?;
        let EncodedHistoryControl {
            kind,
            payload,
            role,
        } = encode_history_control(&value)?;
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
            decode_history_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn assert_public_round_trip(value: HistoryControl) -> Result<(), ProtocolError> {
    let encoded = encode_history_control(&value)?;
    assert_eq!(
        decode_history_control(encoded.kind, encoded.role, &encoded.payload)?,
        value
    );
    Ok(())
}

fn assert_canonical_round_trip(
    kind: u16,
    role: MessageRole,
    payload: &str,
) -> Result<(), Box<dyn Error>> {
    let decoded = decode_history_control(kind, role, payload.as_bytes())?;
    let encoded = encode_history_control(&decoded)?;
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
) -> Result<HistoryControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_history_control(kind, role, &payload)
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_history_control(kind, role, payload.as_bytes()),
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
