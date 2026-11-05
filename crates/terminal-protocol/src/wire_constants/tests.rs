//! Each implemented wire constant is compared with the language-neutral registry.
use std::{collections::BTreeMap, error::Error};

use super::{BinaryFormat, FRAME, GRAPHICS, PROJECTION_BUFFER, ROW_WINDOW, TRANSPORT_MAGIC};
use crate::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, GRAPHICS_DECODE_RESULT_HEADER_BYTES,
    HostEpoch, MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_GRAPHICS_RGBA_BYTES, MAXIMUM_RAW_PAYLOAD_BYTES,
    MAXIMUM_ROW_WINDOW_BYTES, MAXIMUM_TERMINAL_FRAME_BYTES, MAXIMUM_TRANSPORT_RECORD_BYTES,
    MESSAGE_CONTRACTS, MessageRole, PayloadEncoding, TRANSPORT_ENVELOPE_HEADER_BYTES,
    encode_transport_envelope, message_contract,
};
use serde::Deserialize;
use serde_json::json;

const REGISTRY: &str = include_str!("../../../../schemas/terminal/v1/constants.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstantsRegistry {
    color_encoding: serde_json::Value,
    message_contracts: Vec<FixtureMessageContract>,
    protocol_limits: FixtureProtocolLimits,
    formats: serde_json::Value,
    enums: serde_json::Value,
}

