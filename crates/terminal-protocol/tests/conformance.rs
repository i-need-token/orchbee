use std::error::Error;

use crate::encoded_fixture::base64;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HostEpoch, MAXIMUM_ROW_WINDOW_BYTES,
    MAXIMUM_TRANSPORT_RECORD_BYTES, MessageRole, PayloadEncoding, ProtocolError, ProtocolErrorCode,
    ProtocolVersion, decode_terminal_graphics, decode_terminal_projection_buffer,
    decode_terminal_text_frame, decode_terminal_text_row_window, decode_transport_envelope,
    encode_transport_envelope,
};

const ENVELOPE_FIXTURE: &[u8] =
    include_bytes!("../../../schemas/terminal/v1/fixtures/transport-envelope.bin");
const FRAME_FIXTURE: &[u8] =
    include_bytes!("../../../schemas/terminal/v1/fixtures/terminal-frame.bin");
const OTHER_GENERATION_CORPUS: &str =
    include_str!("../../../schemas/terminal/v1/fixtures/other-generation-conformance.json");

#[test]
fn canonical_envelope_decodes_and_reencodes_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let decoded = decode_transport_envelope(ENVELOPE_FIXTURE)?;
    assert_eq!(decoded.correlation_id, None);
    assert_eq!(decoded.host_epoch, Some(host_epoch(0x11)?));
    assert_eq!(decoded.ignored_optional_flags, 0);
    assert_eq!(decoded.kind, 42);
    assert!(decoded.known_message);
    assert!(!decoded.optional_message);
    assert_eq!(decoded.payload, FRAME_FIXTURE);
    assert_eq!(decoded.payload_encoding, PayloadEncoding::TerminalFrame);
    assert_eq!(decoded.protocol, CURRENT_PROTOCOL_VERSION);
    assert_eq!(decoded.role, MessageRole::Event);

    let encoded = encode_transport_envelope(&Envelope {
        correlation_id: decoded.correlation_id,
        host_epoch: decoded.host_epoch,
        kind: decoded.kind,
        optional_message: decoded.optional_message,
        payload: decoded.payload,
        payload_encoding: decoded.payload_encoding,
        protocol: decoded.protocol,
        role: decoded.role,
    })?;
    assert_eq!(encoded, ENVELOPE_FIXTURE);
    Ok(())
}

#[test]
fn malformed_headers_roles_and_encodings_are_rejected() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_transport_envelope(&ENVELOPE_FIXTURE[..63]),
        ProtocolErrorCode::Truncated,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_byte(&mut mutated, 0, b'X')?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidMagic,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 4, 63)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::UnsupportedVersion,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u32(&mut mutated, 8, 500)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::LengthMismatch,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u32(&mut mutated, 12, 436)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::LengthMismatch,
    );

    for offset in [23, 58] {
        let mut mutated = ENVELOPE_FIXTURE.to_vec();
        set_byte(&mut mutated, offset, 1)?;
        assert_code(
            decode_transport_envelope(&mutated),
            ProtocolErrorCode::UnknownRequiredFlag,
        );
    }

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 6, 0x0104)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::UnknownRequiredFlag,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 6, 0x0006)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidField,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 20, 60_000)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::UnknownRequiredMessage,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_byte(&mut mutated, 22, 0)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidField,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 6, 0)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidField,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_byte(&mut mutated, 22, 1)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn unknown_optional_messages_and_flags_remain_skippable() -> Result<(), Box<dyn Error>> {
    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 6, 0x000d)?;
    set_u16(&mut mutated, 20, 60_000)?;
    let decoded = decode_transport_envelope(&mutated)?;
    assert!(!decoded.known_message);
    assert!(decoded.optional_message);
    assert_eq!(decoded.ignored_optional_flags, 0x08);
    assert_eq!(decoded.role, MessageRole::Event);
    assert_eq!(decoded.payload_encoding, PayloadEncoding::TerminalFrame);
    assert_eq!(decoded.payload, FRAME_FIXTURE);
    Ok(())
}

