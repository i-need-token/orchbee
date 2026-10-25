use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    EncodedInventoryControl, Envelope, ExitResult, FailureContainment, InventoryControl,
    MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_INVENTORY_PAGE_TERMINALS, MessageRole, PayloadEncoding,
    ProcessObservation, ProtocolError, ProtocolErrorCode, RuntimeCleanupFailure, RuntimeFailure,
    TerminalInventoryCursor, TerminalList, TerminalSummary, decode_inventory_control,
    decode_transport_envelope, encode_inventory_control, encode_transport_envelope,
};
use serde_json::{Map, Value};

const CONTROL_FIXTURE: &str =
    include_str!("../../../schemas/terminal/v1/fixtures/control-conformance.json");

#[test]
fn inventory_close_activity_is_optional_closed_and_never_inferred_from_phase()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let raw = fixture_string(&fixture, "/outputs/terminal_list")?;
    for (text, activity) in [
        (
            "idle",
            orchbee_terminal_protocol::TerminalCloseActivity::Idle,
        ),
        (
            "active",
            orchbee_terminal_protocol::TerminalCloseActivity::Active,
        ),
        (
            "unknown",
            orchbee_terminal_protocol::TerminalCloseActivity::Unknown,
        ),
    ] {
        let mut value: Value = serde_json::from_str(raw)?;
        value["terminals"][1]["closeActivity"] = text.into();
        let InventoryControl::TerminalList(page) =
            decode_inventory_control(10, MessageRole::Response, &serde_json::to_vec(&value)?)?
        else {
            panic!("inventory");
        };
        let TerminalSummary::Running { core, .. } = &page.terminals[1] else {
            panic!("running shell");
        };
        assert_eq!(core.close_activity, Some(activity));
        let encoded = encode_inventory_control(&InventoryControl::TerminalList(page))?;
        assert_eq!(serde_json::from_slice::<Value>(&encoded.payload)?, value);
    }
    for invalid in [Value::Null, true.into(), "running".into(), 1.into()] {
        let mut value: Value = serde_json::from_str(raw)?;
        value["terminals"][1]["closeActivity"] = invalid;
        assert!(
            decode_inventory_control(10, MessageRole::Response, &serde_json::to_vec(&value)?)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn canonical_inventory_payloads_decode_and_reencode_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    assert_canonical_round_trip(
        MessageRole::Request,
        fixture_string(&fixture, "/commands/list_terminals")?,
    )?;
    assert_canonical_round_trip(
        MessageRole::Response,
        fixture_string(&fixture, "/outputs/terminal_list")?,
    )?;
    Ok(())
}

#[test]
fn canonical_inventory_retains_every_phase_and_nested_result() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let request = decode_inventory_control(
        10,
        MessageRole::Request,
        fixture_string(&fixture, "/commands/list_terminals")?.as_bytes(),
    )?;
    let InventoryControl::ListTerminals(request) = request else {
        return Err(fixture_error("inventory request decoded to the wrong variant").into());
    };
    assert_eq!(request.maximum_terminals, 64);
    assert_eq!(
        request.cursor,
        Some(TerminalInventoryCursor {
            inventory_generation: nonzero(9)?,
            offset: 2,
        })
    );

    let response = fixture_terminal_list(&fixture)?;
    assert_eq!(response.inventory_generation, nonzero(9)?);
    assert_eq!(response.next_cursor, None);
    let [
        awaiting,
        running,
        finishing,
        stopping,
        blocked,
        exited,
        failed,
    ] = response.terminals.as_slice()
    else {
        return Err(
            fixture_error("inventory fixture must contain seven terminal summaries").into(),
        );
    };

    let TerminalSummary::AwaitingStart { core } = awaiting else {
        return Err(fixture_error("first summary is not awaiting start").into());
    };
    assert_eq!(core.configuration.scrollback_bytes, 10_000_000);
    assert_eq!(core.configuration_revision, 2);
    assert_eq!(core.revision, 0);
    assert_eq!(core.theme.revision, 0);
    assert_eq!(core.theme.operation_id, None);
    assert_eq!(core.theme.terminal.palette.len(), 256);
    assert_eq!(core.identity.terminal_id.to_text(), "44".repeat(16));

    assert!(matches!(
        running,
        TerminalSummary::Running {
            geometry,
            terminate_requested: false,
            ..
        } if geometry.columns == 120 && geometry.rows == 40
    ));
    assert!(matches!(
        finishing,
        TerminalSummary::Finishing {
            exit: Some(ExitResult::Unknown(7)),
            terminate_requested: true,
            ..
        }
    ));
    assert!(matches!(
        stopping,
        TerminalSummary::ContainingFailure {
            failure: RuntimeFailure::ReadOutput,
            failure_containment: FailureContainment::Stopping { process: None },
            ..
        }
    ));
    assert!(matches!(
        blocked,
        TerminalSummary::ContainingFailure {
            failure: RuntimeFailure::ApplyResize,
            failure_containment: FailureContainment::Blocked {
                cleanup_failure: RuntimeCleanupFailure::ReleaseOutput,
                process: Some(ProcessObservation::Observed(ExitResult::ExitCode(1))),
            },
            ..
        }
    ));
    assert!(matches!(
        exited,
        TerminalSummary::Exited {
            exit: ExitResult::Signal(15),
            terminate_requested: true,
            ..
        }
    ));
    assert!(matches!(
        failed,
        TerminalSummary::Failed {
            failure: RuntimeFailure::SynchronizeWorker,
            process: ProcessObservation::Unavailable,
            ..
        }
    ));
    Ok(())
}

