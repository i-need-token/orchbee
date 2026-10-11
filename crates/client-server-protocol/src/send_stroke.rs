//! Validated semantic presses beyond the frozen legacy combo vocabulary.
use crate::{ProtocolError, SendKey};

/// A key identity, independent of its modifiers and the terminal's encoding mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendStrokeKey {
    Named(SendKey),
    Insert,
    Function(u8),
    Character(char),
}

#[cfg(test)]
mod tests;

/// Control=1, Alt=2, Shift=4, Super=8; zero means an unmodified press.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendKeyStroke {
    key: SendStrokeKey,
    modifiers: u8,
}

impl SendKeyStroke {
    pub fn new(key: SendStrokeKey, modifiers: u8) -> Result<Self, ProtocolError> {
        let valid = match key {
            SendStrokeKey::Named(SendKey::CtrlC | SendKey::CtrlD) => false,
            SendStrokeKey::Named(_) | SendStrokeKey::Insert => true,
            SendStrokeKey::Function(number) => (1..=25).contains(&number),
            SendStrokeKey::Character(value) => !value.is_control(),
        };
        if !valid || modifiers & !15 != 0 {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { key, modifiers })
    }

    #[must_use]
    pub const fn key(self) -> SendStrokeKey {
        self.key
    }

    #[must_use]
    pub const fn modifiers(self) -> u8 {
        self.modifiers
    }

    pub(crate) fn to_wire(self) -> [u8; 6] {
        let (kind, value) = match self.key {
            SendStrokeKey::Named(key) => (1, u32::from(key.code())),
            SendStrokeKey::Insert => (2, 0),
            SendStrokeKey::Function(number) => (3, u32::from(number)),
            SendStrokeKey::Character(value) => (4, u32::from(value)),
        };
        let value = value.to_be_bytes();
        [self.modifiers, kind, value[0], value[1], value[2], value[3]]
    }

    pub(crate) fn from_wire(bytes: [u8; 6]) -> Result<Self, ProtocolError> {
        let value = u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        let invalid = || ProtocolError::InvalidPayload;
        let key = match bytes[1] {
            1 => SendStrokeKey::Named(SendKey::from_code(
                u8::try_from(value).map_err(|_| invalid())?,
            )?),
            2 if value == 0 => SendStrokeKey::Insert,
            3 => SendStrokeKey::Function(u8::try_from(value).map_err(|_| invalid())?),
            4 => SendStrokeKey::Character(char::from_u32(value).ok_or_else(invalid)?),
            _ => return Err(invalid()),
        };
        Self::new(key, bytes[0])
    }
}
