use std::{fmt, io};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    InvalidMagic,
    IncompatibleFrameVersion { received: u16 },
    PayloadTooLarge { actual: usize, maximum: usize },
    FrameLengthMismatch { expected: usize, actual: usize },
    Utf8Bom,
    InvalidUtf8,
    InvalidPayload,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("invalid Client-Server frame magic"),
            Self::IncompatibleFrameVersion { received } => write!(
                formatter,
                "incompatible Client-Server frame version {received}"
            ),
            Self::PayloadTooLarge { actual, maximum } => write!(
                formatter,
                "Client-Server frame payload has {actual} bytes; maximum is {maximum}"
            ),
            Self::FrameLengthMismatch { expected, actual } => write!(
                formatter,
                "Client-Server frame has {actual} bytes; expected exactly {expected}"
            ),
            Self::Utf8Bom => {
                formatter.write_str("Client-Server JSON payload must not start with a UTF-8 BOM")
            }
            Self::InvalidUtf8 => {
                formatter.write_str("Client-Server JSON payload is not valid UTF-8")
            }
            Self::InvalidPayload => formatter.write_str("invalid Client-Server payload"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug)]
pub enum ReadMessageError {
    Io(io::Error),
    Protocol(ProtocolError),
}

impl fmt::Display for ReadMessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Protocol(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReadMessageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
        }
    }
}

impl From<io::Error> for ReadMessageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProtocolError> for ReadMessageError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

#[derive(Debug)]
pub enum WriteMessageError {
    Io(io::Error),
    Protocol(ProtocolError),
}

impl fmt::Display for WriteMessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Protocol(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WriteMessageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
        }
    }
}

impl From<io::Error> for WriteMessageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProtocolError> for WriteMessageError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}