#[test]
fn required_nullable_unknown_duplicate_and_identity_fields_are_strict() -> Result<(), Box<dyn Error>>
{
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/outputs/terminal_list")?;

    for pointer in [
        "/nextCursor",
        "/terminals/0/exit",
        "/terminals/0/theme/operationId",
        "/terminals/0/theme/terminal/defaultCursorRgb",
    ] {
        let mut value: Value = serde_json::from_str(canonical)?;
        remove_pointer_member(&mut value, pointer)?;
        let result = decode_value(value);
        assert!(
            matches!(result, Err(error) if error.code() == ProtocolErrorCode::InvalidField),
            "missing required nullable member {pointer} produced {result:?}",
        );
    }

    let request = fixture_string(&fixture, "/commands/list_terminals")?;
    let mut request_value: Value = serde_json::from_str(request)?;
    remove_pointer_member(&mut request_value, "/cursor")?;
    let request_payload = serde_json::to_vec(&request_value)?;
    assert_code(
        decode_inventory_control(10, MessageRole::Request, &request_payload),
        ProtocolErrorCode::InvalidField,
    );

    let mut unknown: Value = serde_json::from_str(canonical)?;
    object_mut_at(&mut unknown, "/terminals/0/theme/terminal")?
        .insert("extra".to_owned(), Value::Bool(true));
    assert_value_code(unknown, ProtocolErrorCode::InvalidField);

    let duplicate = canonical.replacen(
        "\"inventoryGeneration\":\"9\"",
        "\"inventoryGeneration\":\"9\",\"inventoryGeneration\":\"9\"",
        1,
    );
    assert_code(
        decode_inventory_control(10, MessageRole::Response, duplicate.as_bytes()),
        ProtocolErrorCode::InvalidJson,
    );

    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(canonical)?;
        set_pointer(
            &mut value,
            "/terminals/0/identity/terminalId",
            Value::String(identity),
        )?;
        assert_value_code(value, ProtocolErrorCode::InvalidIdentifier);
    }
    Ok(())
}

#[test]
fn decimal_uint64_and_binary64_numbers_follow_the_current_refinements() -> Result<(), Box<dyn Error>>
{
    let fixture = read_fixture()?;
    let request = fixture_string(&fixture, "/commands/list_terminals")?;
    let rounded = request.replacen(
        "\"maximumTerminals\":64",
        "\"maximumTerminals\":1.0000000000000001",
        1,
    );
    let decoded = decode_inventory_control(10, MessageRole::Request, rounded.as_bytes())?;
    assert!(matches!(
        decoded,
        InventoryControl::ListTerminals(request) if request.maximum_terminals == 1
    ));

    for token in ["0", "129", "9007199254740992", "1e4000"] {
        let payload = request.replacen(
            "\"maximumTerminals\":64",
            &format!("\"maximumTerminals\":{token}"),
            1,
        );
        assert_code(
            decode_inventory_control(10, MessageRole::Request, payload.as_bytes()),
            ProtocolErrorCode::InvalidField,
        );
    }

    let response = fixture_string(&fixture, "/outputs/terminal_list")?;
    for text in ["0", "00", "18446744073709551616", "+1", " 1"] {
        let mut value: Value = serde_json::from_str(response)?;
        set_pointer(
            &mut value,
            "/inventoryGeneration",
            Value::String(text.to_owned()),
        )?;
        assert_value_code(value, ProtocolErrorCode::InvalidField);
    }
    let mut maximum: Value = serde_json::from_str(response)?;
    set_pointer(
        &mut maximum,
        "/inventoryGeneration",
        Value::String(u64::MAX.to_string()),
    )?;
    assert!(decode_value(maximum).is_ok());
    Ok(())
}

