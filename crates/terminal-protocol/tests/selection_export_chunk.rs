use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    AttachmentId, EncodedSelectionExportChunk, Envelope, HostEpoch, MAXIMUM_RAW_PAYLOAD_BYTES,
    MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES, MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES,
    MessageRole, PayloadEncoding, ProtocolError, ProtocolErrorCode,
    SELECTION_EXPORT_CHUNK_HEADER_BYTES, SelectionExportChunk, decode_selection_export_chunk,
    decode_selection_export_chunk_route, decode_transport_envelope, encode_selection_export_chunk,
    encode_transport_envelope, message_contract,
};

const FIXTURE: &[u8] = include_bytes!(
    "../../../packages/terminal-protocol/fixtures/selection-export-chunk-v0.0.25.bin"
);

#[test]
fn canonical_chunk_decodes_and_reencodes_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let decoded = decode_selection_export_chunk(65, MessageRole::Event, FIXTURE)?;
    assert_eq!(decoded.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(decoded.export_sequence.get(), 0x0102_0304_0506_0708);
    assert_eq!(decoded.chunk_sequence, 0x0102_0304);
    assert!(decoded.final_chunk);
    assert_eq!(decoded.bytes, [0x00, 0xff, 0x80, 0x61]);

    assert_eq!(FIXTURE.get(..16), Some([0x22; 16].as_slice()));
    assert_eq!(
        FIXTURE.get(16..24),
        Some(0x0102_0304_0506_0708_u64.to_le_bytes().as_slice())
    );
    assert_eq!(
        FIXTURE.get(24..28),
        Some(0x0102_0304_u32.to_le_bytes().as_slice())
    );
    assert_eq!(FIXTURE.get(28..32), Some([1, 0, 0, 0].as_slice()));

    let encoded = encode_selection_export_chunk(&decoded)?;
    assert_eq!(encoded.kind, 65);
    assert_eq!(encoded.role, MessageRole::Event);
    assert_eq!(encoded.payload, FIXTURE);
    Ok(())
}