#[test]
fn encoder_rejects_unknown_required_kinds_and_illegal_contract_pairs() -> Result<(), Box<dyn Error>>
{
    let base = Envelope {
        correlation_id: Some(correlation_id(0x22)?),
        host_epoch: Some(host_epoch(0x11)?),
        kind: 1,
        optional_message: false,
        payload: b"{}",
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    };

    assert_code(
        encode_transport_envelope(&Envelope {
            kind: 60_000,
            ..base
        }),
        ProtocolErrorCode::UnknownRequiredMessage,
    );
    assert_code(
        encode_transport_envelope(&Envelope {
            role: MessageRole::Event,
            ..base
        }),
        ProtocolErrorCode::InvalidField,
    );
    assert_code(
        encode_transport_envelope(&Envelope {
            payload_encoding: PayloadEncoding::RawBytes,
            ..base
        }),
        ProtocolErrorCode::InvalidField,
    );

    let optional = encode_transport_envelope(&Envelope {
        kind: 60_000,
        optional_message: true,
        payload_encoding: PayloadEncoding::RawBytes,
        role: MessageRole::Event,
        ..base
    })?;
    let decoded = decode_transport_envelope(&optional)?;
    assert!(!decoded.known_message);
    assert!(decoded.optional_message);
    Ok(())
}

#[test]
fn representative_legal_roles_round_trip_without_payload_reinterpretation()
-> Result<(), Box<dyn Error>> {
    let cases = [
        (1, MessageRole::Request, PayloadEncoding::ControlJson),
        (2, MessageRole::Response, PayloadEncoding::ControlJson),
        (30, MessageRole::Request, PayloadEncoding::RawBytes),
        (42, MessageRole::Event, PayloadEncoding::TerminalFrame),
        (46, MessageRole::Response, PayloadEncoding::RowWindow),
    ];
    let payloads = [0_usize, 1, 63, 512];

    for (kind, role, payload_encoding) in cases {
        for payload_length in payloads {
            let payload = (0..payload_length)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>();
            let value = Envelope {
                correlation_id: Some(correlation_id(0x22)?),
                host_epoch: Some(host_epoch(0x11)?),
                kind,
                optional_message: false,
                payload: &payload,
                payload_encoding,
                protocol: CURRENT_PROTOCOL_VERSION,
                role,
            };
            let encoded = encode_transport_envelope(&value)?;
            let decoded = decode_transport_envelope(&encoded)?;
            assert_eq!(decoded.correlation_id, value.correlation_id);
            assert_eq!(decoded.host_epoch, value.host_epoch);
            assert_eq!(decoded.kind, value.kind);
            assert_eq!(decoded.optional_message, value.optional_message);
            assert_eq!(decoded.payload, value.payload);
            assert_eq!(decoded.payload_encoding, value.payload_encoding);
            assert_eq!(decoded.protocol, value.protocol);
            assert_eq!(decoded.role, value.role);
        }
    }
    Ok(())
}

