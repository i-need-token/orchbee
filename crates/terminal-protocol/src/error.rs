use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolErrorCode {
    ArithmeticOverflow,
    InvalidField,
    InvalidIdentifier,
    InvalidJson,
    InvalidMagic,
    InvalidUtf8,
    LengthMismatch,
    LimitExceeded,
    Truncated,
    UnknownRequiredFlag,
    UnknownRequiredMessage,
    UnsupportedVersion,
}

impl fmt::Display for ProtocolErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ArithmeticOverflow => "arithmetic-overflow",
            Self::InvalidField => "invalid-field",
            Self::InvalidIdentifier => "invalid-identifier",
            Self::InvalidJson => "invalid-json",
            Self::InvalidMagic => "invalid-magic",
            Self::InvalidUtf8 => "invalid-utf8",
            Self::LengthMismatch => "length-mismatch",
            Self::LimitExceeded => "limit-exceeded",
            Self::Truncated => "truncated",
            Self::UnknownRequiredFlag => "unknown-required-flag",
            Self::UnknownRequiredMessage => "unknown-required-message",
            Self::UnsupportedVersion => "unsupported-version",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolError {
    code: ProtocolErrorCode,
    detail: &'static str,
}

impl ProtocolError {
    #[must_use]
    pub const fn new(code: ProtocolErrorCode, detail: &'static str) -> Self {
        Self { code, detail }
    }

    #[must_use]
    pub const fn code(self) -> ProtocolErrorCode {
        self.code
    }

    #[must_use]
    pub const fn detail(self) -> &'static str {
        self.detail
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ProtocolError {}
