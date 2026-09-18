use std::{error::Error, io};

use orchbee_terminal_protocol::{
    EncodedLifecycleControl, Envelope, ExitResult, LifecycleControl, MAXIMUM_CONTROL_JSON_BYTES,
    MessageRole, PayloadEncoding, ProcessObservation, ProtocolError, ProtocolErrorCode,
    RuntimeFailure, StartTransition, TerminalId, TerminalPixelGeometry, TerminalTarget,
    decode_lifecycle_control, decode_transport_envelope, encode_lifecycle_control,
    encode_transport_envelope,
};
use serde_json::{Map, Value};

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.24.json");

#[test]
fn host_lifecycle_acknowledgements_match_the_canonical_wire_bytes() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let acknowledged = fixture_string(&fixture, "/outputs/terminal_acknowledged")?;
    for kind in [12, 13, 18] {
        assert_canonical_round_trip(kind, MessageRole::Response, acknowledged)?;
    }
    Ok(())
}

#[test]
fn acknowledgement_targets_remain_available_for_pending_request_matching()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let acknowledged = fixture_string(&fixture, "/outputs/terminal_acknowledged")?;
    let pending_target = TerminalId::parse(&"44".repeat(16))?;
    for kind in [12, 13, 18] {
        for terminal_id in [
            "00000000000000000000000000000001",
            "44444444444444444444444444444444",
            "ffffffffffffffffffffffffffffffff",
        ] {
            let payload = acknowledged.replace(&pending_target.to_text(), terminal_id);
            let decoded =
                decode_lifecycle_control(kind, MessageRole::Response, payload.as_bytes())?;
            let target = match decoded {
                LifecycleControl::TerminateTerminalAccepted(target)
                | LifecycleControl::DisposeTerminalAccepted(target)
                | LifecycleControl::ForceTerminateTerminalAccepted(target) => target,
                _ => {
                    return Err(
                        fixture_error("acknowledgement decoded to the wrong variant").into(),
                    );
                }
            };
            assert_eq!(target.terminal_id, TerminalId::parse(terminal_id)?);
            assert_eq!(
                target.terminal_id == pending_target,
                terminal_id == pending_target.to_text()
            );
            assert_canonical_round_trip(kind, MessageRole::Response, &payload)?;
        }
    }
    Ok(())
}

#[test]
fn lifecycle_acknowledgements_require_one_canonical_target() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let acknowledged = fixture_string(&fixture, "/outputs/terminal_acknowledged")?;
    let unknown = acknowledged.replacen('}', ",\"extra\":true}", 1);
    let duplicate = acknowledged.replacen(
        "\"terminalId\":",
        "\"terminalId\":\"44444444444444444444444444444444\",\"terminalId\":",
        1,
    );
    let escaped_duplicate = duplicate.replacen("\"terminalId\":", "\"terminal\\u0049d\":", 1);
    for kind in [12, 13, 18] {
        for payload in [
            "{}",
            "{\"terminalId\":null}",
            "{\"terminalId\":1}",
            &unknown,
        ] {
            assert_payload_code(
                kind,
                MessageRole::Response,
                payload,
                ProtocolErrorCode::InvalidField,
            );
        }
        for payload in [&duplicate, &escaped_duplicate] {
            assert_payload_code(
                kind,
                MessageRole::Response,
                payload,
                ProtocolErrorCode::InvalidJson,
            );
        }
        for identity in [
            "00".repeat(16),
            "AA".repeat(16),
            "1".repeat(31),
            "1".repeat(33),
        ] {
            let payload = acknowledged.replacen(&"44".repeat(16), &identity, 1);
            assert_payload_code(
                kind,
                MessageRole::Response,
                &payload,
                ProtocolErrorCode::InvalidIdentifier,
            );
        }
    }
    Ok(())
}

#[test]
fn every_canonical_lifecycle_payload_decodes_and_reencodes_byte_for_byte()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let target = fixture_string(&fixture, "/commands/terminal_target")?;
    for kind in [12, 13, 18] {
        assert_canonical_round_trip(kind, MessageRole::Request, target)?;
    }

    assert_canonical_round_trip(
        14,
        MessageRole::Event,
        fixture_string(&fixture, "/outputs/terminal_started")?,
    )?;
    assert_fixture_array_round_trips(&fixture, "/outputs/terminal_exited", 15, MessageRole::Event)?;
    assert_canonical_round_trip(
        16,
        MessageRole::Request,
        fixture_string(&fixture, "/commands/start_terminal")?,
    )?;
    assert_fixture_array_round_trips(
        &fixture,
        "/outputs/terminal_start_accepted",
        17,
        MessageRole::Response,
    )?;
    assert_fixture_array_round_trips(&fixture, "/outputs/terminal_failed", 19, MessageRole::Event)?;
    Ok(())
}