#[test]
fn aggregate_record_boundary_is_exact() -> Result<(), Box<dyn Error>> {
    let payload = vec![0; MAXIMUM_ROW_WINDOW_BYTES + 1];
    let base = Envelope {
        correlation_id: Some(correlation_id(0x22)?),
        host_epoch: Some(host_epoch(0x11)?),
        kind: 46,
        optional_message: false,
        payload: &payload[..MAXIMUM_ROW_WINDOW_BYTES],
        payload_encoding: PayloadEncoding::RowWindow,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    };
    let encoded = encode_transport_envelope(&base)?;
    assert_eq!(encoded.len(), MAXIMUM_TRANSPORT_RECORD_BYTES);
    assert_eq!(
        decode_transport_envelope(&encoded)?.payload.len(),
        MAXIMUM_ROW_WINDOW_BYTES
    );
    drop(encoded);

    assert_code(
        encode_transport_envelope(&Envelope {
            payload: &payload,
            ..base
        }),
        ProtocolErrorCode::LimitExceeded,
    );

    let mut overlong_record = vec![0; MAXIMUM_TRANSPORT_RECORD_BYTES + 1];
    overlong_record[..4].copy_from_slice(b"OBTP");
    assert_code(
        decode_transport_envelope(&overlong_record),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn every_boundary_refuses_a_record_of_another_generation() -> Result<(), Box<dyn Error>> {
    let corpus: serde_json::Value = serde_json::from_str(OTHER_GENERATION_CORPUS)?;
    assert_eq!(corpus["schemaVersion"], 1);
    let mut codecs = Vec::new();
    for case in corpus["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        let codec = case["codec"].as_str().ok_or("codec")?;
        let bytes = base64(case["bytesBase64"].as_str().ok_or("bytes")?)?;
        let expected = &case["expected"];
        codecs.push(codec);
        if codec == "OBTP" {
            // The envelope codec carries the peer version; the receiver refuses any version
            // other than the current generation and answers with it.
            assert_eq!(expected["result"], "receiver-refusal", "{name}");
            let component = |field: &str| -> Result<u16, Box<dyn Error>> {
                Ok(u16::try_from(
                    expected["protocol"][field].as_u64().ok_or("protocol")?,
                )?)
            };
            let protocol = decode_transport_envelope(&bytes)?.protocol;
            assert_eq!(
                protocol,
                ProtocolVersion {
                    major: component("major")?,
                    minor: component("minor")?,
                    patch: component("patch")?,
                },
                "{name}"
            );
            assert_ne!(protocol.major, CURRENT_PROTOCOL_VERSION.major, "{name}");
            continue;
        }
        let error = match codec {
            "OBTF" => decode_terminal_text_frame(&bytes).err(),
            "OBPB" => decode_terminal_projection_buffer(&bytes).err(),
            "OBRW" => decode_terminal_text_row_window(&bytes).err(),
            "OBKG" => decode_terminal_graphics(&bytes).err(),
            _ => return Err(format!("{name}: unknown codec {codec}").into()),
        }
        .ok_or_else(|| format!("{name}: accepted another generation"))?;
        assert_eq!(expected["result"], "failure", "{name}");
        assert_eq!(
            error.code().to_string(),
            expected["code"].as_str().ok_or("code")?,
            "{name}"
        );
    }
    codecs.sort_unstable();
    assert_eq!(codecs, ["OBKG", "OBPB", "OBRW", "OBTF", "OBTP"]);
    Ok(())
}

fn correlation_id(byte: u8) -> Result<CorrelationId, Box<dyn Error>> {
    Ok(CorrelationId::from_bytes([byte; 16])?)
}

fn host_epoch(byte: u8) -> Result<HostEpoch, Box<dyn Error>> {
    Ok(HostEpoch::from_bytes([byte; 16])?)
}

fn assert_code<T>(result: Result<T, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Ok(_value) => panic!("expected protocol error {expected}"),
        Err(error) => assert_eq!(error.code(), expected),
    }
}

fn set_byte(bytes: &mut [u8], offset: usize, value: u8) -> Result<(), Box<dyn Error>> {
    let target = bytes
        .get_mut(offset)
        .ok_or_else(|| format!("missing fixture byte {offset}"))?;
    *target = value;
    Ok(())
}

fn set_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<(), Box<dyn Error>> {
    set_bytes(bytes, offset, &value.to_le_bytes())
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), Box<dyn Error>> {
    set_bytes(bytes, offset, &value.to_le_bytes())
}

fn set_bytes(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Box<dyn Error>> {
    let target = bytes
        .get_mut(offset..offset.saturating_add(value.len()))
        .ok_or_else(|| format!("missing fixture range at {offset}"))?;
    target.copy_from_slice(value);
    Ok(())
}