#[test]
fn route_decode_retains_only_header_facts_and_body_length() -> Result<(), Box<dyn Error>> {
    let route = decode_selection_export_chunk_route(65, MessageRole::Event, FIXTURE)?;
    assert_eq!(route.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(route.export_sequence.get(), 0x0102_0304_0506_0708);
    assert_eq!(route.chunk_sequence, 0x0102_0304);
    assert!(route.final_chunk);
    assert_eq!(route.body_length, 4);
    Ok(())
}

#[test]
fn kind_role_encoding_and_registry_contract_are_exact() -> Result<(), Box<dyn Error>> {
    for (kind, role) in [
        (64, MessageRole::Event),
        (65, MessageRole::Request),
        (65, MessageRole::Response),
        (66, MessageRole::Event),
    ] {
        assert_code(
            decode_selection_export_chunk(kind, role, FIXTURE),
            ProtocolErrorCode::InvalidField,
        );
        assert_code(
            decode_selection_export_chunk_route(kind, role, FIXTURE),
            ProtocolErrorCode::InvalidField,
        );
    }

    let contract =
        message_contract(65).ok_or_else(|| fixture_error("kind 65 is not registered"))?;
    assert_eq!(contract.event_encoding, Some(PayloadEncoding::RawBytes));
    assert_eq!(contract.request, None);
    assert_eq!(contract.response_encoding, None);
    Ok(())
}

#[test]
fn header_body_and_aggregate_bounds_are_exact() -> Result<(), Box<dyn Error>> {
    assert_eq!(SELECTION_EXPORT_CHUNK_HEADER_BYTES, 32);
    assert_eq!(MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES, 1024 * 1024);
    assert_eq!(
        MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES,
        SELECTION_EXPORT_CHUNK_HEADER_BYTES + MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES
    );
    assert_eq!(
        MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES,
        MAXIMUM_RAW_PAYLOAD_BYTES
    );

    assert_code(
        decode_selection_export_chunk(65, MessageRole::Event, &[0; 31]),
        ProtocolErrorCode::Truncated,
    );

    let empty = raw_payload(0x22, 1, 0, false, 0);
    let decoded = decode_selection_export_chunk(65, MessageRole::Event, &empty)?;
    assert!(decoded.bytes.is_empty());
    let route = decode_selection_export_chunk_route(65, MessageRole::Event, &empty)?;
    assert_eq!(route.body_length, 0);

    let maximum = raw_payload(0x22, 1, 0, true, MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES);
    assert_eq!(maximum.len(), MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES);
    assert!(decode_selection_export_chunk(65, MessageRole::Event, &maximum).is_ok());

    let oversized = raw_payload(0x22, 1, 0, true, MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES + 1);
    assert_code(
        decode_selection_export_chunk(65, MessageRole::Event, &oversized),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn identity_sequences_and_little_endian_layout_cover_their_full_ranges()
-> Result<(), Box<dyn Error>> {
    let zero_identity = raw_payload(0, 1, 0, false, 0);
    assert_code(
        decode_selection_export_chunk(65, MessageRole::Event, &zero_identity),
        ProtocolErrorCode::InvalidIdentifier,
    );

    let zero_export = raw_payload(0x22, 0, 0, false, 0);
    assert_code(
        decode_selection_export_chunk(65, MessageRole::Event, &zero_export),
        ProtocolErrorCode::InvalidField,
    );

    let maximum = raw_payload(0x22, u64::MAX, u32::MAX, false, 0);
    assert_eq!(maximum.get(16..24), Some([0xff; 8].as_slice()));
    assert_eq!(maximum.get(24..28), Some([0xff; 4].as_slice()));
    let decoded = decode_selection_export_chunk(65, MessageRole::Event, &maximum)?;
    assert_eq!(decoded.export_sequence, NonZeroU64::MAX);
    assert_eq!(decoded.chunk_sequence, u32::MAX);
    assert_eq!(encode_selection_export_chunk(&decoded)?.payload, maximum);
    Ok(())
}

#[test]
fn final_flag_and_every_reserved_partition_are_strict() -> Result<(), Box<dyn Error>> {
    for final_chunk in [false, true] {
        let payload = raw_payload(0x22, 1, 0, final_chunk, 0);
        let decoded = decode_selection_export_chunk(65, MessageRole::Event, &payload)?;
        assert_eq!(decoded.final_chunk, final_chunk);
        assert_eq!(payload[28], u8::from(final_chunk));
    }

    for bit in 1..8 {
        let mut payload = raw_payload(0x22, 1, 0, false, 0);
        payload[28] = 1_u8 << bit;
        assert_code(
            decode_selection_export_chunk(65, MessageRole::Event, &payload),
            ProtocolErrorCode::UnknownRequiredFlag,
        );
    }
    for offset in 29..32 {
        let mut payload = raw_payload(0x22, 1, 0, false, 0);
        payload[offset] = 1;
        assert_code(
            decode_selection_export_chunk_route(65, MessageRole::Event, &payload),
            ProtocolErrorCode::UnknownRequiredFlag,
        );
    }
    Ok(())
}

#[test]
fn opaque_body_is_owned_and_never_interpreted_as_utf8() -> Result<(), Box<dyn Error>> {
    let mut payload = raw_payload(0x22, 1, 7, true, 4);
    payload[SELECTION_EXPORT_CHUNK_HEADER_BYTES..].copy_from_slice(&[0x00, 0xff, 0x80, 0x61]);
    let expected = payload.clone();
    let decoded = decode_selection_export_chunk(65, MessageRole::Event, &payload)?;
    payload[SELECTION_EXPORT_CHUNK_HEADER_BYTES..].fill(0x55);
    assert_eq!(decoded.bytes, [0x00, 0xff, 0x80, 0x61]);
    assert_eq!(encode_selection_export_chunk(&decoded)?.payload, expected);
    Ok(())
}

#[test]
fn public_values_are_revalidated_before_encoding() -> Result<(), Box<dyn Error>> {
    let attachment_id = AttachmentId::parse(&"22".repeat(16))?;
    let base = SelectionExportChunk {
        attachment_id,
        bytes: Vec::new(),
        chunk_sequence: 0,
        export_sequence: NonZeroU64::MIN,
        final_chunk: false,
    };
    let empty = encode_selection_export_chunk(&base)?;
    assert_eq!(empty.payload.len(), SELECTION_EXPORT_CHUNK_HEADER_BYTES);

    let maximum = SelectionExportChunk {
        bytes: vec![0xa5; MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES],
        ..base.clone()
    };
    assert_eq!(
        encode_selection_export_chunk(&maximum)?.payload.len(),
        MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES
    );

    let oversized = SelectionExportChunk {
        bytes: vec![0; MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES + 1],
        ..base
    };
    assert_code(
        encode_selection_export_chunk(&oversized),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn chunk_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let value = decode_selection_export_chunk(65, MessageRole::Event, FIXTURE)?;
    let EncodedSelectionExportChunk {
        kind,
        payload,
        role,
    } = encode_selection_export_chunk(&value)?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(HostEpoch::parse(&"11".repeat(16))?),
        kind,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::RawBytes,
        protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
        role,
    })?;
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(envelope.kind, 65);
    assert_eq!(envelope.role, MessageRole::Event);
    assert_eq!(envelope.payload_encoding, PayloadEncoding::RawBytes);
    assert_eq!(
        decode_selection_export_chunk(envelope.kind, envelope.role, envelope.payload)?,
        value
    );
    Ok(())
}

fn raw_payload(
    identity_byte: u8,
    export_sequence: u64,
    chunk_sequence: u32,
    final_chunk: bool,
    body_length: usize,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(SELECTION_EXPORT_CHUNK_HEADER_BYTES + body_length);
    payload.extend_from_slice(&[identity_byte; 16]);
    payload.extend_from_slice(&export_sequence.to_le_bytes());
    payload.extend_from_slice(&chunk_sequence.to_le_bytes());
    payload.push(u8::from(final_chunk));
    payload.extend_from_slice(&[0; 3]);
    payload.resize(SELECTION_EXPORT_CHUNK_HEADER_BYTES + body_length, 0x61);
    payload
}

fn fixture_error(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}

fn assert_code<Value>(result: Result<Value, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Ok(_value) => panic!("expected protocol error {expected}"),
        Err(error) => assert_eq!(error.code(), expected),
    }
}
