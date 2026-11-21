//! The one closed `ioKind` vocabulary machine documents use to name an operating-system error's
//! kind: the cohort diagnostic of the Client status, and a Server the system refused to start. A
//! kind outside it reads as `other`; the raw system number travels beside it as `osError`.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IoKind {
    NotFound,
    PermissionDenied,
    InvalidInput,
    InvalidData,
    TimedOut,
    Unsupported,
    Interrupted,
    UnexpectedEof,
    WouldBlock,
    OutOfMemory,
    Other,
}

impl IoKind {
    pub(crate) fn of(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::NotFound => Self::NotFound,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::InvalidInput => Self::InvalidInput,
            std::io::ErrorKind::InvalidData => Self::InvalidData,
            std::io::ErrorKind::TimedOut => Self::TimedOut,
            std::io::ErrorKind::Unsupported => Self::Unsupported,
            std::io::ErrorKind::Interrupted => Self::Interrupted,
            std::io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            std::io::ErrorKind::WouldBlock => Self::WouldBlock,
            std::io::ErrorKind::OutOfMemory => Self::OutOfMemory,
            _ => Self::Other,
        }
    }

    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::NotFound => "not-found",
            Self::PermissionDenied => "permission-denied",
            Self::InvalidInput => "invalid-input",
            Self::InvalidData => "invalid-data",
            Self::TimedOut => "timed-out",
            Self::Unsupported => "unsupported",
            Self::Interrupted => "interrupted",
            Self::UnexpectedEof => "unexpected-eof",
            Self::WouldBlock => "would-block",
            Self::OutOfMemory => "out-of-memory",
            Self::Other => "other",
        }
    }
}

#[cfg(test)]
mod tests;