#[test]
fn canonical_payloads_retain_typed_lifecycle_facts() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let start = decode_lifecycle_control(
        16,
        MessageRole::Request,
        fixture_string(&fixture, "/commands/start_terminal")?.as_bytes(),
    )?;
    let LifecycleControl::StartTerminal(start) = start else {
        return Err(fixture_error("start payload decoded to the wrong variant").into());
    };
    assert_eq!(
        start.geometry,
        TerminalPixelGeometry {
            cell_pixel_height: 18,
            cell_pixel_width: 9,
            columns: 96,
            rows: 32,
        }
    );
    assert_eq!(start.terminal_id.to_text(), "44".repeat(16));

    let started = decode_lifecycle_control(
        14,
        MessageRole::Event,
        fixture_string(&fixture, "/outputs/terminal_started")?.as_bytes(),
    )?;
    let LifecycleControl::TerminalStarted(identity) = started else {
        return Err(fixture_error("started payload decoded to the wrong variant").into());
    };
    assert_eq!(identity.creation_id.to_text(), "33".repeat(16));
    assert_eq!(identity.host_epoch.to_text(), "11".repeat(16));
    assert_eq!(identity.terminal_id, start.terminal_id);

    let accepted = fixture_array(&fixture, "/outputs/terminal_start_accepted")?;
    let expected_transitions = [StartTransition::Started, StartTransition::AlreadyStarted];
    for (payload, expected) in accepted.iter().zip(expected_transitions) {
        let decoded =
            decode_lifecycle_control(17, MessageRole::Response, fixture_text(payload)?.as_bytes())?;
        let LifecycleControl::TerminalStartAccepted(value) = decoded else {
            return Err(fixture_error("start acceptance decoded to the wrong variant").into());
        };
        assert_eq!(value.transition, expected);
        assert_eq!(value.geometry, start.geometry);
        assert_eq!(value.identity, identity);
    }

    let exited = fixture_array(&fixture, "/outputs/terminal_exited")?;
    let expected_exits = [
        ExitResult::ExitCode(0),
        ExitResult::Signal(15),
        ExitResult::Unknown(7),
    ];
    for (payload, expected) in exited.iter().zip(expected_exits) {
        let decoded =
            decode_lifecycle_control(15, MessageRole::Event, fixture_text(payload)?.as_bytes())?;
        assert!(matches!(
            decoded,
            LifecycleControl::TerminalExited(value)
                if value.exit == expected && value.final_revision == 42 && value.identity == identity
        ));
    }
    Ok(())
}

#[test]
fn equal_wire_shapes_preserve_the_selected_operation() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let target = fixture_string(&fixture, "/commands/terminal_target")?;
    let acknowledged = fixture_string(&fixture, "/outputs/terminal_acknowledged")?;
    let expected_target = TerminalTarget {
        terminal_id: TerminalId::parse(&"44".repeat(16))?,
    };

    let terminate = decode_lifecycle_control(12, MessageRole::Request, target.as_bytes())?;
    let dispose = decode_lifecycle_control(13, MessageRole::Request, target.as_bytes())?;
    let force = decode_lifecycle_control(18, MessageRole::Request, target.as_bytes())?;
    assert!(matches!(terminate, LifecycleControl::TerminateTerminal(_)));
    assert!(matches!(dispose, LifecycleControl::DisposeTerminal(_)));
    assert!(matches!(force, LifecycleControl::ForceTerminateTerminal(_)));
    assert_ne!(terminate, dispose);
    assert_ne!(dispose, force);

    assert_eq!(
        decode_lifecycle_control(12, MessageRole::Response, acknowledged.as_bytes())?,
        LifecycleControl::TerminateTerminalAccepted(expected_target)
    );
    assert_eq!(
        decode_lifecycle_control(13, MessageRole::Response, acknowledged.as_bytes())?,
        LifecycleControl::DisposeTerminalAccepted(expected_target)
    );
    assert_eq!(
        decode_lifecycle_control(18, MessageRole::Response, acknowledged.as_bytes())?,
        LifecycleControl::ForceTerminateTerminalAccepted(expected_target)
    );
    Ok(())
}

