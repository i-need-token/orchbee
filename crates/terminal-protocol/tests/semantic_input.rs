use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    EncodedInputMessage, Envelope, FOCUS_INPUT_BYTES, InputMessage, KEY_INPUT_HEADER_BYTES,
    KeyAction, MAXIMUM_INPUT_BYTES, MAXIMUM_KEY_TEXT_BYTES, MAXIMUM_PASTE_AUTO_INPUT_BYTES,
    MAXIMUM_PASTE_AUTO_INPUT_PAYLOAD_BYTES, MAXIMUM_SEMANTIC_INPUT_PAYLOAD_BYTES,
    MOUSE_INPUT_BYTES, MessageRole, MouseAction, MouseButton, PASTE_AUTO_INPUT_HEADER_BYTES,
    PASTE_INPUT_HEADER_BYTES, PasteAutoInput, PasteInput, PayloadEncoding, ProtocolError,
    ProtocolErrorCode, TEXT_INPUT_HEADER_BYTES, TerminalKeyCode, TextInput, decode_input_message,
    decode_transport_envelope, encode_input_message, encode_transport_envelope,
};

const KEY_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/key-input-v0.0.25.bin");
const TEXT_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/text-input-v0.0.25.bin");
const MOUSE_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/mouse-input-v0.0.25.bin");
const FOCUS_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/focus-input-v0.0.25.bin");
const PASTE_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/paste-input-v0.0.25.bin");
const PASTE_AUTO_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/paste-auto-input-v0.0.25.bin");

#[test]
fn every_canonical_semantic_input_decodes_and_reencodes_byte_for_byte() -> Result<(), Box<dyn Error>>
{
    for (kind, payload) in fixture_cases() {
        let decoded = decode_input_message(kind, MessageRole::Request, payload)?;
        let encoded = encode_input_message(&decoded)?;
        assert_eq!(encoded.kind, kind);
        assert_eq!(encoded.role, MessageRole::Request);
        assert_eq!(encoded.payload, payload);
    }
    Ok(())
}

#[test]
fn canonical_payloads_retain_complete_semantic_input_facts() -> Result<(), Box<dyn Error>> {
    let InputMessage::KeyInput(key) = decode_input_message(34, MessageRole::Request, KEY_FIXTURE)?
    else {
        return Err(fixture_error("key fixture decoded to the wrong variant").into());
    };
    assert_authority(key.attachment_id.to_text(), key.capability_revision);
    assert_eq!(key.action, KeyAction::Press);
    assert!(!key.composing);
    assert!(key.modifiers.shift);
    assert!(key.modifiers.shift_right);
    assert!(key.consumed_modifiers.shift);
    assert!(!key.consumed_modifiers.shift_right);
    assert_eq!(key.key.as_str(), "KeyA");
    assert!(key.option_as_alt);
    assert_eq!(key.text, "A");
    assert_eq!(key.unshifted_codepoint, u32::from('a'));

    let InputMessage::TextInput(text) =
        decode_input_message(35, MessageRole::Request, TEXT_FIXTURE)?
    else {
        return Err(fixture_error("text fixture decoded to the wrong variant").into());
    };
    assert_authority(text.attachment_id.to_text(), text.capability_revision);
    assert_eq!(text.text, "中文");

    let InputMessage::MouseInput(mouse) =
        decode_input_message(36, MessageRole::Request, MOUSE_FIXTURE)?
    else {
        return Err(fixture_error("mouse fixture decoded to the wrong variant").into());
    };
    assert_authority(mouse.attachment_id.to_text(), mouse.capability_revision);
    assert_eq!(mouse.action, MouseAction::Motion);
    assert_eq!(mouse.button, Some(MouseButton::Left));
    assert!(mouse.modifiers.alt);
    assert_eq!(mouse.pressed_buttons, 1);
    assert_eq!(mouse.x, 10.5);
    assert_eq!(mouse.y, -2.25);

    let InputMessage::FocusInput(focus) =
        decode_input_message(37, MessageRole::Request, FOCUS_FIXTURE)?
    else {
        return Err(fixture_error("focus fixture decoded to the wrong variant").into());
    };
    assert_authority(focus.attachment_id.to_text(), focus.capability_revision);
    assert!(focus.focused);

    let InputMessage::PasteInput(paste) =
        decode_input_message(38, MessageRole::Request, PASTE_FIXTURE)?
    else {
        return Err(fixture_error("paste fixture decoded to the wrong variant").into());
    };
    assert_authority(paste.attachment_id.to_text(), paste.capability_revision);
    assert!(paste.bracketed);
    assert_eq!(paste.text, "line 1\nline 2");

    let InputMessage::PasteAutoInput(paste) =
        decode_input_message(39, MessageRole::Request, PASTE_AUTO_FIXTURE)?
    else {
        return Err(fixture_error("paste-auto fixture decoded to the wrong variant").into());
    };
    assert_authority(paste.attachment_id.to_text(), paste.capability_revision);
    assert_eq!(paste.content, b"\x02\x80\xff\x1b\r\nraw-paste".as_slice());
    Ok(())
}

