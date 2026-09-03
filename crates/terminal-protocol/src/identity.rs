use std::fmt;

const IDENTITY_BYTE_LENGTH: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    InvalidFormat,
    ZeroSentinel,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFormat => {
                "identity must contain exactly 32 lowercase hexadecimal characters"
            }
            Self::ZeroSentinel => "identity must not use the zero sentinel",
        })
    }
}

impl std::error::Error for IdentityError {}

macro_rules! define_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; IDENTITY_BYTE_LENGTH]);

        impl $name {
            pub fn from_bytes(bytes: [u8; IDENTITY_BYTE_LENGTH]) -> Result<Self, IdentityError> {
                if bytes.iter().all(|byte| *byte == 0) {
                    return Err(IdentityError::ZeroSentinel);
                }
                Ok(Self(bytes))
            }

            pub fn parse(value: &str) -> Result<Self, IdentityError> {
                if value.len() != IDENTITY_BYTE_LENGTH * 2
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(IdentityError::InvalidFormat);
                }

                let mut bytes = [0_u8; IDENTITY_BYTE_LENGTH];
                for (target, pair) in bytes.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
                    let [high_byte, low_byte] = pair else {
                        return Err(IdentityError::InvalidFormat);
                    };
                    let high = decode_hex_nibble(*high_byte).ok_or(IdentityError::InvalidFormat)?;
                    let low = decode_hex_nibble(*low_byte).ok_or(IdentityError::InvalidFormat)?;
                    *target = (high << 4) | low;
                }
                Self::from_bytes(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; IDENTITY_BYTE_LENGTH] {
                &self.0
            }

            #[must_use]
            pub fn to_text(self) -> String {
                const HEX: &[u8; 16] = b"0123456789abcdef";

                let mut text = String::with_capacity(IDENTITY_BYTE_LENGTH * 2);
                for byte in self.0 {
                    text.push(char::from(HEX[usize::from(byte >> 4)]));
                    text.push(char::from(HEX[usize::from(byte & 0x0f)]));
                }
                text
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.to_text())
            }
        }
    };
}

define_identity!(ClientId);
define_identity!(CorrelationId);
define_identity!(CreationId);
define_identity!(HostEpoch);
define_identity!(AttachmentId);
define_identity!(TerminalId);
define_identity!(TerminalThemeOperationId);

const fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