#[test]
fn cursor_theme_palette_geometry_and_page_invariants_are_enforced() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/outputs/terminal_list")?;

    let mut cursor_mismatch: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut cursor_mismatch,
        "/nextCursor",
        serde_json::json!({"inventoryGeneration":"10","offset":"7"}),
    )?;
    assert_value_code(cursor_mismatch, ProtocolErrorCode::InvalidField);

    let mut theme_revision: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut theme_revision,
        "/terminals/0/theme/revision",
        Value::String("1".to_owned()),
    )?;
    assert_value_code(theme_revision, ProtocolErrorCode::InvalidField);

    let mut themed: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut themed,
        "/terminals/0/theme/revision",
        Value::String("1".to_owned()),
    )?;
    set_pointer(
        &mut themed,
        "/terminals/0/theme/operationId",
        Value::String("77".repeat(16)),
    )?;
    assert!(decode_value(themed.clone()).is_ok());
    set_pointer(
        &mut themed,
        "/terminals/0/theme/operationId",
        Value::String("GG".repeat(16)),
    )?;
    assert_value_code(themed, ProtocolErrorCode::InvalidIdentifier);

    let mut palette_length: Value = serde_json::from_str(canonical)?;
    array_mut_at(&mut palette_length, "/terminals/0/theme/terminal/palette")?.pop();
    assert_value_code(palette_length, ProtocolErrorCode::InvalidField);

    let mut rgb: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut rgb,
        "/terminals/0/theme/terminal/defaultBackgroundRgb",
        Value::from(0x0100_0000_u32),
    )?;
    assert_value_code(rgb, ProtocolErrorCode::InvalidField);

    for (pointer, replacement) in [
        ("/terminals/1/columns", Value::from(0)),
        ("/terminals/1/rows", Value::from(4097)),
        (
            "/terminals/0/configuration/scrollbackBytes",
            Value::from(1_000_000_001_u64),
        ),
    ] {
        let mut invalid: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut invalid, pointer, replacement)?;
        assert_value_code(invalid, ProtocolErrorCode::InvalidField);
    }

    let mut response = fixture_terminal_list(&fixture)?;
    response.next_cursor = Some(TerminalInventoryCursor {
        inventory_generation: nonzero(10)?,
        offset: 7,
    });
    assert_code(
        encode_inventory_control(&InventoryControl::TerminalList(response.clone())),
        ProtocolErrorCode::InvalidField,
    );

    let exemplar = response
        .terminals
        .first()
        .cloned()
        .ok_or_else(|| fixture_error("inventory fixture has no terminal exemplar"))?;
    response.next_cursor = None;
    response.terminals = vec![exemplar; MAXIMUM_INVENTORY_PAGE_TERMINALS + 1];
    assert_code(
        encode_inventory_control(&InventoryControl::TerminalList(response)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn every_runtime_and_cleanup_failure_pair_is_closed() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/outputs/terminal_list")?;
    for (kind, stage) in [
        ("terminal-reply", "admit-terminal-reply"),
        ("resize", "apply-resize"),
        ("process-control", "dispatch-termination"),
        ("process-observation", "observe-process"),
        ("pty-read", "read-output"),
        ("worker-integrity", "synchronize-worker"),
        ("pty-write", "write-input"),
    ] {
        let mut value: Value = serde_json::from_str(canonical)?;
        set_failure_pair(&mut value, "/terminals/3/failure", kind, stage)?;
        assert!(decode_value(value).is_ok(), "runtime pair {kind}/{stage}");
    }
    for (kind, stage) in [
        ("process-control", "contain-process"),
        ("pty-read", "release-output"),
        ("process-observation", "release-process-identity"),
        ("editor-document", "release-editor-document"),
    ] {
        let mut value: Value = serde_json::from_str(canonical)?;
        set_failure_pair(
            &mut value,
            "/terminals/4/failureContainment/cleanupFailure",
            kind,
            stage,
        )?;
        assert!(decode_value(value).is_ok(), "cleanup pair {kind}/{stage}");
    }

    let mut invalid_runtime: Value = serde_json::from_str(canonical)?;
    set_failure_pair(
        &mut invalid_runtime,
        "/terminals/3/failure",
        "pty-write",
        "read-output",
    )?;
    assert_value_code(invalid_runtime, ProtocolErrorCode::InvalidField);

    let mut invalid_cleanup: Value = serde_json::from_str(canonical)?;
    set_failure_pair(
        &mut invalid_cleanup,
        "/terminals/4/failureContainment/cleanupFailure",
        "process-control",
        "release-output",
    )?;
    assert_value_code(invalid_cleanup, ProtocolErrorCode::InvalidField);
    for pointer in ["/terminals/3/failure", "/terminals/6/process/failure"] {
        let mut invalid: Value = serde_json::from_str(canonical)?;
        set_failure_pair(
            &mut invalid,
            pointer,
            "editor-document",
            "release-editor-document",
        )?;
        assert_value_code(invalid, ProtocolErrorCode::InvalidField);
    }
    let mut invalid_document: Value = serde_json::from_str(canonical)?;
    set_failure_pair(
        &mut invalid_document,
        "/terminals/4/failureContainment/cleanupFailure",
        "pty-read",
        "release-editor-document",
    )?;
    assert_value_code(invalid_document, ProtocolErrorCode::InvalidField);
    Ok(())
}

#[test]
fn terminal_phase_fields_are_mutually_exclusive() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/outputs/terminal_list")?;
    for (pointer, replacement) in [
        ("/terminals/0/columns", Value::from(80)),
        ("/terminals/0/terminateRequested", Value::Bool(true)),
        (
            "/terminals/1/exit",
            serde_json::json!({"kind":"unknown","value":0}),
        ),
        (
            "/terminals/3/process",
            serde_json::json!({"kind":"observed","result":{"kind":"unknown","value":0}}),
        ),
        ("/terminals/5/exit", Value::Null),
        (
            "/terminals/6/failureContainment",
            serde_json::json!({"kind":"stopping","process":null}),
        ),
    ] {
        let mut value: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut value, pointer, replacement)?;
        assert_value_code(value, ProtocolErrorCode::InvalidField);
    }
    Ok(())
}

