use std::{error::Error, io};

use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ColorScheme, CorrelationId, CursorShape,
    EncodedTerminalCreationControl, Envelope, HostEpoch, LaunchStartMode,
    MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_ENVIRONMENT_NAME_BYTES,
    MAXIMUM_ENVIRONMENT_OVERLAY_ENTRIES, MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES,
    MAXIMUM_LAUNCH_ARGUMENTS, MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES, MAXIMUM_RGB_VALUE,
    MAXIMUM_SCROLLBACK_BYTES, MAXIMUM_TERMINAL_PROFILE_FEATURES, MAXIMUM_TERMINAL_RESPONSE_BYTES,
    MessageRole, PayloadEncoding, ProtocolError, ProtocolErrorCode, TERMINAL_THEME_PALETTE_ENTRIES,
    TerminalConfiguration, TerminalCreationControl, TerminalPixelGeometry,
    decode_terminal_creation_control, decode_transport_envelope, encode_terminal_creation_control,
    encode_transport_envelope,
};
use serde_json::{Map, Value, json};

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.25.json");

#[test]
fn editor_capture_round_trips_full_u64_fences_and_retains_the_ordinary_request()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for revision in ["0", "18446744073709551615"] {
        let mut request: Value = serde_json::from_str(fixture_string(
            &fixture,
            "/commands/create_terminal_deferred",
        )?)?;
        request["captureSource"] = json!({
            "attachmentId": "66".repeat(16), "terminalId": "44".repeat(16),
            "generation": "18446744073709551615", "revision": revision,
        });
        let decoded = decode_value(MessageRole::Request, request.clone())?;
        let TerminalCreationControl::CreateScrollbackEditor(editor) = &decoded else {
            return Err(fixture_error("editor capture selected the wrong creation variant").into());
        };
        assert_eq!(editor.creation.launch.start_mode, LaunchStartMode::Deferred);
        assert_eq!(editor.source.generation.get(), u64::MAX);
        assert_eq!(editor.source.revision.to_string(), revision);
        assert_eq!(
            serde_json::to_value(&editor.creation.launch.arguments)?,
            request["launch"]["arguments"]
        );
        let encoded = encode_terminal_creation_control(&decoded)?;
        assert_eq!(encoded.kind, 11);
        assert_eq!(encoded.role, MessageRole::Request);
        assert_eq!(serde_json::from_slice::<Value>(&encoded.payload)?, request);
        assert_eq!(
            decode_terminal_creation_control(11, MessageRole::Request, &encoded.payload)?,
            decoded
        );
    }
    Ok(())
}