#[test]
fn every_semantic_input_uses_the_same_nonzero_little_endian_authority_header()
-> Result<(), Box<dyn Error>> {
    for (kind, payload) in fixture_cases() {
        assert_eq!(payload.get(..16), Some([0x22; 16].as_slice()));
        assert_eq!(payload.get(16..24), Some(7_u64.to_le_bytes().as_slice()));

        let mut zero_identity = payload.to_vec();
        zero_identity
            .get_mut(..16)
            .ok_or_else(|| fixture_error("fixture authority identity is truncated"))?
            .fill(0);
        assert_code(
            decode_input_message(kind, MessageRole::Request, &zero_identity),
            ProtocolErrorCode::InvalidIdentifier,
        );

        let mut zero_revision = payload.to_vec();
        write_u64(&mut zero_revision, 16, 0)?;
        assert_code(
            decode_input_message(kind, MessageRole::Request, &zero_revision),
            ProtocolErrorCode::InvalidField,
        );

        let mut maximum_revision = payload.to_vec();
        write_u64(&mut maximum_revision, 16, u64::MAX)?;
        let decoded = decode_input_message(kind, MessageRole::Request, &maximum_revision)?;
        assert_eq!(encode_input_message(&decoded)?.payload, maximum_revision);
    }
    Ok(())
}

