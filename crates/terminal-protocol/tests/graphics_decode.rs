use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, Envelope, GraphicsDecodeRequest, GraphicsDecodeResult,
    MessageRole, PayloadEncoding, TerminalId, decode_graphics_decode_request,
    decode_graphics_decode_result, decode_transport_envelope, encode_graphics_decode_request,
    encode_graphics_decode_result, encode_transport_envelope, message_contract,
};

#[test]
fn graphics_decode_contract_fences_attachment_terminal_dimensions_and_exact_pixels() {
    let attachment_id = AttachmentId::from_bytes([1; 16]).unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend(1u32.to_be_bytes());
    png.extend(1u32.to_be_bytes());
    png.extend([0; 9]);
    let request = GraphicsDecodeRequest {
        attachment_id,
        width: 1,
        height: 1,
        png: &png,
    };
    let encoded = encode_graphics_decode_request(&request).unwrap();
    assert_eq!(decode_graphics_decode_request(&encoded).unwrap(), request);
    let result = GraphicsDecodeResult {
        attachment_id,
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        width: 1,
        height: 1,
        rgba: &[255, 0, 0, 255],
    };
    let body = encode_graphics_decode_result(&result).unwrap();
    assert_eq!(decode_graphics_decode_result(&body).unwrap(), result);
    assert_eq!(
        message_contract(80).unwrap().request.unwrap().success_kind,
        81
    );
    let wire = encode_transport_envelope(&Envelope {
        kind: 81,
        payload: &body,
        payload_encoding: PayloadEncoding::GraphicsImage,
        role: MessageRole::Response,
        protocol: CURRENT_PROTOCOL_VERSION,
        optional_message: false,
        correlation_id: None,
        host_epoch: None,
    });
    // Physical response envelopes require correlation and Host epoch.
    assert!(wire.is_err());
    for offset in [6, 32, 36] {
        let mut broken = encoded.clone();
        broken[offset] ^= 1;
        assert!(decode_graphics_decode_request(&broken).is_err());
    }
    for offset in [6, 48, 52, 44] {
        let mut broken = body.clone();
        broken[offset] ^= 1;
        assert!(decode_graphics_decode_result(&broken).is_err());
    }
    let mut overflowing = request;
    overflowing.width = u32::MAX;
    overflowing.height = u32::MAX;
    assert!(encode_graphics_decode_request(&overflowing).is_err());
    let _ = decode_transport_envelope;
}
