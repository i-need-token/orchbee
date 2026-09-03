use std::{fmt, io};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    InvalidMagic,
    IncompatibleVersion { received: u16 },
    PayloadTooLarge { actual: usize, maximum: usize },
    InvalidPayload,
    InvalidIdentity,
    InvalidCwd,
    InvalidGeometry,
    ContradictoryHostObservation,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("invalid Capability Node frame magic"),
            Self::IncompatibleVersion { received } => {
                write!(
                    formatter,
                    "incompatible Capability Node protocol version {received}"
                )
            }
            Self::PayloadTooLarge { actual, maximum } => write!(
                formatter,
                "Capability Node frame payload has {actual} bytes; maximum is {maximum}"
            ),
            Self::InvalidPayload => formatter.write_str("invalid Capability Node frame payload"),
            Self::InvalidIdentity => {
                formatter.write_str("invalid Capability Node nonzero 128-bit identity")
            }
            Self::InvalidCwd => formatter.write_str("invalid authorized terminal cwd"),
            Self::InvalidGeometry => formatter.write_str("invalid terminal pixel geometry"),
            Self::ContradictoryHostObservation => formatter
                .write_str("Capability Node result contradicts its target Host terminal identity"),
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