#[test]
fn editor_capture_rejects_null_unknown_fields_immediate_geometry_and_malformed_fences()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/commands/create_terminal_deferred")?;
    let source = json!({
        "attachmentId": "66".repeat(16), "terminalId": "44".repeat(16),
        "generation": "1", "revision": "0",
    });
    for invalid_source in [Value::Null, json!({}), json!(1), json!([])] {
        let mut request: Value = serde_json::from_str(canonical)?;
        request["captureSource"] = invalid_source;
        assert_value_code(
            MessageRole::Request,
            request,
            ProtocolErrorCode::InvalidField,
        );
    }
    let mut request: Value = serde_json::from_str(canonical)?;
    request["captureSource"] = source.clone();
    for (key, invalid) in [
        ("generation", json!("0")),
        ("generation", json!("01")),
        ("generation", json!("18446744073709551616")),
        ("generation", json!(1)),
        ("revision", json!("-1")),
        ("revision", json!("00")),
        ("revision", json!("1e2")),
        ("revision", json!("18446744073709551616")),
        ("revision", json!(0)),
        ("path", json!("C:\\foreign.txt")),
    ] {
        let mut invalid_request = request.clone();
        invalid_request["captureSource"][key] = invalid;
        assert_value_code(
            MessageRole::Request,
            invalid_request,
            ProtocolErrorCode::InvalidField,
        );
    }
    for key in ["attachmentId", "terminalId"] {
        let mut invalid = request.clone();
        invalid["captureSource"][key] = json!("00".repeat(16));
        assert_value_code(
            MessageRole::Request,
            invalid,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }
    let mut immediate: Value =
        serde_json::from_str(fixture_string(&fixture, "/commands/create_terminal")?)?;
    immediate["captureSource"] = source;
    assert_value_code(
        MessageRole::Request,
        immediate,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn editor_capture_encoder_reserves_exactly_one_document_argument() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let mut request: Value = serde_json::from_str(fixture_string(
        &fixture,
        "/commands/create_terminal_deferred",
    )?)?;
    request["captureSource"] = json!({
        "attachmentId": "66".repeat(16), "terminalId": "44".repeat(16), "generation": "1", "revision": "0",
    });
    let TerminalCreationControl::CreateScrollbackEditor(mut editor) =
        decode_value(MessageRole::Request, request)?
    else {
        return Err(fixture_error("expected editor creation").into());
    };
    editor.creation.launch.arguments = vec![String::new(); MAXIMUM_LAUNCH_ARGUMENTS - 1];
    assert!(
        encode_terminal_creation_control(&TerminalCreationControl::CreateScrollbackEditor(
            editor.clone()
        ))
        .is_ok()
    );
    editor.creation.launch.arguments.push(String::new());
    assert_code(
        encode_terminal_creation_control(&TerminalCreationControl::CreateScrollbackEditor(editor)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn canonical_creation_controls_round_trip_in_every_direction() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (role, pointer) in [
        (MessageRole::Request, "/commands/create_terminal"),
        (MessageRole::Request, "/commands/create_terminal_deferred"),
        (MessageRole::Response, "/outputs/terminal_identity"),
    ] {
        let payload = fixture_string(&fixture, pointer)?;
        let decoded = decode_terminal_creation_control(11, role, payload.as_bytes())?;
        let encoded: EncodedTerminalCreationControl = encode_terminal_creation_control(&decoded)?;
        assert_eq!(encoded.kind, 11);
        assert_eq!(encoded.role, role);
        assert_eq!(encoded.payload, payload.as_bytes());
    }
    Ok(())
}

#[test]
fn canonical_requests_retain_complete_launch_and_profile_facts() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let immediate = fixture_string(&fixture, "/commands/create_terminal")?;
    let TerminalCreationControl::CreateTerminal(immediate) =
        decode_terminal_creation_control(11, MessageRole::Request, immediate.as_bytes())?
    else {
        return Err(fixture_error("create request decoded to the wrong variant").into());
    };
    assert_eq!(immediate.creation_id.to_text(), "33".repeat(16));
    assert_eq!(immediate.environment_overlay.len(), 1);
    assert_eq!(
        immediate.environment_overlay[0].name,
        "ORCHBEE_AGENT_OBSERVATION_SOCKET"
    );
    assert_eq!(
        immediate.environment_overlay[0].value,
        "/tmp/orchbee-agent.sock"
    );
    assert_eq!(immediate.launch.arguments, ["--login", "项目"]);
    assert_eq!(immediate.launch.configuration, None);
    assert_eq!(
        immediate.launch.current_directory.as_deref(),
        Some("/tmp/项目")
    );
    assert_eq!(immediate.launch.executable, "/bin/sh");
    assert_eq!(immediate.launch.environment.len(), 2);
    assert_eq!(immediate.launch.environment[0].value.as_deref(), Some(""));
    assert_eq!(immediate.launch.environment[1].value, None);
    let LaunchStartMode::Immediate(geometry) = immediate.launch.start_mode else {
        return Err(fixture_error("immediate launch decoded as deferred").into());
    };
    assert_eq!(geometry.cell_pixel_height, 16);
    assert_eq!(geometry.cell_pixel_width, 8);
    assert_eq!(geometry.columns, 120);
    assert_eq!(geometry.rows, 40);

    let profile = &immediate.launch.terminal_profile;
    assert_eq!(profile.color_scheme, ColorScheme::Dark);
    assert_eq!(profile.default_background_rgb, 1_052_688);
    assert!(profile.default_cursor_blinking);
    assert_eq!(profile.default_cursor_rgb, None);
    assert_eq!(profile.default_cursor_shape, CursorShape::Underline);
    assert_eq!(profile.default_foreground_rgb, 15_790_320);
    assert_eq!(profile.device_attributes.primary_conformance_level, 65);
    assert_eq!(profile.device_attributes.primary_features, [1, 4]);
    assert_eq!(profile.device_attributes.secondary_device_type, 1);
    assert_eq!(profile.device_attributes.secondary_firmware_version, 2);
    assert_eq!(profile.device_attributes.secondary_rom_cartridge, 0);
    assert_eq!(profile.device_attributes.tertiary_unit_id, 42);
    assert_eq!(profile.enquiry_response, "\u{1b}[?1;2c");
    assert_eq!(profile.palette.len(), TERMINAL_THEME_PALETTE_ENTRIES);
    assert_eq!(profile.palette.first(), Some(&0));
    assert_eq!(profile.palette.last(), Some(&MAXIMUM_RGB_VALUE));
    assert_eq!(profile.version_response, "\u{1b}[>1;2;3c");

    let deferred = fixture_string(&fixture, "/commands/create_terminal_deferred")?;
    let TerminalCreationControl::CreateTerminal(deferred) =
        decode_terminal_creation_control(11, MessageRole::Request, deferred.as_bytes())?
    else {
        return Err(fixture_error("deferred request decoded to the wrong variant").into());
    };
    assert_eq!(deferred.environment_overlay, []);
    assert_eq!(deferred.launch.start_mode, LaunchStartMode::Deferred);

    let response = fixture_string(&fixture, "/outputs/terminal_identity")?;
    let TerminalCreationControl::TerminalCreated(identity) =
        decode_terminal_creation_control(11, MessageRole::Response, response.as_bytes())?
    else {
        return Err(fixture_error("create response decoded to the wrong variant").into());
    };
    assert_eq!(identity.creation_id.to_text(), "33".repeat(16));
    assert_eq!(identity.host_epoch.to_text(), "11".repeat(16));
    assert_eq!(identity.terminal_id.to_text(), "44".repeat(16));
    Ok(())
}

#[test]
fn profile_cursor_compatibility_defaults_decode_and_current_encoding_materializes_them()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let mut value: Value = serde_json::from_str(fixture_string(
        &fixture,
        "/commands/create_terminal_deferred",
    )?)?;
    let profile = object_mut_at(&mut value, "/launch/terminalProfile")?;
    profile.remove("defaultCursorBlinking");
    profile.remove("defaultCursorShape");

    let decoded = decode_value(MessageRole::Request, value)?;
    let TerminalCreationControl::CreateTerminal(create) = &decoded else {
        return Err(fixture_error("compatibility profile decoded to the wrong variant").into());
    };
    assert!(!create.launch.terminal_profile.default_cursor_blinking);
    assert_eq!(
        create.launch.terminal_profile.default_cursor_shape,
        CursorShape::Block
    );

    let encoded = encode_terminal_creation_control(&decoded)?;
    let materialized: Value = serde_json::from_slice(&encoded.payload)?;
    assert_eq!(
        materialized.pointer("/launch/terminalProfile/defaultCursorBlinking"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        materialized.pointer("/launch/terminalProfile/defaultCursorShape"),
        Some(&Value::String("block".to_owned()))
    );
    Ok(())
}

#[test]
fn creation_json_is_utf8_unique_complete_and_closed_at_every_depth() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_terminal_creation_control(11, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(MessageRole::Request, "{", ProtocolErrorCode::InvalidJson);
    assert_code(
        decode_terminal_creation_control(11, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(MessageRole::Request, "{}", ProtocolErrorCode::InvalidField);

    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/commands/create_terminal")?;
    let mut unknown_top: Value = serde_json::from_str(canonical)?;
    object_mut_at(&mut unknown_top, "")?.insert("extra".to_owned(), Value::Bool(true));
    assert_value_code(
        MessageRole::Request,
        unknown_top,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = canonical.replacen(
        "\"creationId\":",
        "\"creationId\":\"33333333333333333333333333333333\",\"creationId\":",
        1,
    );
    assert_payload_code(
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = canonical.replacen(
        "\"terminalProfile\":",
        "\"terminalProfile\":{},\"terminal\\u0050rofile\":",
        1,
    );
    assert_payload_code(
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let surrogate = canonical.replacen("\"/bin/sh\"", "\"\\ud800\"", 1);
    assert_payload_code(
        MessageRole::Request,
        &surrogate,
        ProtocolErrorCode::InvalidJson,
    );

    for pointer in [
        "/launch",
        "/launch/terminalProfile",
        "/launch/terminalProfile/deviceAttributes",
        "/launch/environment/0",
        "/environmentOverlay/0",
    ] {
        let mut value: Value = serde_json::from_str(canonical)?;
        object_mut_at(&mut value, pointer)?.insert("extra".to_owned(), Value::Bool(true));
        assert_value_code(MessageRole::Request, value, ProtocolErrorCode::InvalidField);
    }

    for (parent, member) in [
        ("", "creationId"),
        ("", "environmentOverlay"),
        ("", "launch"),
        ("/launch", "configuration"),
        ("/launch", "currentDirectory"),
        ("/launch", "cellPixelHeight"),
        ("/launch", "terminalProfile"),
        ("/launch/environment/0", "value"),
        ("/launch/terminalProfile", "defaultCursorRgb"),
        (
            "/launch/terminalProfile/deviceAttributes",
            "primaryFeatures",
        ),
    ] {
        let mut value: Value = serde_json::from_str(canonical)?;
        object_mut_at(&mut value, parent)?.remove(member);
        assert_value_code(MessageRole::Request, value, ProtocolErrorCode::InvalidField);
    }

    for (pointer, vocabulary) in [
        ("/launch/terminalProfile/colorScheme", "sepia"),
        ("/launch/terminalProfile/defaultCursorShape", "beam"),
    ] {
        let mut invalid: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut invalid, pointer, Value::String(vocabulary.to_owned()))?;
        assert_value_code(
            MessageRole::Request,
            invalid,
            ProtocolErrorCode::InvalidField,
        );
    }

    let response = fixture_string(&fixture, "/outputs/terminal_identity")?;
    let response_unknown = response.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        MessageRole::Response,
        &response_unknown,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn launch_geometry_is_a_closed_immediate_or_deferred_mode() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let immediate = fixture_string(&fixture, "/commands/create_terminal")?;
    let deferred = fixture_string(&fixture, "/commands/create_terminal_deferred")?;
    assert!(
        decode_terminal_creation_control(11, MessageRole::Request, immediate.as_bytes()).is_ok()
    );
    assert!(
        decode_terminal_creation_control(11, MessageRole::Request, deferred.as_bytes()).is_ok()
    );

    for pointer in [
        "/launch/cellPixelHeight",
        "/launch/cellPixelWidth",
        "/launch/columns",
        "/launch/rows",
    ] {
        let mut mixed: Value = serde_json::from_str(immediate)?;
        set_pointer(&mut mixed, pointer, Value::Null)?;
        assert_value_code(MessageRole::Request, mixed, ProtocolErrorCode::InvalidField);
    }

    let mut maximum: Value = serde_json::from_str(immediate)?;
    set_pointer(&mut maximum, "/launch/cellPixelHeight", json!(1023))?;
    set_pointer(&mut maximum, "/launch/cellPixelWidth", json!(15))?;
    set_pointer(&mut maximum, "/launch/columns", json!(4096))?;
    set_pointer(&mut maximum, "/launch/rows", json!(64))?;
    assert!(decode_value(MessageRole::Request, maximum).is_ok());

    for (pointer, invalid) in [
        ("/launch/cellPixelHeight", json!(0)),
        ("/launch/cellPixelWidth", json!(4097)),
        ("/launch/columns", json!(0)),
        ("/launch/rows", json!(4097)),
    ] {
        let mut value: Value = serde_json::from_str(immediate)?;
        set_pointer(&mut value, pointer, invalid)?;
        assert_value_code(MessageRole::Request, value, ProtocolErrorCode::InvalidField);
    }

    let mut too_many_cells: Value = serde_json::from_str(immediate)?;
    set_pointer(&mut too_many_cells, "/launch/columns", json!(4096))?;
    set_pointer(&mut too_many_cells, "/launch/rows", json!(65))?;
    assert_value_code(
        MessageRole::Request,
        too_many_cells,
        ProtocolErrorCode::InvalidField,
    );

    let mut too_wide: Value = serde_json::from_str(immediate)?;
    set_pointer(&mut too_wide, "/launch/cellPixelWidth", json!(4096))?;
    set_pointer(&mut too_wide, "/launch/columns", json!(16))?;
    assert_value_code(
        MessageRole::Request,
        too_wide,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn environment_collections_preserve_null_empty_and_wire_name_identity() -> Result<(), Box<dyn Error>>
{
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/commands/create_terminal")?;

    let mut case_distinct: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut case_distinct,
        "/launch/environment",
        json!([
            {"name": "Path", "value": "a"},
            {"name": "PATH", "value": "b"}
        ]),
    )?;
    set_pointer(
        &mut case_distinct,
        "/environmentOverlay",
        json!([
            {"name": "Token", "value": ""},
            {"name": "TOKEN", "value": "value"}
        ]),
    )?;
    assert!(decode_value(MessageRole::Request, case_distinct).is_ok());

    for pointer in ["/launch/environment", "/environmentOverlay"] {
        let mut duplicate: Value = serde_json::from_str(canonical)?;
        let entries = if pointer == "/launch/environment" {
            json!([
                {"name": "SAME", "value": ""},
                {"name": "SAME", "value": null}
            ])
        } else {
            json!([
                {"name": "SAME", "value": "a"},
                {"name": "SAME", "value": "b"}
            ])
        };
        set_pointer(&mut duplicate, pointer, entries)?;
        assert_value_code(
            MessageRole::Request,
            duplicate,
            ProtocolErrorCode::InvalidField,
        );
    }

    let mut overlap: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut overlap,
        "/launch/environment",
        json!([{"name": "SAME", "value": null}]),
    )?;
    set_pointer(
        &mut overlap,
        "/environmentOverlay",
        json!([{"name": "SAME", "value": "transient"}]),
    )?;
    assert!(decode_value(MessageRole::Request, overlap).is_ok());
    Ok(())
}

#[test]
fn creation_collection_and_text_bounds_match_the_canonical_fixture() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    assert_fixture_limit(
        &fixture,
        "/protocol_limits/launch_arguments",
        MAXIMUM_LAUNCH_ARGUMENTS,
    )?;
    assert_fixture_limit(
        &fixture,
        "/protocol_limits/launch_environment_entries",
        MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES,
    )?;
    assert_fixture_limit(
        &fixture,
        "/protocol_limits/environment_overlay_entries",
        MAXIMUM_ENVIRONMENT_OVERLAY_ENTRIES,
    )?;
    assert_fixture_limit(
        &fixture,
        "/protocol_limits/environment_name_bytes",
        MAXIMUM_ENVIRONMENT_NAME_BYTES,
    )?;
    assert_fixture_limit(
        &fixture,
        "/protocol_limits/environment_overlay_value_bytes",
        MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES,
    )?;
    assert_fixture_limit(
        &fixture,
        "/protocol_limits/terminal_profile_features",
        MAXIMUM_TERMINAL_PROFILE_FEATURES,
    )?;
    assert_fixture_limit(
        &fixture,
        "/protocol_limits/terminal_response_bytes",
        MAXIMUM_TERMINAL_RESPONSE_BYTES,
    )?;
    assert_fixture_limit(
        &fixture,
        "/protocol_limits/terminal_profile_palette_entries",
        TERMINAL_THEME_PALETTE_ENTRIES,
    )?;

    let canonical = fixture_string(&fixture, "/commands/create_terminal")?;
    assert_array_bound(
        canonical,
        "/launch/arguments",
        (0..MAXIMUM_LAUNCH_ARGUMENTS)
            .map(|_| Value::String(String::new()))
            .collect(),
        Value::String(String::new()),
    )?;
    assert_array_bound(
        canonical,
        "/launch/environment",
        (0..MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES)
            .map(|index| json!({"name": format!("N{index}"), "value": null}))
            .collect(),
        json!({"name": "OVER", "value": null}),
    )?;
    assert_array_bound(
        canonical,
        "/environmentOverlay",
        (0..MAXIMUM_ENVIRONMENT_OVERLAY_ENTRIES)
            .map(|index| json!({"name": format!("O{index}"), "value": ""}))
            .collect(),
        json!({"name": "OVER", "value": ""}),
    )?;

    assert_string_bound(
        canonical,
        "/launch/environment/0/name",
        MAXIMUM_ENVIRONMENT_NAME_BYTES,
    )?;
    assert_string_bound(
        canonical,
        "/environmentOverlay/0/value",
        MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES,
    )?;
    assert_string_bound(
        canonical,
        "/launch/terminalProfile/enquiryResponse",
        MAXIMUM_TERMINAL_RESPONSE_BYTES,
    )?;
    assert_string_bound(
        canonical,
        "/launch/terminalProfile/versionResponse",
        MAXIMUM_TERMINAL_RESPONSE_BYTES,
    )?;
    Ok(())
}

#[test]
fn profile_colors_palette_features_and_device_values_are_closed() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/commands/create_terminal")?;

    for (pointer, values) in [
        (
            "/launch/terminalProfile/colorScheme",
            ["dark", "light"].as_slice(),
        ),
        (
            "/launch/terminalProfile/defaultCursorShape",
            ["block", "underline", "bar"].as_slice(),
        ),
    ] {
        for vocabulary in values {
            let mut accepted: Value = serde_json::from_str(canonical)?;
            set_pointer(
                &mut accepted,
                pointer,
                Value::String((*vocabulary).to_owned()),
            )?;
            let decoded = decode_value(MessageRole::Request, accepted.clone())?;
            let encoded: Value =
                serde_json::from_slice(&encode_terminal_creation_control(&decoded)?.payload)?;
            assert_eq!(encoded, accepted);
        }
    }

    let mut maximum_features: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut maximum_features,
        "/launch/terminalProfile/deviceAttributes/primaryFeatures",
        Value::Array(
            (0..MAXIMUM_TERMINAL_PROFILE_FEATURES)
                .map(|value| json!(value))
                .collect(),
        ),
    )?;
    assert!(decode_value(MessageRole::Request, maximum_features.clone()).is_ok());
    let features = array_mut_at(
        &mut maximum_features,
        "/launch/terminalProfile/deviceAttributes/primaryFeatures",
    )?;
    features.push(json!(MAXIMUM_TERMINAL_PROFILE_FEATURES));
    assert_value_code(
        MessageRole::Request,
        maximum_features,
        ProtocolErrorCode::InvalidField,
    );

    let mut duplicate_feature: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut duplicate_feature,
        "/launch/terminalProfile/deviceAttributes/primaryFeatures",
        json!([1, 1]),
    )?;
    assert_value_code(
        MessageRole::Request,
        duplicate_feature,
        ProtocolErrorCode::InvalidField,
    );

    for pointer in [
        "/launch/terminalProfile/defaultBackgroundRgb",
        "/launch/terminalProfile/defaultCursorRgb",
        "/launch/terminalProfile/defaultForegroundRgb",
        "/launch/terminalProfile/palette/0",
    ] {
        let mut invalid: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut invalid, pointer, json!(MAXIMUM_RGB_VALUE + 1))?;
        assert_value_code(
            MessageRole::Request,
            invalid,
            ProtocolErrorCode::InvalidField,
        );
    }

    for length in [
        TERMINAL_THEME_PALETTE_ENTRIES - 1,
        TERMINAL_THEME_PALETTE_ENTRIES + 1,
    ] {
        let mut invalid: Value = serde_json::from_str(canonical)?;
        set_pointer(
            &mut invalid,
            "/launch/terminalProfile/palette",
            Value::Array((0..length).map(|_| json!(0)).collect()),
        )?;
        assert_value_code(
            MessageRole::Request,
            invalid,
            ProtocolErrorCode::InvalidField,
        );
    }

    for pointer in [
        "/launch/terminalProfile/deviceAttributes/primaryConformanceLevel",
        "/launch/terminalProfile/deviceAttributes/secondaryDeviceType",
        "/launch/terminalProfile/deviceAttributes/secondaryFirmwareVersion",
        "/launch/terminalProfile/deviceAttributes/secondaryRomCartridge",
    ] {
        let mut invalid: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut invalid, pointer, json!(65_536))?;
        assert_value_code(
            MessageRole::Request,
            invalid,
            ProtocolErrorCode::InvalidField,
        );
    }
    let mut maximum_devices: Value = serde_json::from_str(canonical)?;
    for pointer in [
        "/launch/terminalProfile/deviceAttributes/primaryConformanceLevel",
        "/launch/terminalProfile/deviceAttributes/secondaryDeviceType",
        "/launch/terminalProfile/deviceAttributes/secondaryFirmwareVersion",
        "/launch/terminalProfile/deviceAttributes/secondaryRomCartridge",
    ] {
        set_pointer(&mut maximum_devices, pointer, json!(u16::MAX))?;
    }
    set_pointer(
        &mut maximum_devices,
        "/launch/terminalProfile/deviceAttributes/tertiaryUnitId",
        json!(u32::MAX),
    )?;
    assert!(decode_value(MessageRole::Request, maximum_devices).is_ok());

    let mut invalid_u32: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut invalid_u32,
        "/launch/terminalProfile/deviceAttributes/tertiaryUnitId",
        json!(u64::from(u32::MAX) + 1),
    )?;
    assert_value_code(
        MessageRole::Request,
        invalid_u32,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn identifiers_configuration_and_launch_strings_fail_closed() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/commands/create_terminal")?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut value, "/creationId", Value::String(identity))?;
        assert_value_code(
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let response = fixture_string(&fixture, "/outputs/terminal_identity")?;
    for pointer in ["/creationId", "/hostEpoch", "/terminalId"] {
        let mut value: Value = serde_json::from_str(response)?;
        set_pointer(&mut value, pointer, Value::String("00".repeat(16)))?;
        assert_value_code(
            MessageRole::Response,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }

    let mut maximum_configuration: Value = serde_json::from_str(canonical)?;
    set_pointer(
        &mut maximum_configuration,
        "/launch/configuration",
        json!({"scrollbackBytes": 1_000_000_000_u64}),
    )?;
    assert!(decode_value(MessageRole::Request, maximum_configuration.clone()).is_ok());
    set_pointer(
        &mut maximum_configuration,
        "/launch/configuration/scrollbackBytes",
        json!(1_000_000_001_u64),
    )?;
    assert_value_code(
        MessageRole::Request,
        maximum_configuration,
        ProtocolErrorCode::InvalidField,
    );

    for (pointer, value) in [
        ("/launch/executable", ""),
        ("/launch/executable", "/bin/bad\0name"),
        ("/launch/currentDirectory", "/tmp/bad\0path"),
        ("/launch/arguments/0", "bad\0argument"),
        ("/launch/environment/0/name", "BAD=NAME"),
        ("/launch/environment/0/name", ""),
        ("/launch/environment/0/value", "bad\0value"),
        ("/environmentOverlay/0/name", "BAD\0NAME"),
        ("/environmentOverlay/0/value", "bad\0value"),
        ("/launch/terminalProfile/enquiryResponse", "bad\0response"),
    ] {
        let mut invalid: Value = serde_json::from_str(canonical)?;
        set_pointer(&mut invalid, pointer, Value::String(value.to_owned()))?;
        assert_value_code(
            MessageRole::Request,
            invalid,
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn public_values_revalidate_before_encoding_and_obey_the_aggregate_limit()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let canonical = fixture_string(&fixture, "/commands/create_terminal")?;
    let decoded = decode_terminal_creation_control(11, MessageRole::Request, canonical.as_bytes())?;
    assert_eq!(
        decode_terminal_creation_control(
            11,
            MessageRole::Request,
            &encode_terminal_creation_control(&decoded)?.payload,
        )?,
        decoded
    );

    let TerminalCreationControl::CreateTerminal(mut invalid) = decoded.clone() else {
        return Err(fixture_error("create request decoded to the wrong variant").into());
    };
    invalid.launch.terminal_profile.default_background_rgb = MAXIMUM_RGB_VALUE + 1;
    assert_code(
        encode_terminal_creation_control(&TerminalCreationControl::CreateTerminal(invalid)),
        ProtocolErrorCode::InvalidField,
    );

    let TerminalCreationControl::CreateTerminal(mut invalid) = decoded.clone() else {
        return Err(fixture_error("create request decoded to the wrong variant").into());
    };
    invalid.launch.arguments = vec![String::new(); MAXIMUM_LAUNCH_ARGUMENTS + 1];
    assert_code(
        encode_terminal_creation_control(&TerminalCreationControl::CreateTerminal(invalid)),
        ProtocolErrorCode::InvalidField,
    );

    let TerminalCreationControl::CreateTerminal(mut duplicate_environment) = decoded.clone() else {
        return Err(fixture_error("create request decoded to the wrong variant").into());
    };
    duplicate_environment
        .launch
        .environment
        .push(duplicate_environment.launch.environment[0].clone());
    assert_code(
        encode_terminal_creation_control(&TerminalCreationControl::CreateTerminal(
            duplicate_environment,
        )),
        ProtocolErrorCode::InvalidField,
    );

    let TerminalCreationControl::CreateTerminal(mut invalid_configuration) = decoded.clone() else {
        return Err(fixture_error("create request decoded to the wrong variant").into());
    };
    invalid_configuration.launch.configuration = Some(TerminalConfiguration {
        scrollback_bytes: MAXIMUM_SCROLLBACK_BYTES + 1,
    });
    assert_code(
        encode_terminal_creation_control(&TerminalCreationControl::CreateTerminal(
            invalid_configuration,
        )),
        ProtocolErrorCode::InvalidField,
    );

    let TerminalCreationControl::CreateTerminal(mut invalid_geometry) = decoded.clone() else {
        return Err(fixture_error("create request decoded to the wrong variant").into());
    };
    invalid_geometry.launch.start_mode = LaunchStartMode::Immediate(TerminalPixelGeometry {
        cell_pixel_height: 1,
        cell_pixel_width: 1,
        columns: 4096,
        rows: 4096,
    });
    assert_code(
        encode_terminal_creation_control(&TerminalCreationControl::CreateTerminal(
            invalid_geometry,
        )),
        ProtocolErrorCode::InvalidField,
    );

    let TerminalCreationControl::CreateTerminal(mut duplicate_feature) = decoded.clone() else {
        return Err(fixture_error("create request decoded to the wrong variant").into());
    };
    duplicate_feature
        .launch
        .terminal_profile
        .device_attributes
        .primary_features = vec![1, 1];
    assert_code(
        encode_terminal_creation_control(&TerminalCreationControl::CreateTerminal(
            duplicate_feature,
        )),
        ProtocolErrorCode::InvalidField,
    );

    let TerminalCreationControl::CreateTerminal(mut too_large) = decoded else {
        return Err(fixture_error("create request decoded to the wrong variant").into());
    };
    too_large.launch.arguments = vec!["x".repeat(MAXIMUM_CONTROL_JSON_BYTES)];
    assert_code(
        encode_terminal_creation_control(&TerminalCreationControl::CreateTerminal(too_large)),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn creation_control_kind_role_and_aggregate_bounds_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let request = fixture_string(&fixture, "/commands/create_terminal")?;
    let response = fixture_string(&fixture, "/outputs/terminal_identity")?;
    assert!(decode_terminal_creation_control(11, MessageRole::Request, request.as_bytes()).is_ok());
    assert!(
        decode_terminal_creation_control(11, MessageRole::Response, response.as_bytes()).is_ok()
    );
    for (role, payload) in [
        (MessageRole::Response, request),
        (MessageRole::Event, request),
        (MessageRole::Request, response),
        (MessageRole::Event, response),
    ] {
        assert_payload_code(role, payload, ProtocolErrorCode::InvalidField);
    }
    for kind in [10, 12] {
        assert_code(
            decode_terminal_creation_control(kind, MessageRole::Request, request.as_bytes()),
            ProtocolErrorCode::InvalidField,
        );
    }

    let exact = format!(
        "{request}{}",
        " ".repeat(MAXIMUM_CONTROL_JSON_BYTES - request.len())
    );
    assert_eq!(exact.len(), MAXIMUM_CONTROL_JSON_BYTES);
    assert!(decode_terminal_creation_control(11, MessageRole::Request, exact.as_bytes()).is_ok());
    let over = format!("{exact} ");
    assert_code(
        decode_terminal_creation_control(11, MessageRole::Request, over.as_bytes()),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn creation_controls_compose_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (role, pointer) in [
        (MessageRole::Request, "/commands/create_terminal"),
        (MessageRole::Response, "/outputs/terminal_identity"),
    ] {
        let value = decode_terminal_creation_control(
            11,
            role,
            fixture_string(&fixture, pointer)?.as_bytes(),
        )?;
        let encoded = encode_terminal_creation_control(&value)?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: Some(CorrelationId::parse(&"55".repeat(16))?),
            host_epoch: Some(HostEpoch::parse(&"11".repeat(16))?),
            kind: encoded.kind,
            optional_message: false,
            payload: &encoded.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: encoded.role,
        })?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(
            decode_terminal_creation_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn assert_array_bound(
    canonical: &str,
    pointer: &str,
    exact: Vec<Value>,
    over_entry: Value,
) -> Result<(), Box<dyn Error>> {
    let mut exact_value: Value = serde_json::from_str(canonical)?;
    set_pointer(&mut exact_value, pointer, Value::Array(exact))?;
    assert!(decode_value(MessageRole::Request, exact_value.clone()).is_ok());
    array_mut_at(&mut exact_value, pointer)?.push(over_entry);
    assert_value_code(
        MessageRole::Request,
        exact_value,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

fn assert_string_bound(
    canonical: &str,
    pointer: &str,
    maximum: usize,
) -> Result<(), Box<dyn Error>> {
    let mut exact: Value = serde_json::from_str(canonical)?;
    set_pointer(&mut exact, pointer, Value::String("x".repeat(maximum)))?;
    assert!(decode_value(MessageRole::Request, exact.clone()).is_ok());
    set_pointer(&mut exact, pointer, Value::String("x".repeat(maximum + 1)))?;
    assert_value_code(MessageRole::Request, exact, ProtocolErrorCode::InvalidField);
    Ok(())
}

fn assert_fixture_limit(
    fixture: &Value,
    pointer: &str,
    actual: usize,
) -> Result<(), Box<dyn Error>> {
    let expected = fixture
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| fixture_error(&format!("fixture limit {pointer} is missing")))?;
    assert_eq!(u64::try_from(actual)?, expected);
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
        .ok_or_else(|| fixture_error(&format!("fixture string {pointer} is missing")))
}

fn decode_value(role: MessageRole, value: Value) -> Result<TerminalCreationControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_terminal_creation_control(11, role, &payload)
}

fn object_mut_at<'value>(
    value: &'value mut Value,
    pointer: &str,
) -> Result<&'value mut Map<String, Value>, io::Error> {
    let target = if pointer.is_empty() {
        value
    } else {
        value
            .pointer_mut(pointer)
            .ok_or_else(|| fixture_error(&format!("fixture pointer {pointer} is missing")))?
    };
    target
        .as_object_mut()
        .ok_or_else(|| fixture_error("fixture value is not an object"))
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

fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| fixture_error(&format!("fixture pointer {pointer} is missing")))?;
    *target = replacement;
    Ok(())
}

fn assert_value_code(role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(role, value), expected);
}

fn assert_payload_code(role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_terminal_creation_control(11, role, payload.as_bytes()),
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