#[test]
fn inventory_kind_and_role_are_exact_and_payloads_remain_bounded() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let request = fixture_string(&fixture, "/commands/list_terminals")?;
    for (kind, role) in [
        (9, MessageRole::Request),
        (10, MessageRole::Event),
        (11, MessageRole::Request),
    ] {
        assert_code(
            decode_inventory_control(kind, role, request.as_bytes()),
            ProtocolErrorCode::InvalidField,
        );
    }

    let oversized = vec![b' '; MAXIMUM_CONTROL_JSON_BYTES + 1];
    assert_code(
        decode_inventory_control(10, MessageRole::Request, &oversized),
        ProtocolErrorCode::LimitExceeded,
    );
    assert_code(
        decode_inventory_control(10, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_code(
        decode_inventory_control(10, MessageRole::Request, b"{"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_inventory_control(10, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );

    let mut response = fixture_terminal_list(&fixture)?;
    let exemplar = response
        .terminals
        .first()
        .cloned()
        .ok_or_else(|| fixture_error("inventory fixture has no terminal exemplar"))?;
    response.terminals = vec![exemplar; MAXIMUM_INVENTORY_PAGE_TERMINALS];
    assert_code(
        encode_inventory_control(&InventoryControl::TerminalList(response)),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn public_inventory_values_are_revalidated_before_encoding() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let response = fixture_terminal_list(&fixture)?;

    let mut invalid_theme = response.clone();
    summary_core_mut(first_summary_mut(&mut invalid_theme)?)?
        .theme
        .terminal
        .palette
        .pop();
    assert_code(
        encode_inventory_control(&InventoryControl::TerminalList(invalid_theme)),
        ProtocolErrorCode::InvalidField,
    );

    let mut invalid_geometry = response.clone();
    let running = terminal_mut(&mut invalid_geometry, 1)?;
    let TerminalSummary::Running { geometry, .. } = running else {
        return Err(fixture_error("second fixture summary is not running").into());
    };
    geometry.columns = 0;
    assert_code(
        encode_inventory_control(&InventoryControl::TerminalList(invalid_geometry)),
        ProtocolErrorCode::InvalidField,
    );

    let mut invalid_theme_state = response.clone();
    summary_core_mut(first_summary_mut(&mut invalid_theme_state)?)?
        .theme
        .revision = 1;
    assert_code(
        encode_inventory_control(&InventoryControl::TerminalList(invalid_theme_state)),
        ProtocolErrorCode::InvalidField,
    );

    let mut invalid_scrollback = response.clone();
    summary_core_mut(first_summary_mut(&mut invalid_scrollback)?)?
        .configuration
        .scrollback_bytes = 1_000_000_001;
    assert_code(
        encode_inventory_control(&InventoryControl::TerminalList(invalid_scrollback)),
        ProtocolErrorCode::InvalidField,
    );

    let mut invalid_signal = response;
    let exited = terminal_mut(&mut invalid_signal, 5)?;
    let TerminalSummary::Exited { exit, .. } = exited else {
        return Err(fixture_error("sixth fixture summary is not exited").into());
    };
    *exit = ExitResult::Signal(0);
    assert_code(
        encode_inventory_control(&InventoryControl::TerminalList(invalid_signal)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn inventory_control_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let value = decode_inventory_control(
        10,
        MessageRole::Response,
        fixture_string(&fixture, "/outputs/terminal_list")?.as_bytes(),
    )?;
    let EncodedInventoryControl {
        kind,
        payload,
        role,
    } = encode_inventory_control(&value)?;
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
        decode_inventory_control(envelope.kind, envelope.role, envelope.payload)?,
        value
    );
    Ok(())
}

fn assert_canonical_round_trip(role: MessageRole, payload: &str) -> Result<(), Box<dyn Error>> {
    let decoded = decode_inventory_control(10, role, payload.as_bytes())?;
    let encoded = encode_inventory_control(&decoded)?;
    assert_eq!(encoded.kind, 10);
    assert_eq!(encoded.role, role);
    assert_eq!(encoded.payload, payload.as_bytes());
    Ok(())
}

fn fixture_terminal_list(fixture: &Value) -> Result<TerminalList, Box<dyn Error>> {
    let decoded = decode_inventory_control(
        10,
        MessageRole::Response,
        fixture_string(fixture, "/outputs/terminal_list")?.as_bytes(),
    )?;
    let InventoryControl::TerminalList(response) = decoded else {
        return Err(fixture_error("inventory response decoded to the wrong variant").into());
    };
    Ok(response)
}

fn decode_value(value: Value) -> Result<InventoryControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_inventory_control(10, MessageRole::Response, &payload)
}

fn assert_value_code(value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(value), expected);
}

fn set_failure_pair(
    value: &mut Value,
    pointer: &str,
    kind: &str,
    stage: &str,
) -> Result<(), io::Error> {
    let failure = object_mut_at(value, pointer)?;
    failure.insert("kind".to_owned(), Value::String(kind.to_owned()));
    failure.insert("stage".to_owned(), Value::String(stage.to_owned()));
    Ok(())
}

fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| fixture_error(&format!("fixture pointer {pointer} is missing")))?;
    *target = replacement;
    Ok(())
}

fn remove_pointer_member(value: &mut Value, pointer: &str) -> Result<(), io::Error> {
    let (parent_pointer, member) = pointer
        .rsplit_once('/')
        .ok_or_else(|| fixture_error("member pointer has no parent"))?;
    let removed = object_mut_at(value, parent_pointer)?.remove(member);
    if removed.is_none() {
        return Err(fixture_error(&format!(
            "fixture member {pointer} is missing"
        )));
    }
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

fn array_mut_at<'value>(
    value: &'value mut Value,
    pointer: &str,
) -> Result<&'value mut Vec<Value>, io::Error> {
    value
        .pointer_mut(pointer)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| fixture_error(&format!("fixture array {pointer} is missing")))
}

