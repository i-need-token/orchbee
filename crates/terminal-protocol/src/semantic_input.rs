use std::num::NonZeroU64;

use crate::{
    AttachmentId, ProtocolError, ProtocolErrorCode,
    input_authority::{InputAuthority, append_input_authority, decode_input_authority},
    limits::{
        MAXIMUM_INPUT_BYTES, MAXIMUM_KEY_TEXT_BYTES, MAXIMUM_PASTE_AUTO_INPUT_BYTES,
        MAXIMUM_SEMANTIC_INPUT_PAYLOAD_BYTES,
    },
};

pub const KEY_INPUT_HEADER_BYTES: usize = 40;
pub const TEXT_INPUT_HEADER_BYTES: usize = 28;
pub const PASTE_INPUT_HEADER_BYTES: usize = 32;
pub const PASTE_AUTO_INPUT_HEADER_BYTES: usize = 32;
pub const MOUSE_INPUT_BYTES: usize = 40;
pub const FOCUS_INPUT_BYTES: usize = 28;

const KEY_FLAGS_MASK: u8 = 0b11;
const PASTE_FLAGS_MASK: u8 = 0b1;
const MODIFIER_FLAGS_MASK: u16 = 0x03ff;
const MOUSE_BUTTON_FLAGS_MASK: u16 = 0x07ff;
const MAXIMUM_UNICODE_CODEPOINT: u32 = 0x10_ffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyAction {
    Release,
    Press,
    Repeat,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalKeyCode(String);

impl TerminalKeyCode {
    pub fn parse(value: &str) -> Result<Self, ProtocolError> {
        if !is_terminal_key_code(value) {
            return Err(invalid_field(
                "key identity is not in the terminal protocol vocabulary",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_string(value: String) -> Result<Self, ProtocolError> {
        if !is_terminal_key_code(&value) {
            return Err(invalid_field(
                "key identity is not in the terminal protocol vocabulary",
            ));
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalModifiers {
    pub alt: bool,
    pub alt_right: bool,
    pub caps_lock: bool,
    pub control: bool,
    pub control_right: bool,
    pub meta: bool,
    pub meta_right: bool,
    pub num_lock: bool,
    pub shift: bool,
    pub shift_right: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyInput {
    pub action: KeyAction,
    pub attachment_id: AttachmentId,
    pub capability_revision: NonZeroU64,
    pub composing: bool,
    pub consumed_modifiers: TerminalModifiers,
    pub key: TerminalKeyCode,
    pub modifiers: TerminalModifiers,
    pub option_as_alt: bool,
    pub text: String,
    pub unshifted_codepoint: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInput {
    pub attachment_id: AttachmentId,
    pub capability_revision: NonZeroU64,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasteInput {
    pub attachment_id: AttachmentId,
    pub bracketed: bool,
    pub capability_revision: NonZeroU64,
    pub text: String,
}

/// Raw-byte semantic paste. Unlike [`PasteInput`], the content is opaque:
/// it is not UTF-8 validated and the Host decides whether to add the
/// bracketed-paste markers from the terminal's own mode under the terminal
/// mutex. The flags byte and the three reserved bytes must be zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasteAutoInput {
    pub attachment_id: AttachmentId,
    pub capability_revision: NonZeroU64,
    pub content: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseAction {
    Press,
    Release,
    Motion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MouseButton {
    Left = 1,
    Right = 2,
    Middle = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Eleven = 11,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseInput {
    pub action: MouseAction,
    pub attachment_id: AttachmentId,
    pub button: Option<MouseButton>,
    pub capability_revision: NonZeroU64,
    pub modifiers: TerminalModifiers,
    pub pressed_buttons: u16,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusInput {
    pub attachment_id: AttachmentId,
    pub capability_revision: NonZeroU64,
    pub focused: bool,
}

pub(crate) fn decode_key_input(payload: &[u8]) -> Result<KeyInput, ProtocolError> {
    if payload.len() < KEY_INPUT_HEADER_BYTES {
        return Err(truncated("KeyInput header is truncated"));
    }
    if payload.len() > MAXIMUM_SEMANTIC_INPUT_PAYLOAD_BYTES {
        return Err(limit_exceeded("KeyInput payload exceeds its limit"));
    }

    let authority = decode_input_authority(payload)?;
    let action = key_action_from_code(read_u8(payload, 24)?)?;
    let flags = read_u8(payload, 25)?;
    let modifiers = modifiers_from_bits(read_u16(payload, 26)?)?;
    let consumed_modifiers = modifiers_from_bits(read_u16(payload, 28)?)?;
    let key_length = usize::from(read_u16(payload, 30)?);
    let unshifted_codepoint = read_u32(payload, 32)?;
    let text_length = usize::try_from(read_u32(payload, 36)?)
        .map_err(|_error| limit_exceeded("KeyInput text length cannot fit this target"))?;

    if flags & !KEY_FLAGS_MASK != 0 {
        return Err(unknown_required_flag("KeyInput has unknown flags"));
    }
    validate_consumed_modifiers(modifiers, consumed_modifiers)?;
    if text_length > MAXIMUM_KEY_TEXT_BYTES {
        return Err(limit_exceeded("KeyInput text exceeds its limit"));
    }
    let body_length = key_length
        .checked_add(text_length)
        .ok_or_else(|| limit_exceeded("KeyInput body length overflows this target"))?;
    if body_length != payload.len() - KEY_INPUT_HEADER_BYTES {
        return Err(length_mismatch("KeyInput lengths do not match payload"));
    }

    validate_unicode_codepoint(unshifted_codepoint)?;
    let key_end = KEY_INPUT_HEADER_BYTES
        .checked_add(key_length)
        .ok_or_else(|| limit_exceeded("KeyInput key range overflows this target"))?;
    let key = TerminalKeyCode::from_string(decode_utf8(
        bytes_at(payload, KEY_INPUT_HEADER_BYTES, key_length)?,
        "KeyInput key is not valid UTF-8",
    )?)?;
    let text = decode_utf8(
        payload
            .get(key_end..)
            .ok_or_else(|| truncated("KeyInput text range is truncated"))?,
        "KeyInput text is not valid UTF-8",
    )?;

    Ok(KeyInput {
        action,
        attachment_id: authority.attachment_id,
        capability_revision: authority.capability_revision,
        composing: flags & 1 != 0,
        consumed_modifiers,
        key,
        modifiers,
        option_as_alt: flags & (1 << 1) != 0,
        text,
        unshifted_codepoint,
    })
}

pub(crate) fn encode_key_input(value: &KeyInput) -> Result<Vec<u8>, ProtocolError> {
    validate_key_input(value)?;
    let key_bytes = value.key.as_str().as_bytes();
    let text_bytes = value.text.as_bytes();
    let total_length = KEY_INPUT_HEADER_BYTES
        .checked_add(key_bytes.len())
        .and_then(|length| length.checked_add(text_bytes.len()))
        .ok_or_else(|| limit_exceeded("KeyInput payload length overflows this target"))?;
    if total_length > MAXIMUM_SEMANTIC_INPUT_PAYLOAD_BYTES {
        return Err(limit_exceeded("KeyInput payload exceeds its limit"));
    }
    let key_length = u16::try_from(key_bytes.len())
        .map_err(|_error| invalid_field("KeyInput key length exceeds its wire field"))?;
    let text_length = u32::try_from(text_bytes.len())
        .map_err(|_error| invalid_field("KeyInput text length exceeds its wire field"))?;

    let mut payload = Vec::with_capacity(total_length);
    append_authority(&mut payload, value.attachment_id, value.capability_revision);
    payload.push(key_action_code(value.action));
    payload.push(u8::from(value.composing) | (u8::from(value.option_as_alt) << 1));
    append_u16(&mut payload, modifiers_bits(value.modifiers));
    append_u16(&mut payload, modifiers_bits(value.consumed_modifiers));
    append_u16(&mut payload, key_length);
    append_u32(&mut payload, value.unshifted_codepoint);
    append_u32(&mut payload, text_length);
    payload.extend_from_slice(key_bytes);
    payload.extend_from_slice(text_bytes);
    Ok(payload)
}

pub(crate) fn decode_text_input(payload: &[u8]) -> Result<TextInput, ProtocolError> {
    if payload.len() < TEXT_INPUT_HEADER_BYTES {
        return Err(truncated("TextInput header is truncated"));
    }
    if payload.len() > TEXT_INPUT_HEADER_BYTES + MAXIMUM_INPUT_BYTES {
        return Err(limit_exceeded("TextInput payload exceeds its limit"));
    }
    let authority = decode_input_authority(payload)?;
    let text = decode_declared_text(payload, TEXT_INPUT_HEADER_BYTES, 24, "TextInput")?;
    validate_input_text(&text)?;
    Ok(TextInput {
        attachment_id: authority.attachment_id,
        capability_revision: authority.capability_revision,
        text,
    })
}

pub(crate) fn encode_text_input(value: &TextInput) -> Result<Vec<u8>, ProtocolError> {
    validate_input_text(&value.text)?;
    encode_text_payload(
        value.attachment_id,
        value.capability_revision,
        &value.text,
        TEXT_INPUT_HEADER_BYTES,
        None,
    )
}

pub(crate) fn decode_paste_input(payload: &[u8]) -> Result<PasteInput, ProtocolError> {
    if payload.len() < PASTE_INPUT_HEADER_BYTES {
        return Err(truncated("PasteInput header is truncated"));
    }
    if payload.len() > MAXIMUM_SEMANTIC_INPUT_PAYLOAD_BYTES {
        return Err(limit_exceeded("PasteInput payload exceeds its limit"));
    }
    let authority = decode_input_authority(payload)?;
    let flags = read_u8(payload, 24)?;
    let reserved_byte = read_u8(payload, 25)?;
    let reserved_word = read_u16(payload, 26)?;
    if flags & !PASTE_FLAGS_MASK != 0 || reserved_byte != 0 || reserved_word != 0 {
        return Err(unknown_required_flag("PasteInput has unknown flags"));
    }
    let text = decode_declared_text(payload, PASTE_INPUT_HEADER_BYTES, 28, "PasteInput")?;
    validate_input_text(&text)?;
    Ok(PasteInput {
        attachment_id: authority.attachment_id,
        bracketed: flags & 1 != 0,
        capability_revision: authority.capability_revision,
        text,
    })
}

pub(crate) fn encode_paste_input(value: &PasteInput) -> Result<Vec<u8>, ProtocolError> {
    validate_input_text(&value.text)?;
    encode_text_payload(
        value.attachment_id,
        value.capability_revision,
        &value.text,
        PASTE_INPUT_HEADER_BYTES,
        Some(value.bracketed),
    )
}

pub(crate) fn decode_paste_auto_input(payload: &[u8]) -> Result<PasteAutoInput, ProtocolError> {
    if payload.len() < PASTE_AUTO_INPUT_HEADER_BYTES {
        return Err(truncated("PasteAutoInput header is truncated"));
    }
    if payload.len() > PASTE_AUTO_INPUT_HEADER_BYTES + MAXIMUM_PASTE_AUTO_INPUT_BYTES {
        return Err(limit_exceeded("PasteAutoInput payload exceeds its limit"));
    }
    let authority = decode_input_authority(payload)?;
    let flags = read_u8(payload, 24)?;
    let reserved = bytes_at(payload, 25, 3)?;
    if flags != 0 || reserved.iter().any(|byte| *byte != 0) {
        return Err(unknown_required_flag(
            "PasteAutoInput has unknown flags or reserved bytes",
        ));
    }
    let content_length = usize::try_from(read_u32(payload, 28)?)
        .map_err(|_error| length_mismatch("PasteAutoInput length cannot fit this target"))?;
    if content_length != payload.len() - PASTE_AUTO_INPUT_HEADER_BYTES {
        return Err(length_mismatch(
            "PasteAutoInput length does not match payload",
        ));
    }
    let content = payload
        .get(PASTE_AUTO_INPUT_HEADER_BYTES..)
        .ok_or_else(|| truncated("PasteAutoInput content range is truncated"))?
        .to_vec();
    validate_paste_auto_content(&content)?;
    Ok(PasteAutoInput {
        attachment_id: authority.attachment_id,
        capability_revision: authority.capability_revision,
        content,
    })
}

pub(crate) fn encode_paste_auto_input(value: &PasteAutoInput) -> Result<Vec<u8>, ProtocolError> {
    validate_paste_auto_content(&value.content)?;
    let content_length = u32::try_from(value.content.len())
        .map_err(|_error| invalid_field("PasteAutoInput content exceeds its wire length field"))?;
    let mut payload = Vec::with_capacity(PASTE_AUTO_INPUT_HEADER_BYTES + value.content.len());
    append_authority(&mut payload, value.attachment_id, value.capability_revision);
    payload.push(0);
    payload.extend_from_slice(&[0; 3]);
    append_u32(&mut payload, content_length);
    payload.extend_from_slice(&value.content);
    Ok(payload)
}

fn validate_paste_auto_content(content: &[u8]) -> Result<(), ProtocolError> {
    if content.is_empty() || content.len() > MAXIMUM_PASTE_AUTO_INPUT_BYTES {
        return Err(invalid_field(
            "semantic paste content must contain one byte through the paste maximum",
        ));
    }
    Ok(())
}

pub(crate) fn decode_mouse_input(payload: &[u8]) -> Result<MouseInput, ProtocolError> {
    if payload.len() < MOUSE_INPUT_BYTES {
        return Err(truncated("MouseInput payload is truncated"));
    }
    if payload.len() != MOUSE_INPUT_BYTES {
        return Err(length_mismatch("MouseInput must be exactly 40 bytes"));
    }

    let authority = decode_input_authority(payload)?;
    let action = mouse_action_from_code(read_u8(payload, 24)?)?;
    let button = mouse_button_from_code(read_u8(payload, 25)?)?;
    let modifiers = modifiers_from_bits(read_u16(payload, 26)?)?;
    let x = f32::from_bits(read_u32(payload, 28)?);
    let y = f32::from_bits(read_u32(payload, 32)?);
    let pressed_buttons = read_u16(payload, 36)?;
    let reserved = read_u16(payload, 38)?;
    if reserved != 0 {
        return Err(unknown_required_flag("MouseInput has reserved bits"));
    }

    let value = MouseInput {
        action,
        attachment_id: authority.attachment_id,
        button,
        capability_revision: authority.capability_revision,
        modifiers,
        pressed_buttons,
        x,
        y,
    };
    validate_mouse_input(value)?;
    Ok(value)
}

pub(crate) fn encode_mouse_input(value: MouseInput) -> Result<Vec<u8>, ProtocolError> {
    validate_mouse_input(value)?;
    let mut payload = Vec::with_capacity(MOUSE_INPUT_BYTES);
    append_authority(&mut payload, value.attachment_id, value.capability_revision);
    payload.push(mouse_action_code(value.action));
    payload.push(value.button.map_or(0, mouse_button_code));
    append_u16(&mut payload, modifiers_bits(value.modifiers));
    append_u32(&mut payload, value.x.to_bits());
    append_u32(&mut payload, value.y.to_bits());
    append_u16(&mut payload, value.pressed_buttons);
    append_u16(&mut payload, 0);
    Ok(payload)
}

pub(crate) fn decode_focus_input(payload: &[u8]) -> Result<FocusInput, ProtocolError> {
    if payload.len() < FOCUS_INPUT_BYTES {
        return Err(truncated("FocusInput payload is truncated"));
    }
    if payload.len() != FOCUS_INPUT_BYTES {
        return Err(length_mismatch("FocusInput must be exactly 28 bytes"));
    }

    let authority = decode_input_authority(payload)?;
    let focused = read_u8(payload, 24)?;
    let reserved = bytes_at(payload, 25, 3)?;
    if (focused != 0 && focused != 1) || reserved.iter().any(|byte| *byte != 0) {
        return Err(unknown_required_flag("FocusInput has invalid flags"));
    }
    Ok(FocusInput {
        attachment_id: authority.attachment_id,
        capability_revision: authority.capability_revision,
        focused: focused == 1,
    })
}

pub(crate) fn encode_focus_input(value: FocusInput) -> Vec<u8> {
    let mut payload = Vec::with_capacity(FOCUS_INPUT_BYTES);
    append_authority(&mut payload, value.attachment_id, value.capability_revision);
    payload.push(u8::from(value.focused));
    payload.extend_from_slice(&[0; 3]);
    payload
}

fn validate_key_input(value: &KeyInput) -> Result<(), ProtocolError> {
    validate_modifiers(value.modifiers)?;
    validate_modifiers(value.consumed_modifiers)?;
    validate_consumed_modifiers(value.modifiers, value.consumed_modifiers)?;
    validate_unicode_codepoint(value.unshifted_codepoint)?;
    if value.text.len() > MAXIMUM_KEY_TEXT_BYTES {
        return Err(invalid_field("KeyInput text exceeds its UTF-8 byte limit"));
    }
    Ok(())
}

fn validate_input_text(text: &str) -> Result<(), ProtocolError> {
    if text.is_empty() || text.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid_field(
            "semantic text must contain one UTF-8 byte through the protocol maximum",
        ));
    }
    Ok(())
}

fn validate_mouse_input(value: MouseInput) -> Result<(), ProtocolError> {
    validate_modifiers(value.modifiers)?;
    if value.pressed_buttons & !MOUSE_BUTTON_FLAGS_MASK != 0 {
        return Err(invalid_field(
            "MouseInput pressed buttons exceed the protocol mask",
        ));
    }
    if !value.x.is_finite() || !value.y.is_finite() {
        return Err(invalid_field(
            "MouseInput coordinates must be finite float32 values",
        ));
    }
    if value.action != MouseAction::Motion && value.button.is_none() {
        return Err(invalid_field(
            "MouseInput press and release require a button",
        ));
    }
    if value.action == MouseAction::Motion {
        match value.button {
            None if value.pressed_buttons != 0 => {
                return Err(invalid_field(
                    "mouse motion with pressed buttons requires an active button",
                ));
            }
            Some(button) if value.pressed_buttons & mouse_button_mask(button) == 0 => {
                return Err(invalid_field(
                    "mouse motion button is absent from the pressed-button state",
                ));
            }
            None | Some(_) => {}
        }
    }
    Ok(())
}

fn validate_unicode_codepoint(value: u32) -> Result<(), ProtocolError> {
    if value > MAXIMUM_UNICODE_CODEPOINT || (0xd800..=0xdfff).contains(&value) {
        return Err(invalid_field("unshifted codepoint is not a Unicode scalar"));
    }
    Ok(())
}

fn validate_modifiers(value: TerminalModifiers) -> Result<(), ProtocolError> {
    for (right, base) in [
        (value.alt_right, value.alt),
        (value.control_right, value.control),
        (value.meta_right, value.meta),
        (value.shift_right, value.shift),
    ] {
        if right && !base {
            return Err(invalid_field(
                "right-side modifier requires its base modifier",
            ));
        }
    }
    Ok(())
}

fn validate_consumed_modifiers(
    modifiers: TerminalModifiers,
    consumed: TerminalModifiers,
) -> Result<(), ProtocolError> {
    if modifiers_bits(consumed) & !modifiers_bits(modifiers) != 0 {
        return Err(invalid_field(
            "consumed modifiers must be a subset of active modifiers",
        ));
    }
    Ok(())
}

fn modifiers_from_bits(bits: u16) -> Result<TerminalModifiers, ProtocolError> {
    if bits & !MODIFIER_FLAGS_MASK != 0 {
        return Err(unknown_required_flag(
            "terminal modifiers contain unknown required bits",
        ));
    }
    let value = TerminalModifiers {
        alt: bits & (1 << 2) != 0,
        alt_right: bits & (1 << 8) != 0,
        caps_lock: bits & (1 << 4) != 0,
        control: bits & (1 << 1) != 0,
        control_right: bits & (1 << 7) != 0,
        meta: bits & (1 << 3) != 0,
        meta_right: bits & (1 << 9) != 0,
        num_lock: bits & (1 << 5) != 0,
        shift: bits & 1 != 0,
        shift_right: bits & (1 << 6) != 0,
    };
    validate_modifiers(value)?;
    Ok(value)
}

fn modifiers_bits(value: TerminalModifiers) -> u16 {
    u16::from(value.shift)
        | (u16::from(value.control) << 1)
        | (u16::from(value.alt) << 2)
        | (u16::from(value.meta) << 3)
        | (u16::from(value.caps_lock) << 4)
        | (u16::from(value.num_lock) << 5)
        | (u16::from(value.shift_right) << 6)
        | (u16::from(value.control_right) << 7)
        | (u16::from(value.alt_right) << 8)
        | (u16::from(value.meta_right) << 9)
}

fn key_action_from_code(value: u8) -> Result<KeyAction, ProtocolError> {
    match value {
        0 => Ok(KeyAction::Release),
        1 => Ok(KeyAction::Press),
        2 => Ok(KeyAction::Repeat),
        _ => Err(invalid_field("KeyInput action code is unknown")),
    }
}

const fn key_action_code(value: KeyAction) -> u8 {
    match value {
        KeyAction::Release => 0,
        KeyAction::Press => 1,
        KeyAction::Repeat => 2,
    }
}

fn mouse_action_from_code(value: u8) -> Result<MouseAction, ProtocolError> {
    match value {
        0 => Ok(MouseAction::Press),
        1 => Ok(MouseAction::Release),
        2 => Ok(MouseAction::Motion),
        _ => Err(invalid_field("MouseInput action code is unknown")),
    }
}

const fn mouse_action_code(value: MouseAction) -> u8 {
    match value {
        MouseAction::Press => 0,
        MouseAction::Release => 1,
        MouseAction::Motion => 2,
    }
}

fn mouse_button_from_code(value: u8) -> Result<Option<MouseButton>, ProtocolError> {
    let button = match value {
        0 => return Ok(None),
        1 => MouseButton::Left,
        2 => MouseButton::Right,
        3 => MouseButton::Middle,
        4 => MouseButton::Four,
        5 => MouseButton::Five,
        6 => MouseButton::Six,
        7 => MouseButton::Seven,
        8 => MouseButton::Eight,
        9 => MouseButton::Nine,
        10 => MouseButton::Ten,
        11 => MouseButton::Eleven,
        _ => return Err(invalid_field("MouseInput button code is unknown")),
    };
    Ok(Some(button))
}

const fn mouse_button_code(value: MouseButton) -> u8 {
    value as u8
}

const fn mouse_button_mask(value: MouseButton) -> u16 {
    1 << (mouse_button_code(value) - 1)
}

fn encode_text_payload(
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
    text: &str,
    header_bytes: usize,
    bracketed: Option<bool>,
) -> Result<Vec<u8>, ProtocolError> {
    let total_length = header_bytes
        .checked_add(text.len())
        .ok_or_else(|| limit_exceeded("semantic text payload length overflows this target"))?;
    let text_length = u32::try_from(text.len())
        .map_err(|_error| invalid_field("semantic text exceeds its wire length field"))?;
    let mut payload = Vec::with_capacity(total_length);
    append_authority(&mut payload, attachment_id, capability_revision);
    if let Some(bracketed) = bracketed {
        payload.push(u8::from(bracketed));
        payload.extend_from_slice(&[0; 3]);
    }
    append_u32(&mut payload, text_length);
    payload.extend_from_slice(text.as_bytes());
    Ok(payload)
}

fn decode_declared_text(
    payload: &[u8],
    header_bytes: usize,
    length_offset: usize,
    name: &'static str,
) -> Result<String, ProtocolError> {
    let text_length = usize::try_from(read_u32(payload, length_offset)?)
        .map_err(|_error| length_mismatch("semantic text length cannot fit this target"))?;
    if text_length != payload.len() - header_bytes {
        return Err(length_mismatch(match name {
            "PasteInput" => "PasteInput length does not match payload",
            _ => "TextInput length does not match payload",
        }));
    }
    decode_utf8(
        payload
            .get(header_bytes..)
            .ok_or_else(|| truncated("semantic text range is truncated"))?,
        match name {
            "PasteInput" => "PasteInput text is not valid UTF-8",
            _ => "TextInput text is not valid UTF-8",
        },
    )
}

fn append_authority(
    payload: &mut Vec<u8>,
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
) {
    append_input_authority(
        payload,
        InputAuthority {
            attachment_id,
            capability_revision,
        },
    );
}

fn decode_utf8(bytes: &[u8], detail: &'static str) -> Result<String, ProtocolError> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_error| ProtocolError::new(ProtocolErrorCode::InvalidUtf8, detail))
}

fn read_u8(payload: &[u8], offset: usize) -> Result<u8, ProtocolError> {
    payload
        .get(offset)
        .copied()
        .ok_or_else(|| truncated("semantic input fixed field is truncated"))
}

fn read_u16(payload: &[u8], offset: usize) -> Result<u16, ProtocolError> {
    read_array::<2>(payload, offset).map(u16::from_le_bytes)
}

fn read_u32(payload: &[u8], offset: usize) -> Result<u32, ProtocolError> {
    read_array::<4>(payload, offset).map(u32::from_le_bytes)
}

fn read_array<const LENGTH: usize>(
    payload: &[u8],
    offset: usize,
) -> Result<[u8; LENGTH], ProtocolError> {
    payload
        .get(offset..offset.saturating_add(LENGTH))
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| truncated("semantic input fixed field is truncated"))
}

fn bytes_at(payload: &[u8], offset: usize, length: usize) -> Result<&[u8], ProtocolError> {
    payload
        .get(offset..offset.saturating_add(length))
        .ok_or_else(|| truncated("semantic input byte range is truncated"))
}

fn append_u16(payload: &mut Vec<u8>, value: u16) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn append_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn is_terminal_key_code(value: &str) -> bool {
    if value.strip_prefix("Key").is_some_and(|suffix| {
        suffix.len() == 1 && suffix.bytes().all(|byte| byte.is_ascii_uppercase())
    }) || value
        .strip_prefix("Digit")
        .is_some_and(|suffix| suffix.len() == 1 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
        || value.strip_prefix("Numpad").is_some_and(|suffix| {
            suffix.len() == 1 && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
        || is_function_key(value)
    {
        return true;
    }

    matches!(
        value,
        "Unidentified"
            | "Backquote"
            | "Backslash"
            | "BracketLeft"
            | "BracketRight"
            | "Comma"
            | "Equal"
            | "IntlBackslash"
            | "IntlRo"
            | "IntlYen"
            | "Minus"
            | "Period"
            | "Quote"
            | "Semicolon"
            | "Slash"
            | "AltLeft"
            | "AltRight"
            | "Backspace"
            | "CapsLock"
            | "ContextMenu"
            | "ControlLeft"
            | "ControlRight"
            | "Enter"
            | "MetaLeft"
            | "MetaRight"
            | "ShiftLeft"
            | "ShiftRight"
            | "Space"
            | "Tab"
            | "Convert"
            | "KanaMode"
            | "NonConvert"
            | "Delete"
            | "End"
            | "Help"
            | "Home"
            | "Insert"
            | "PageDown"
            | "PageUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "ArrowUp"
            | "NumLock"
            | "NumpadAdd"
            | "NumpadBackspace"
            | "NumpadClear"
            | "NumpadClearEntry"
            | "NumpadComma"
            | "NumpadDecimal"
            | "NumpadDivide"
            | "NumpadEnter"
            | "NumpadEqual"
            | "NumpadMemoryAdd"
            | "NumpadMemoryClear"
            | "NumpadMemoryRecall"
            | "NumpadMemoryStore"
            | "NumpadMemorySubtract"
            | "NumpadMultiply"
            | "NumpadParenLeft"
            | "NumpadParenRight"
            | "NumpadSubtract"
            | "NumpadSeparator"
            | "NumpadUp"
            | "NumpadDown"
            | "NumpadRight"
            | "NumpadLeft"
            | "NumpadBegin"
            | "NumpadHome"
            | "NumpadEnd"
            | "NumpadInsert"
            | "NumpadDelete"
            | "NumpadPageUp"
            | "NumpadPageDown"
            | "Escape"
            | "Fn"
            | "FnLock"
            | "PrintScreen"
            | "ScrollLock"
            | "Pause"
            | "BrowserBack"
            | "BrowserFavorites"
            | "BrowserForward"
            | "BrowserHome"
            | "BrowserRefresh"
            | "BrowserSearch"
            | "BrowserStop"
            | "Eject"
            | "LaunchApp1"
            | "LaunchApp2"
            | "LaunchMail"
            | "MediaPlayPause"
            | "MediaSelect"
            | "MediaStop"
            | "MediaTrackNext"
            | "MediaTrackPrevious"
            | "Power"
            | "Sleep"
            | "AudioVolumeDown"
            | "AudioVolumeMute"
            | "AudioVolumeUp"
            | "WakeUp"
            | "Copy"
            | "Cut"
            | "Paste"
    )
}

fn is_function_key(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix('F') else {
        return false;
    };
    if suffix.is_empty()
        || suffix.starts_with('0')
        || !suffix.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    suffix
        .parse::<u8>()
        .is_ok_and(|number| (1..=25).contains(&number))
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}

const fn truncated(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::Truncated, detail)
}

const fn length_mismatch(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::LengthMismatch, detail)
}

const fn limit_exceeded(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::LimitExceeded, detail)
}

const fn unknown_required_flag(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::UnknownRequiredFlag, detail)
}