#[test]
fn cell_color_codes_match_registry() -> Result<(), Box<dyn Error>> {
    let registry = read_registry()?;
    assert_eq!(
        registry.color_encoding,
        json!({
            "default": crate::COLOR_DEFAULT,
            "none": crate::COLOR_NONE,
            "palette": crate::COLOR_PALETTE,
            "palette_entries": 16,
            "rgb_mask": crate::COLOR_RGB_MASK,
        })
    );
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureMessageContract {
    event_encoding: Option<String>,
    kind: u16,
    name: String,
    request: Option<FixtureRequestContract>,
    response_encoding: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRequestContract {
    payload_encoding: String,
    success_kind: u16,
}

#[derive(Deserialize)]
struct FixtureProtocolLimits {
    launch_arguments: usize,
    launch_environment_entries: usize,
    environment_overlay_entries: usize,
    environment_name_bytes: usize,
    environment_overlay_value_bytes: usize,
    terminal_profile_features: usize,
    terminal_response_bytes: usize,
    terminal_profile_palette_entries: usize,

    control_json_bytes: usize,
    frame_bytes: usize,
    graphics_image_bytes: usize,
    raw_payload_bytes: usize,
    row_window_bytes: usize,
    transport_record_bytes: usize,
}

fn message_contracts_match_registry() -> Result<(), Box<dyn Error>> {
    let fixture = read_registry()?;
    assert_eq!(MESSAGE_CONTRACTS.len(), fixture.message_contracts.len());

    for (actual, expected) in MESSAGE_CONTRACTS.iter().zip(&fixture.message_contracts) {
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.name, expected.name);
        assert_eq!(
            actual.event_encoding,
            expected
                .event_encoding
                .as_deref()
                .map(fixture_payload_encoding)
                .transpose()?
        );
        assert_eq!(
            actual.response_encoding,
            expected
                .response_encoding
                .as_deref()
                .map(fixture_payload_encoding)
                .transpose()?
        );
        match (actual.request, &expected.request) {
            (Some(actual_request), Some(expected_request)) => {
                assert_eq!(
                    actual_request.payload_encoding,
                    fixture_payload_encoding(&expected_request.payload_encoding)?
                );
                assert_eq!(actual_request.success_kind, expected_request.success_kind);
            }
            (None, None) => {}
            _ => panic!("request contract differs for kind {}", actual.kind),
        }
        assert_eq!(message_contract(actual.kind), Some(actual));
    }

    for kind in 0..=u16::MAX {
        let expected = fixture
            .message_contracts
            .binary_search_by_key(&kind, |contract| contract.kind)
            .is_ok();
        assert_eq!(message_contract(kind).is_some(), expected, "kind {kind}");
    }
    Ok(())
}

fn envelope_limits_match_registry() -> Result<(), Box<dyn Error>> {
    let limits = read_registry()?.protocol_limits;
    assert_eq!(crate::MAXIMUM_LAUNCH_ARGUMENTS, limits.launch_arguments);
    assert_eq!(
        crate::MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES,
        limits.launch_environment_entries
    );
    assert_eq!(
        crate::MAXIMUM_ENVIRONMENT_OVERLAY_ENTRIES,
        limits.environment_overlay_entries
    );
    assert_eq!(
        crate::MAXIMUM_ENVIRONMENT_NAME_BYTES,
        limits.environment_name_bytes
    );
    assert_eq!(
        crate::MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES,
        limits.environment_overlay_value_bytes
    );
    assert_eq!(
        crate::MAXIMUM_TERMINAL_PROFILE_FEATURES,
        limits.terminal_profile_features
    );
    assert_eq!(
        crate::MAXIMUM_TERMINAL_RESPONSE_BYTES,
        limits.terminal_response_bytes
    );
    assert_eq!(
        crate::TERMINAL_THEME_PALETTE_ENTRIES,
        limits.terminal_profile_palette_entries
    );

    assert_eq!(MAXIMUM_CONTROL_JSON_BYTES, limits.control_json_bytes);
    assert_eq!(MAXIMUM_RAW_PAYLOAD_BYTES, limits.raw_payload_bytes);
    assert_eq!(MAXIMUM_TERMINAL_FRAME_BYTES, limits.frame_bytes);
    assert_eq!(
        MAXIMUM_GRAPHICS_RGBA_BYTES + GRAPHICS_DECODE_RESULT_HEADER_BYTES,
        limits.graphics_image_bytes
    );
    assert_eq!(MAXIMUM_ROW_WINDOW_BYTES, limits.row_window_bytes);
    assert_eq!(
        MAXIMUM_TRANSPORT_RECORD_BYTES,
        limits.transport_record_bytes
    );
    assert_eq!(
        MAXIMUM_TRANSPORT_RECORD_BYTES,
        TRANSPORT_ENVELOPE_HEADER_BYTES + MAXIMUM_ROW_WINDOW_BYTES
    );
    Ok(())
}

#[test]
fn constants_match_language_neutral_registry() -> Result<(), Box<dyn Error>> {
    let registry = read_registry()?;
    assert_eq!(
        registry.formats,
        json!({
            "wire": {
                "magic": std::str::from_utf8(TRANSPORT_MAGIC)?,
                "header_bytes": TRANSPORT_ENVELOPE_HEADER_BYTES,
                "version": {
                    "major": CURRENT_PROTOCOL_VERSION.major,
                    "minor": CURRENT_PROTOCOL_VERSION.minor,
                    "patch": CURRENT_PROTOCOL_VERSION.patch,
                },
            },
            "frame": format_value(&FRAME)?,
            "projection_buffer": format_value(&PROJECTION_BUFFER)?,
            "row_window": format_value(&ROW_WINDOW)?,
            "graphics": format_value(&GRAPHICS)?,
        })
    );
    let mut roles = BTreeMap::new();
    for (name, role) in [
        ("request", MessageRole::Request),
        ("response", MessageRole::Response),
        ("event", MessageRole::Event),
    ] {
        let bytes = encode_transport_envelope(&Envelope {
            correlation_id: Some(CorrelationId::from_bytes([0x22; 16])?),
            host_epoch: Some(HostEpoch::from_bytes([0x11; 16])?),
            kind: 60_000,
            optional_message: true,
            payload: &[],
            payload_encoding: PayloadEncoding::RawBytes,
            protocol: CURRENT_PROTOCOL_VERSION,
            role,
        })?;
        // Exclude optional-message bit; the remaining flags encode the actual role.
        roles.insert(name, u16::from_le_bytes([bytes[6], bytes[7]]) & !1);
    }
    assert_eq!(
        registry.enums,
        json!({
            "payload_encoding": {
                "control-json": PayloadEncoding::ControlJson.code(),
                "raw-bytes": PayloadEncoding::RawBytes.code(),
                "terminal-frame": PayloadEncoding::TerminalFrame.code(),
                "row-window": PayloadEncoding::RowWindow.code(),
                "graphics-image": PayloadEncoding::GraphicsImage.code(),
            },
            "message_role": roles,
        })
    );
    message_contracts_match_registry()?;
    envelope_limits_match_registry()?;
    Ok(())
}

fn format_value(format: &BinaryFormat) -> Result<serde_json::Value, std::str::Utf8Error> {
    Ok(json!({
        "magic": std::str::from_utf8(format.magic)?,
        "header_bytes": format.header_bytes,
        "version": {"major": format.version[0], "minor": format.version[1], "patch": format.version[2]},
    }))
}

fn read_registry() -> Result<ConstantsRegistry, serde_json::Error> {
    serde_json::from_str(REGISTRY)
}

fn fixture_payload_encoding(value: &str) -> Result<PayloadEncoding, Box<dyn Error>> {
    match value {
        "control-json" => Ok(PayloadEncoding::ControlJson),
        "raw-bytes" => Ok(PayloadEncoding::RawBytes),
        "terminal-frame" => Ok(PayloadEncoding::TerminalFrame),
        "row-window" => Ok(PayloadEncoding::RowWindow),
        "graphics-image" => Ok(PayloadEncoding::GraphicsImage),
        _ => Err(format!("unknown fixture payload encoding {value}").into()),
    }
}
