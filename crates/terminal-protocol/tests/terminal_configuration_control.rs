use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    CorrelationId, EncodedTerminalConfigurationControl, Envelope, HostEpoch,
    MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_SCROLLBACK_BYTES, MessageRole, PayloadEncoding,
    ProtocolError, ProtocolErrorCode, SetTerminalDefaults, TerminalConfiguration,
    TerminalConfigurationAccepted, TerminalConfigurationControl, TerminalDefaultsAccepted,
    TerminalId, UpdateTerminalConfiguration, decode_terminal_configuration_control,
    decode_transport_envelope, encode_terminal_configuration_control, encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.24.json");

#[test]
fn canonical_configuration_controls_round_trip_in_every_direction() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, pointer) in [
        (70, MessageRole::Request, "/commands/set_terminal_defaults"),
        (
            71,
            MessageRole::Response,
            "/outputs/terminal_defaults_accepted",
        ),
        (
            72,
            MessageRole::Request,
            "/commands/update_terminal_configuration",
        ),
        (
            73,
            MessageRole::Response,
            "/outputs/terminal_configuration_accepted",
        ),
    ] {
        let payload = fixture_string(&fixture, pointer)?;
        let decoded = decode_terminal_configuration_control(kind, role, payload.as_bytes())?;
        let encoded: EncodedTerminalConfigurationControl =
            encode_terminal_configuration_control(&decoded)?;
        assert_eq!(encoded.kind, kind);
        assert_eq!(encoded.role, role);
        assert_eq!(encoded.payload, payload.as_bytes());
    }
    Ok(())
}

#[test]
fn canonical_controls_retain_complete_configuration_facts() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let defaults = fixture_string(&fixture, "/commands/set_terminal_defaults")?;
    let TerminalConfigurationControl::SetDefaultsRequest(defaults) =
        decode_terminal_configuration_control(70, MessageRole::Request, defaults.as_bytes())?
    else {
        return Err(fixture_error("defaults request decoded to the wrong variant").into());
    };
    assert_eq!(defaults.configuration.scrollback_bytes, 25_000_000);

    let accepted = fixture_string(&fixture, "/outputs/terminal_defaults_accepted")?;
    let TerminalConfigurationControl::DefaultsAcceptedResponse(accepted) =
        decode_terminal_configuration_control(71, MessageRole::Response, accepted.as_bytes())?
    else {
        return Err(fixture_error("defaults response decoded to the wrong variant").into());
    };
    assert_eq!(accepted.configuration.scrollback_bytes, 25_000_000);
    assert_eq!(accepted.revision, 3);

    let update = fixture_string(&fixture, "/commands/update_terminal_configuration")?;
    let TerminalConfigurationControl::UpdateRequest(update) =
        decode_terminal_configuration_control(72, MessageRole::Request, update.as_bytes())?
    else {
        return Err(fixture_error("configuration request decoded to the wrong variant").into());
    };
    assert_eq!(update.configuration.scrollback_bytes, 0);
    assert_eq!(update.terminal_id.to_text(), "44".repeat(16));

    let response = fixture_string(&fixture, "/outputs/terminal_configuration_accepted")?;
    let TerminalConfigurationControl::ConfigurationAcceptedResponse(response) =
        decode_terminal_configuration_control(73, MessageRole::Response, response.as_bytes())?
    else {
        return Err(fixture_error("configuration response decoded to the wrong variant").into());
    };
    assert_eq!(response.configuration.scrollback_bytes, 25_000_000);
    assert_eq!(response.configuration_revision, 3);
    assert_eq!(response.generation.get(), 4);
    assert_eq!(response.history_rows_evicted, 5);
    assert_eq!(response.revision, 43);
    assert_eq!(response.terminal_id.to_text(), "44".repeat(16));
    Ok(())
}