#[test]
fn key_input_enforces_actions_flags_modifiers_lengths_and_unicode() -> Result<(), Box<dyn Error>> {
    let mut invalid_action = KEY_FIXTURE.to_vec();
    invalid_action[24] = 3;
    assert_payload_code(34, &invalid_action, ProtocolErrorCode::InvalidField);

    let mut unknown_flag = KEY_FIXTURE.to_vec();
    unknown_flag[25] |= 1 << 2;
    assert_payload_code(34, &unknown_flag, ProtocolErrorCode::UnknownRequiredFlag);

    let mut unknown_modifier = KEY_FIXTURE.to_vec();
    write_u16(&mut unknown_modifier, 26, 1 << 12)?;
    assert_payload_code(
        34,
        &unknown_modifier,
        ProtocolErrorCode::UnknownRequiredFlag,
    );

    let mut side_without_base = KEY_FIXTURE.to_vec();
    write_u16(&mut side_without_base, 26, 1 << 6)?;
    assert_payload_code(34, &side_without_base, ProtocolErrorCode::InvalidField);

    let mut consumed_but_inactive = KEY_FIXTURE.to_vec();
    write_u16(&mut consumed_but_inactive, 28, 1 << 1)?;
    assert_payload_code(34, &consumed_but_inactive, ProtocolErrorCode::InvalidField);

    for codepoint in [0x00d800, 0x11_0000] {
        let mut invalid_codepoint = KEY_FIXTURE.to_vec();
        write_u32(&mut invalid_codepoint, 32, codepoint)?;
        assert_payload_code(34, &invalid_codepoint, ProtocolErrorCode::InvalidField);
    }

    let mut malformed_key = KEY_FIXTURE.to_vec();
    malformed_key[40] = 0xff;
    assert_payload_code(34, &malformed_key, ProtocolErrorCode::InvalidUtf8);

    let mut unknown_key = KEY_FIXTURE.to_vec();
    unknown_key[43] = b'0';
    assert_payload_code(34, &unknown_key, ProtocolErrorCode::InvalidField);

    let mut mismatched_key_length = KEY_FIXTURE.to_vec();
    write_u16(&mut mismatched_key_length, 30, 3)?;
    assert_payload_code(
        34,
        &mismatched_key_length,
        ProtocolErrorCode::LengthMismatch,
    );

    let mut oversized_declared_text = KEY_FIXTURE.to_vec();
    write_u32(
        &mut oversized_declared_text,
        36,
        u32::try_from(MAXIMUM_KEY_TEXT_BYTES)? + 1,
    )?;
    assert_payload_code(
        34,
        &oversized_declared_text,
        ProtocolErrorCode::LimitExceeded,
    );

    assert_eq!(TerminalKeyCode::parse("F1")?.as_str(), "F1");
    assert_eq!(TerminalKeyCode::parse("F25")?.as_str(), "F25");
    assert_code(
        TerminalKeyCode::parse("F+1"),
        ProtocolErrorCode::InvalidField,
    );

    let InputMessage::KeyInput(mut key) =
        decode_input_message(34, MessageRole::Request, KEY_FIXTURE)?
    else {
        return Err(fixture_error("key fixture decoded to the wrong variant").into());
    };
    key.text = "a".repeat(MAXIMUM_KEY_TEXT_BYTES);
    assert!(encode_input_message(&InputMessage::KeyInput(key.clone())).is_ok());
    key.text.push('a');
    assert_code(
        encode_input_message(&InputMessage::KeyInput(key)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn text_and_paste_enforce_utf8_exact_lengths_flags_and_bounds() -> Result<(), Box<dyn Error>> {
    let mut zero_declared_text = TEXT_FIXTURE.to_vec();
    write_u32(&mut zero_declared_text, 24, 0)?;
    assert_payload_code(35, &zero_declared_text, ProtocolErrorCode::LengthMismatch);
    let header_only_text = authority_payload(TEXT_INPUT_HEADER_BYTES, 7)?;
    assert_payload_code(35, &header_only_text, ProtocolErrorCode::InvalidField);
    let mut malformed_text = TEXT_FIXTURE.to_vec();
    malformed_text[28] = 0xff;
    assert_payload_code(35, &malformed_text, ProtocolErrorCode::InvalidUtf8);

    let mut unknown_paste_flag = PASTE_FIXTURE.to_vec();
    unknown_paste_flag[24] = 2;
    assert_payload_code(
        38,
        &unknown_paste_flag,
        ProtocolErrorCode::UnknownRequiredFlag,
    );
    for offset in [25, 26, 27] {
        let mut reserved = PASTE_FIXTURE.to_vec();
        reserved[offset] = 1;
        assert_payload_code(38, &reserved, ProtocolErrorCode::UnknownRequiredFlag);
    }
    let mut zero_declared_paste = PASTE_FIXTURE.to_vec();
    write_u32(&mut zero_declared_paste, 28, 0)?;
    assert_payload_code(38, &zero_declared_paste, ProtocolErrorCode::LengthMismatch);

    for (kind, header_bytes, variant) in [
        (35, TEXT_INPUT_HEADER_BYTES, false),
        (38, PASTE_INPUT_HEADER_BYTES, true),
    ] {
        let text = "a".repeat(MAXIMUM_INPUT_BYTES);
        let value = if variant {
            InputMessage::PasteInput(PasteInput {
                attachment_id: fixture_attachment_id()?,
                bracketed: false,
                capability_revision: NonZeroU64::new(7)
                    .ok_or_else(|| fixture_error("fixture revision is zero"))?,
                text,
            })
        } else {
            InputMessage::TextInput(TextInput {
                attachment_id: fixture_attachment_id()?,
                capability_revision: NonZeroU64::new(7)
                    .ok_or_else(|| fixture_error("fixture revision is zero"))?,
                text,
            })
        };
        let encoded = encode_input_message(&value)?;
        assert_eq!(encoded.kind, kind);
        assert_eq!(encoded.payload.len(), header_bytes + MAXIMUM_INPUT_BYTES);
        let mut oversized = value;
        match &mut oversized {
            InputMessage::TextInput(value) => value.text.push('a'),
            InputMessage::PasteInput(value) => value.text.push('a'),
            _ => return Err(fixture_error("text boundary used the wrong variant").into()),
        }
        assert_code(
            encode_input_message(&oversized),
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn semantic_text_preserves_a_leading_unicode_bom() -> Result<(), Box<dyn Error>> {
    for (kind, fixture) in [(35, TEXT_FIXTURE), (38, PASTE_FIXTURE)] {
        let mut value = decode_input_message(kind, MessageRole::Request, fixture)?;
        match &mut value {
            InputMessage::TextInput(value) => value.text = "\u{feff}".to_owned(),
            InputMessage::PasteInput(value) => value.text = "\u{feff}".to_owned(),
            _ => return Err(fixture_error("BOM case decoded to the wrong variant").into()),
        }
        let encoded = encode_input_message(&value)?;
        assert_eq!(
            encoded.payload.get(encoded.payload.len() - 3..),
            Some([0xef, 0xbb, 0xbf].as_slice())
        );
        assert_eq!(
            decode_input_message(kind, MessageRole::Request, &encoded.payload)?,
            value
        );
    }
    Ok(())
}

#[test]
fn mouse_input_enforces_float_modifier_button_and_motion_invariants() -> Result<(), Box<dyn Error>>
{
    for (offset, code) in [
        (24, ProtocolErrorCode::InvalidField),
        (25, ProtocolErrorCode::InvalidField),
    ] {
        let mut invalid = MOUSE_FIXTURE.to_vec();
        invalid[offset] = if offset == 24 { 3 } else { 12 };
        assert_payload_code(36, &invalid, code);
    }
    let mut unknown_modifier = MOUSE_FIXTURE.to_vec();
    write_u16(&mut unknown_modifier, 26, 1 << 12)?;
    assert_payload_code(
        36,
        &unknown_modifier,
        ProtocolErrorCode::UnknownRequiredFlag,
    );
    let mut reserved = MOUSE_FIXTURE.to_vec();
    reserved[38] = 1;
    assert_payload_code(36, &reserved, ProtocolErrorCode::UnknownRequiredFlag);
    let mut pressed_overflow = MOUSE_FIXTURE.to_vec();
    write_u16(&mut pressed_overflow, 36, 1 << 11)?;
    assert_payload_code(36, &pressed_overflow, ProtocolErrorCode::InvalidField);
    let mut infinite = MOUSE_FIXTURE.to_vec();
    write_u32(&mut infinite, 28, f32::INFINITY.to_bits())?;
    assert_payload_code(36, &infinite, ProtocolErrorCode::InvalidField);

    let InputMessage::MouseInput(mut mouse) =
        decode_input_message(36, MessageRole::Request, MOUSE_FIXTURE)?
    else {
        return Err(fixture_error("mouse fixture decoded to the wrong variant").into());
    };
    mouse.button = None;
    assert_code(
        encode_input_message(&InputMessage::MouseInput(mouse)),
        ProtocolErrorCode::InvalidField,
    );

    let InputMessage::MouseInput(mut mouse) =
        decode_input_message(36, MessageRole::Request, MOUSE_FIXTURE)?
    else {
        return Err(fixture_error("mouse fixture decoded to the wrong variant").into());
    };
    mouse.button = Some(MouseButton::Right);
    assert_code(
        encode_input_message(&InputMessage::MouseInput(mouse)),
        ProtocolErrorCode::InvalidField,
    );

    let InputMessage::MouseInput(mut wheel) =
        decode_input_message(36, MessageRole::Request, MOUSE_FIXTURE)?
    else {
        return Err(fixture_error("mouse fixture decoded to the wrong variant").into());
    };
    wheel.action = MouseAction::Press;
    wheel.button = Some(MouseButton::Four);
    wheel.pressed_buttons = 0;
    wheel.x = f32::MAX;
    wheel.y = -f32::MAX;
    let encoded = encode_input_message(&InputMessage::MouseInput(wheel))?;
    assert_eq!(encoded.payload.len(), MOUSE_INPUT_BYTES);

    wheel.x = f32::NAN;
    assert_code(
        encode_input_message(&InputMessage::MouseInput(wheel)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn focus_and_all_semantic_kinds_have_exact_physical_boundaries() -> Result<(), Box<dyn Error>> {
    assert_eq!(KEY_INPUT_HEADER_BYTES, 40);
    assert_eq!(TEXT_INPUT_HEADER_BYTES, 28);
    assert_eq!(PASTE_INPUT_HEADER_BYTES, 32);
    assert_eq!(PASTE_AUTO_INPUT_HEADER_BYTES, 32);
    assert_eq!(MOUSE_INPUT_BYTES, 40);
    assert_eq!(FOCUS_INPUT_BYTES, 28);
    assert_eq!(
        MAXIMUM_SEMANTIC_INPUT_PAYLOAD_BYTES,
        PASTE_INPUT_HEADER_BYTES + MAXIMUM_INPUT_BYTES
    );
    assert_eq!(
        MAXIMUM_PASTE_AUTO_INPUT_PAYLOAD_BYTES,
        PASTE_AUTO_INPUT_HEADER_BYTES + MAXIMUM_PASTE_AUTO_INPUT_BYTES
    );

    assert_code(
        decode_input_message(37, MessageRole::Request, &FOCUS_FIXTURE[..27]),
        ProtocolErrorCode::Truncated,
    );
    let mut long_focus = FOCUS_FIXTURE.to_vec();
    long_focus.push(0);
    assert_payload_code(37, &long_focus, ProtocolErrorCode::LengthMismatch);
    for (offset, value) in [(24, 2), (25, 1), (26, 1), (27, 1)] {
        let mut invalid = FOCUS_FIXTURE.to_vec();
        invalid[offset] = value;
        assert_payload_code(37, &invalid, ProtocolErrorCode::UnknownRequiredFlag);
    }

    assert_code(
        decode_input_message(36, MessageRole::Request, &MOUSE_FIXTURE[..39]),
        ProtocolErrorCode::Truncated,
    );
    let mut long_mouse = MOUSE_FIXTURE.to_vec();
    long_mouse.push(0);
    assert_payload_code(36, &long_mouse, ProtocolErrorCode::LengthMismatch);

    let oversized = vec![0; MAXIMUM_SEMANTIC_INPUT_PAYLOAD_BYTES + 1];
    assert_payload_code(34, &oversized, ProtocolErrorCode::LimitExceeded);
    assert_payload_code(38, &oversized, ProtocolErrorCode::LimitExceeded);
    assert_payload_code(39, &oversized, ProtocolErrorCode::LimitExceeded);

    for (kind, role) in [
        (34, MessageRole::Response),
        (34, MessageRole::Event),
        (35, MessageRole::Response),
        (36, MessageRole::Event),
        (37, MessageRole::Response),
        (38, MessageRole::Event),
        (39, MessageRole::Response),
        (39, MessageRole::Event),
    ] {
        assert_code(
            decode_input_message(kind, role, KEY_FIXTURE),
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn paste_auto_input_carries_opaque_bytes_with_exact_bounds() -> Result<(), Box<dyn Error>> {
    let decoded = decode_input_message(39, MessageRole::Request, PASTE_AUTO_FIXTURE)?;
    let encoded = encode_input_message(&decoded)?;
    assert_eq!(encoded.kind, 39);
    assert_eq!(encoded.role, MessageRole::Request);
    assert_eq!(encoded.payload, PASTE_AUTO_FIXTURE);

    for offset in 24..28 {
        let mut invalid = PASTE_AUTO_FIXTURE.to_vec();
        invalid[offset] = 1;
        assert_payload_code(39, &invalid, ProtocolErrorCode::UnknownRequiredFlag);
    }

    let mut mismatched = PASTE_AUTO_FIXTURE.to_vec();
    write_u32(&mut mismatched, 28, 0)?;
    assert_payload_code(39, &mismatched, ProtocolErrorCode::LengthMismatch);

    let header_only = authority_payload(PASTE_AUTO_INPUT_HEADER_BYTES, 7)?;
    assert_payload_code(39, &header_only, ProtocolErrorCode::InvalidField);

    assert_code(
        decode_input_message(39, MessageRole::Request, &PASTE_AUTO_FIXTURE[..31]),
        ProtocolErrorCode::Truncated,
    );

    let attachment_id = fixture_attachment_id()?;
    let capability_revision =
        NonZeroU64::new(7).ok_or_else(|| fixture_error("fixture revision is zero"))?;
    for length in [1, MAXIMUM_PASTE_AUTO_INPUT_BYTES] {
        let content = vec![0xff; length];
        let encoded = encode_input_message(&InputMessage::PasteAutoInput(PasteAutoInput {
            attachment_id,
            capability_revision,
            content: content.clone(),
        }))?;
        assert_eq!(
            encoded.payload.len(),
            PASTE_AUTO_INPUT_HEADER_BYTES + length
        );
        let decoded = decode_input_message(39, MessageRole::Request, &encoded.payload)?;
        assert_eq!(
            decoded,
            InputMessage::PasteAutoInput(PasteAutoInput {
                attachment_id,
                capability_revision,
                content,
            })
        );
    }
    for length in [0, MAXIMUM_PASTE_AUTO_INPUT_BYTES + 1] {
        assert_code(
            encode_input_message(&InputMessage::PasteAutoInput(PasteAutoInput {
                attachment_id,
                capability_revision,
                content: vec![0x61; length],
            })),
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn semantic_input_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    for (kind, payload) in fixture_cases() {
        let value = decode_input_message(kind, MessageRole::Request, payload)?;
        let EncodedInputMessage {
            kind,
            payload,
            role,
        } = encode_input_message(&value)?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: None,
            kind,
            optional_message: false,
            payload: &payload,
            payload_encoding: PayloadEncoding::RawBytes,
            protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
            role,
        })?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(envelope.payload_encoding, PayloadEncoding::RawBytes);
        assert_eq!(
            decode_input_message(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn fixture_cases() -> [(u16, &'static [u8]); 6] {
    [
        (34, KEY_FIXTURE),
        (35, TEXT_FIXTURE),
        (36, MOUSE_FIXTURE),
        (37, FOCUS_FIXTURE),
        (38, PASTE_FIXTURE),
        (39, PASTE_AUTO_FIXTURE),
    ]
}

fn fixture_attachment_id() -> Result<orchbee_terminal_protocol::AttachmentId, ProtocolError> {
    orchbee_terminal_protocol::AttachmentId::parse(&"22".repeat(16)).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "fixture attachment identity is invalid",
        )
    })
}

fn assert_authority(attachment_id: String, capability_revision: NonZeroU64) {
    assert_eq!(attachment_id, "22".repeat(16));
    assert_eq!(capability_revision.get(), 7);
}

fn authority_payload(length: usize, revision: u64) -> Result<Vec<u8>, io::Error> {
    let mut payload = vec![0; length];
    payload
        .get_mut(..16)
        .ok_or_else(|| fixture_error("authority payload identity is truncated"))?
        .fill(0x22);
    write_u64(&mut payload, 16, revision)?;
    Ok(payload)
}

fn write_u16(payload: &mut [u8], offset: usize, value: u16) -> Result<(), io::Error> {
    write_bytes(payload, offset, &value.to_le_bytes())
}

fn write_u32(payload: &mut [u8], offset: usize, value: u32) -> Result<(), io::Error> {
    write_bytes(payload, offset, &value.to_le_bytes())
}

fn write_u64(payload: &mut [u8], offset: usize, value: u64) -> Result<(), io::Error> {
    write_bytes(payload, offset, &value.to_le_bytes())
}

fn write_bytes(payload: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), io::Error> {
    let target = payload
        .get_mut(offset..offset.saturating_add(bytes.len()))
        .ok_or_else(|| fixture_error("test mutation exceeds the payload"))?;
    target.copy_from_slice(bytes);
    Ok(())
}

fn assert_payload_code(kind: u16, payload: &[u8], expected: ProtocolErrorCode) {
    assert_code(
        decode_input_message(kind, MessageRole::Request, payload),
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