#[test]
fn lifecycle_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_lifecycle_control(12, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_code(
        decode_lifecycle_control(12, MessageRole::Request, b"{"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_lifecycle_control(12, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );

    let fixture = read_fixture()?;
    let target = fixture_string(&fixture, "/commands/terminal_target")?;
    let unknown = target.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        12,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    assert_payload_code(
        12,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = target.replacen(
        "\"terminalId\":",
        "\"terminalId\":\"44444444444444444444444444444444\",\"terminalId\":",
        1,
    );
    assert_payload_code(
        12,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = target.replacen(
        "\"terminalId\":",
        "\"terminalId\":\"44444444444444444444444444444444\",\"terminal\\u0049d\":",
        1,
    );
    assert_payload_code(
        12,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let start = fixture_string(&fixture, "/commands/start_terminal")?;
    let mut missing: Value = serde_json::from_str(start)?;
    object_mut_at(&mut missing, "/geometry")?.remove("rows");
    assert_value_code(
        16,
        MessageRole::Request,
        missing,
        ProtocolErrorCode::InvalidField,
    );
    let mut nested_unknown: Value = serde_json::from_str(start)?;
    object_mut_at(&mut nested_unknown, "/geometry")?.insert("extra".to_owned(), Value::Bool(true));
    assert_value_code(
        16,
        MessageRole::Request,
        nested_unknown,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn lifecycle_identities_and_revisions_are_canonical() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let target = fixture_string(&fixture, "/commands/terminal_target")?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let payload = target.replacen(&"44".repeat(16), &identity, 1);
        assert_payload_code(
            12,
            MessageRole::Request,
            &payload,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let started = fixture_string(&fixture, "/outputs/terminal_started")?;
    for pointer in ["/creationId", "/hostEpoch", "/terminalId"] {
        let mut value: Value = serde_json::from_str(started)?;
        set_pointer(&mut value, pointer, Value::String("00".repeat(16)))?;
        assert_value_code(
            14,
            MessageRole::Event,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let exited = fixture_array(&fixture, "/outputs/terminal_exited")?;
    let canonical = fixture_text(
        exited
            .first()
            .ok_or_else(|| fixture_error("terminal-exited fixture is empty"))?,
    )?;
    for revision in ["00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(canonical)?;
        set_pointer(
            &mut value,
            "/finalRevision",
            Value::String(revision.to_owned()),
        )?;
        assert_value_code(
            15,
            MessageRole::Event,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    let mut numeric: Value = serde_json::from_str(canonical)?;
    set_pointer(&mut numeric, "/finalRevision", Value::from(42))?;
    assert_value_code(
        15,
        MessageRole::Event,
        numeric,
        ProtocolErrorCode::InvalidField,
    );
    for revision in ["0".to_owned(), u64::MAX.to_string()] {
        let mut value: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut value, "/finalRevision", Value::String(revision))?;
        assert!(decode_value(15, MessageRole::Event, value).is_ok());
    }
    Ok(())
}

#[test]
fn full_pixel_geometry_enforces_numeric_grid_and_extent_bounds() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/commands/start_terminal")?;

    let rounded = canonical.replacen("\"columns\":96", "\"columns\":1.0000000000000001", 1);
    let decoded = decode_lifecycle_control(16, MessageRole::Request, rounded.as_bytes())?;
    assert!(matches!(
        decoded,
        LifecycleControl::StartTerminal(value) if value.geometry.columns == 1
    ));

    for (pointer, replacement) in [
        ("/geometry/cellPixelHeight", Value::from(0)),
        ("/geometry/cellPixelWidth", Value::from(4097)),
        ("/geometry/columns", Value::from(0)),
        ("/geometry/rows", Value::from(4097)),
        ("/geometry/columns", Value::from(9_007_199_254_740_992_u64)),
    ] {
        let mut value: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut value, pointer, replacement)?;
        assert_value_code(
            16,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    for token in ["1.5", "-1", "1e4000"] {
        let payload = canonical.replacen("\"columns\":96", &format!("\"columns\":{token}"), 1);
        assert_payload_code(
            16,
            MessageRole::Request,
            &payload,
            ProtocolErrorCode::InvalidField,
        );
    }

    assert_geometry(&fixture, 4096, 64, 1, 1, true)?;
    assert_geometry(&fixture, 4096, 65, 1, 1, false)?;
    assert_geometry(&fixture, 255, 257, 257, 255, true)?;
    assert_geometry(&fixture, 4096, 1, 16, 1, false)?;
    assert_geometry(&fixture, 1, 4096, 1, 16, false)?;
    assert_geometry(&fixture, 80, 24, 0, 0, true)?;
    assert_geometry(&fixture, 80, 24, 0, 16, false)?;
    assert_geometry(&fixture, 80, 24, 8, 0, false)?;
    Ok(())
}

#[test]
fn runtime_failure_exit_and_process_variants_are_closed() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let failures = fixture_array(&fixture, "/outputs/terminal_failed")?;
    let expected = [
        RuntimeFailure::DispatchTermination,
        RuntimeFailure::ObserveProcess,
        RuntimeFailure::ReadOutput,
        RuntimeFailure::WriteInput,
        RuntimeFailure::ApplyResize,
        RuntimeFailure::AdmitTerminalReply,
        RuntimeFailure::SynchronizeWorker,
    ];
    for ((payload, expected_failure), expected_revision) in
        failures.iter().zip(expected).zip(42_u64..)
    {
        let decoded =
            decode_lifecycle_control(19, MessageRole::Event, fixture_text(payload)?.as_bytes())?;
        let LifecycleControl::TerminalFailed(value) = decoded else {
            return Err(fixture_error("terminal failure decoded to the wrong variant").into());
        };
        assert_eq!(value.failure, expected_failure);
        assert_eq!(value.final_revision, expected_revision);
        if expected_failure == RuntimeFailure::ObserveProcess {
            assert_eq!(value.process, ProcessObservation::Unavailable);
        }
    }

    let first = fixture_text(
        failures
            .first()
            .ok_or_else(|| fixture_error("terminal-failed fixture is empty"))?,
    )?;
    let mut mismatched: Value = serde_json::from_str(first)?;
    set_pointer(
        &mut mismatched,
        "/failure/stage",
        Value::String("read-output".to_owned()),
    )?;
    assert_value_code(
        19,
        MessageRole::Event,
        mismatched,
        ProtocolErrorCode::InvalidField,
    );

    let unavailable = fixture_text(
        failures
            .get(1)
            .ok_or_else(|| fixture_error("unavailable-process fixture is missing"))?,
    )?;
    let mut wrong_unavailable: Value = serde_json::from_str(unavailable)?;
    set_pointer(
        &mut wrong_unavailable,
        "/process/failure/stage",
        Value::String("release-process-identity".to_owned()),
    )?;
    assert_value_code(
        19,
        MessageRole::Event,
        wrong_unavailable,
        ProtocolErrorCode::InvalidField,
    );

    let exited = fixture_array(&fixture, "/outputs/terminal_exited")?;
    let signal = fixture_text(
        exited
            .get(1)
            .ok_or_else(|| fixture_error("signal-exit fixture is missing"))?,
    )?;
    let mut zero_signal: Value = serde_json::from_str(signal)?;
    set_pointer(&mut zero_signal, "/exit/signal", Value::from(0))?;
    assert_value_code(
        15,
        MessageRole::Event,
        zero_signal,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn public_lifecycle_values_are_revalidated_before_encoding() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let decoded = decode_lifecycle_control(
        16,
        MessageRole::Request,
        fixture_string(&fixture, "/commands/start_terminal")?.as_bytes(),
    )?;
    let LifecycleControl::StartTerminal(mut start) = decoded else {
        return Err(fixture_error("start payload decoded to the wrong variant").into());
    };
    start.geometry.columns = 0;
    assert_code(
        encode_lifecycle_control(&LifecycleControl::StartTerminal(start)),
        ProtocolErrorCode::InvalidField,
    );

    let exited = fixture_array(&fixture, "/outputs/terminal_exited")?;
    let decoded = decode_lifecycle_control(
        15,
        MessageRole::Event,
        fixture_text(
            exited
                .get(1)
                .ok_or_else(|| fixture_error("signal-exit fixture is missing"))?,
        )?
        .as_bytes(),
    )?;
    let LifecycleControl::TerminalExited(mut exited) = decoded else {
        return Err(fixture_error("exit payload decoded to the wrong variant").into());
    };
    exited.exit = ExitResult::Signal(0);
    assert_code(
        encode_lifecycle_control(&LifecycleControl::TerminalExited(exited)),
        ProtocolErrorCode::InvalidField,
    );

    let failures = fixture_array(&fixture, "/outputs/terminal_failed")?;
    let decoded = decode_lifecycle_control(
        19,
        MessageRole::Event,
        fixture_text(
            failures
                .first()
                .ok_or_else(|| fixture_error("terminal-failed fixture is empty"))?,
        )?
        .as_bytes(),
    )?;
    let LifecycleControl::TerminalFailed(mut failed) = decoded else {
        return Err(fixture_error("failure payload decoded to the wrong variant").into());
    };
    failed.process = ProcessObservation::Observed(ExitResult::Signal(0));
    assert_code(
        encode_lifecycle_control(&LifecycleControl::TerminalFailed(failed)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn lifecycle_kind_role_and_payload_bounds_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let target = fixture_string(&fixture, "/commands/terminal_target")?;
    for (kind, role) in [
        (11, MessageRole::Request),
        (12, MessageRole::Event),
        (14, MessageRole::Request),
        (15, MessageRole::Response),
        (16, MessageRole::Response),
        (17, MessageRole::Request),
        (18, MessageRole::Event),
        (19, MessageRole::Response),
        (20, MessageRole::Request),
    ] {
        assert_payload_code(kind, role, target, ProtocolErrorCode::InvalidField);
    }
    assert_payload_code(
        12,
        MessageRole::Request,
        fixture_string(&fixture, "/outputs/empty")?,
        ProtocolErrorCode::InvalidField,
    );

    let oversized = vec![b' '; MAXIMUM_CONTROL_JSON_BYTES + 1];
    assert_code(
        decode_lifecycle_control(12, MessageRole::Request, &oversized),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn lifecycle_control_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let cases = [
        (
            16,
            MessageRole::Request,
            fixture_string(&fixture, "/commands/start_terminal")?,
        ),
        (
            17,
            MessageRole::Response,
            fixture_text(
                fixture_array(&fixture, "/outputs/terminal_start_accepted")?
                    .first()
                    .ok_or_else(|| fixture_error("start-accepted fixture is empty"))?,
            )?,
        ),
        (
            19,
            MessageRole::Event,
            fixture_text(
                fixture_array(&fixture, "/outputs/terminal_failed")?
                    .first()
                    .ok_or_else(|| fixture_error("terminal-failed fixture is empty"))?,
            )?,
        ),
    ];

    for (kind, role, payload) in cases {
        let value = decode_lifecycle_control(kind, role, payload.as_bytes())?;
        let EncodedLifecycleControl {
            kind,
            payload,
            role,
        } = encode_lifecycle_control(&value)?;
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
            decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload)?,
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
    let decoded = decode_lifecycle_control(kind, role, payload.as_bytes())?;
    let encoded = encode_lifecycle_control(&decoded)?;
    assert_eq!(encoded.kind, kind);
    assert_eq!(encoded.role, role);
    assert_eq!(encoded.payload, payload.as_bytes());
    Ok(())
}

fn assert_fixture_array_round_trips(
    fixture: &Value,
    pointer: &str,
    kind: u16,
    role: MessageRole,
) -> Result<(), Box<dyn Error>> {
    for payload in fixture_array(fixture, pointer)? {
        assert_canonical_round_trip(kind, role, fixture_text(payload)?)?;
    }
    Ok(())
}

fn assert_geometry(
    fixture: &Value,
    columns: u64,
    rows: u64,
    cell_pixel_width: u64,
    cell_pixel_height: u64,
    valid: bool,
) -> Result<(), Box<dyn Error>> {
    let mut value: Value =
        serde_json::from_str(fixture_string(fixture, "/commands/start_terminal")?)?;
    set_pointer(&mut value, "/geometry/columns", Value::from(columns))?;
    set_pointer(&mut value, "/geometry/rows", Value::from(rows))?;
    set_pointer(
        &mut value,
        "/geometry/cellPixelWidth",
        Value::from(cell_pixel_width),
    )?;
    set_pointer(
        &mut value,
        "/geometry/cellPixelHeight",
        Value::from(cell_pixel_height),
    )?;
    let result = decode_value(16, MessageRole::Request, value);
    if valid {
        result?;
    } else {
        assert_code(result, ProtocolErrorCode::InvalidField);
    }
    Ok(())
}

fn decode_value(
    kind: u16,
    role: MessageRole,
    value: Value,
) -> Result<LifecycleControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_lifecycle_control(kind, role, &payload)
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_lifecycle_control(kind, role, payload.as_bytes()),
        expected,
    );
}

fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| fixture_error(&format!("fixture pointer {pointer} is missing")))?;
    *target = replacement;
    Ok(())
}

fn object_mut_at<'value>(
    value: &'value mut Value,
    pointer: &str,
) -> Result<&'value mut Map<String, Value>, io::Error> {
    value
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| fixture_error(&format!("fixture object {pointer} is missing")))
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

fn fixture_array<'value>(
    value: &'value Value,
    pointer: &str,
) -> Result<&'value Vec<Value>, io::Error> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| fixture_error(&format!("fixture array {pointer} is missing")))
}

fn fixture_text(value: &Value) -> Result<&str, io::Error> {
    value
        .as_str()
        .ok_or_else(|| fixture_error("fixture array member is not text"))
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