#[test]
fn configuration_json_is_utf8_unique_complete_and_closed_at_both_depths()
-> Result<(), Box<dyn Error>> {
    assert_code(
        decode_terminal_configuration_control(70, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(
        70,
        MessageRole::Request,
        "{",
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_terminal_configuration_control(70, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(
        70,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let payload = fixture_string(&fixture, "/commands/set_terminal_defaults")?;
    let unknown_top = payload.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        70,
        MessageRole::Request,
        &unknown_top,
        ProtocolErrorCode::InvalidField,
    );
    let unknown_nested = payload.replacen(
        "\"scrollbackBytes\":25000000",
        "\"scrollbackBytes\":25000000,\"extra\":true",
        1,
    );
    assert_payload_code(
        70,
        MessageRole::Request,
        &unknown_nested,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = payload.replacen(
        "\"scrollbackBytes\":",
        "\"scrollbackBytes\":0,\"scrollbackBytes\":",
        1,
    );
    assert_payload_code(
        70,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = payload.replacen(
        "\"configuration\":",
        "\"configuration\":{\"scrollbackBytes\":0},\"configur\\u0061tion\":",
        1,
    );
    assert_payload_code(
        70,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let mut missing: Value = serde_json::from_str(payload)?;
    object_mut(
        missing
            .get_mut("configuration")
            .ok_or_else(|| fixture_error("configuration is absent"))?,
    )?
    .remove("scrollbackBytes");
    assert_value_code(
        70,
        MessageRole::Request,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn scrollback_bytes_normalize_safely_and_obey_the_exact_bound() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let payload = fixture_string(&fixture, "/commands/set_terminal_defaults")?;
    for accepted in ["0", "1000000000", "1.0", "1e0"] {
        let candidate = replace_numeric_member(payload, "scrollbackBytes", accepted)?;
        assert!(
            decode_terminal_configuration_control(70, MessageRole::Request, candidate.as_bytes())
                .is_ok()
        );
    }
    for rejected in ["-1", "1000000001", "1.5", "9007199254740992"] {
        let candidate = replace_numeric_member(payload, "scrollbackBytes", rejected)?;
        assert_payload_code(
            70,
            MessageRole::Request,
            &candidate,
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn terminal_identity_and_decimal_counter_refinements_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let update = fixture_string(&fixture, "/commands/update_terminal_configuration")?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(update)?;
        set_pointer(&mut value, "/terminalId", Value::String(identity))?;
        assert_value_code(
            72,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let defaults = fixture_string(&fixture, "/outputs/terminal_defaults_accepted")?;
    assert_decimal_partition(71, defaults, "/revision", true)?;

    let response = fixture_string(&fixture, "/outputs/terminal_configuration_accepted")?;
    for pointer in ["/configurationRevision", "/historyRowsEvicted", "/revision"] {
        assert_decimal_partition(73, response, pointer, true)?;
    }
    assert_decimal_partition(73, response, "/generation", false)?;

    let mut unrelated: Value = serde_json::from_str(response)?;
    set_pointer(
        &mut unrelated,
        "/configurationRevision",
        Value::String(u64::MAX.to_string()),
    )?;
    set_pointer(&mut unrelated, "/generation", Value::String("1".to_owned()))?;
    set_pointer(
        &mut unrelated,
        "/historyRowsEvicted",
        Value::String(u64::MAX.to_string()),
    )?;
    set_pointer(&mut unrelated, "/revision", Value::String("0".to_owned()))?;
    assert!(decode_value(73, MessageRole::Response, unrelated).is_ok());
    Ok(())
}

#[test]
fn public_values_encode_every_direction_and_revalidate_configuration() -> Result<(), Box<dyn Error>>
{
    let terminal_id = TerminalId::parse(&"44".repeat(16))?;
    let configuration = TerminalConfiguration {
        scrollback_bytes: MAXIMUM_SCROLLBACK_BYTES,
    };
    let values = [
        TerminalConfigurationControl::SetDefaultsRequest(SetTerminalDefaults {
            configuration: configuration.clone(),
        }),
        TerminalConfigurationControl::DefaultsAcceptedResponse(TerminalDefaultsAccepted {
            configuration: configuration.clone(),
            revision: u64::MAX,
        }),
        TerminalConfigurationControl::UpdateRequest(UpdateTerminalConfiguration {
            configuration: configuration.clone(),
            terminal_id,
        }),
        TerminalConfigurationControl::ConfigurationAcceptedResponse(
            TerminalConfigurationAccepted {
                configuration: configuration.clone(),
                configuration_revision: u64::MAX,
                generation: NonZeroU64::MAX,
                history_rows_evicted: u64::MAX,
                revision: u64::MAX,
                terminal_id,
            },
        ),
    ];
    for value in values {
        let encoded = encode_terminal_configuration_control(&value)?;
        assert_eq!(
            decode_terminal_configuration_control(encoded.kind, encoded.role, &encoded.payload)?,
            value
        );
    }

    assert_code(
        encode_terminal_configuration_control(&TerminalConfigurationControl::SetDefaultsRequest(
            SetTerminalDefaults {
                configuration: TerminalConfiguration {
                    scrollback_bytes: MAXIMUM_SCROLLBACK_BYTES + 1,
                },
            },
        )),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn configuration_control_kind_role_and_aggregate_bounds_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let cases = [
        (70, MessageRole::Request, "/commands/set_terminal_defaults"),
        (
            71,
            MessageRole::Response,
            "/outputs/terminal_defaults_accepted",
        ),
        (
            72,
            MessageRole::Request,
            "/commands/update_terminal_configuration",
        ),
        (
            73,
            MessageRole::Response,
            "/outputs/terminal_configuration_accepted",
        ),
    ];
    for (kind, legal_role, pointer) in cases {
        let payload = fixture_string(&fixture, pointer)?;
        for role in [
            MessageRole::Request,
            MessageRole::Response,
            MessageRole::Event,
        ] {
            if role == legal_role {
                assert!(
                    decode_terminal_configuration_control(kind, role, payload.as_bytes()).is_ok()
                );
            } else {
                assert_payload_code(kind, role, payload, ProtocolErrorCode::InvalidField);
            }
        }
    }
    for kind in [69, 74] {
        assert_payload_code(
            kind,
            MessageRole::Request,
            "{}",
            ProtocolErrorCode::InvalidField,
        );
    }

    let payload = fixture_string(&fixture, "/commands/set_terminal_defaults")?;
    let exact = format!(
        "{payload}{}",
        " ".repeat(MAXIMUM_CONTROL_JSON_BYTES - payload.len())
    );
    assert_eq!(exact.len(), MAXIMUM_CONTROL_JSON_BYTES);
    assert!(
        decode_terminal_configuration_control(70, MessageRole::Request, exact.as_bytes()).is_ok()
    );
    let over = format!("{exact} ");
    assert_code(
        decode_terminal_configuration_control(70, MessageRole::Request, over.as_bytes()),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn configuration_controls_compose_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, pointer) in [
        (70, MessageRole::Request, "/commands/set_terminal_defaults"),
        (
            71,
            MessageRole::Response,
            "/outputs/terminal_defaults_accepted",
        ),
        (
            72,
            MessageRole::Request,
            "/commands/update_terminal_configuration",
        ),
        (
            73,
            MessageRole::Response,
            "/outputs/terminal_configuration_accepted",
        ),
    ] {
        let value = decode_terminal_configuration_control(
            kind,
            role,
            fixture_string(&fixture, pointer)?.as_bytes(),
        )?;
        let encoded = encode_terminal_configuration_control(&value)?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: Some(CorrelationId::parse(&"55".repeat(16))?),
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
            decode_terminal_configuration_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn assert_decimal_partition(
    kind: u16,
    payload: &str,
    pointer: &str,
    zero_allowed: bool,
) -> Result<(), Box<dyn Error>> {
    let mut maximum: Value = serde_json::from_str(payload)?;
    set_pointer(&mut maximum, pointer, Value::String(u64::MAX.to_string()))?;
    assert!(decode_value(kind, MessageRole::Response, maximum).is_ok());

    let mut zero: Value = serde_json::from_str(payload)?;
    set_pointer(&mut zero, pointer, Value::String("0".to_owned()))?;
    if zero_allowed {
        assert!(decode_value(kind, MessageRole::Response, zero).is_ok());
    } else {
        assert_value_code(
            kind,
            MessageRole::Response,
            zero,
            ProtocolErrorCode::InvalidField,
        );
    }

    for invalid in ["00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(payload)?;
        set_pointer(&mut value, pointer, Value::String(invalid.to_owned()))?;
        assert_value_code(
            kind,
            MessageRole::Response,
            value,
            ProtocolErrorCode::InvalidField,
        );
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
        .ok_or_else(|| fixture_error(&format!("fixture string {pointer} is missing")))
}

fn decode_value(
    kind: u16,
    role: MessageRole,
    value: Value,
) -> Result<TerminalConfigurationControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_terminal_configuration_control(kind, role, &payload)
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
        decode_terminal_configuration_control(kind, role, payload.as_bytes()),
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