fn terminal_mut(value: &mut TerminalList, index: usize) -> Result<&mut TerminalSummary, io::Error> {
    value
        .terminals
        .get_mut(index)
        .ok_or_else(|| fixture_error("terminal fixture index is missing"))
}

fn first_summary_mut(value: &mut TerminalList) -> Result<&mut TerminalSummary, io::Error> {
    terminal_mut(value, 0)
}

fn summary_core_mut(
    value: &mut TerminalSummary,
) -> Result<&mut orchbee_terminal_protocol::TerminalSummaryCore, io::Error> {
    let core = match value {
        TerminalSummary::AwaitingStart { core }
        | TerminalSummary::Running { core, .. }
        | TerminalSummary::Finishing { core, .. }
        | TerminalSummary::ContainingFailure { core, .. }
        | TerminalSummary::Exited { core, .. }
        | TerminalSummary::Failed { core, .. } => core,
    };
    Ok(core)
}

fn nonzero(value: u64) -> Result<NonZeroU64, io::Error> {
    NonZeroU64::new(value).ok_or_else(|| fixture_error("test generation must be nonzero"))
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

fn fixture_error(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}

fn assert_code<ValueType>(result: Result<ValueType, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Ok(_value) => panic!("expected protocol error {expected}"),
        Err(error) => assert_eq!(error.code(), expected),
    }
}
