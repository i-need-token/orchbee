//! Why a saved machine refused this Client. The refusal rides inside the `io::Error` that a
//! connection attempt returns, so discovery keeps it after the attempt's thread ends and the
//! picker and notices name the side that must change.
use std::{error::Error, fmt, io};

/// A definitive refusal: retrying cannot clear it until the user changes one side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The remote Orchbee belongs to another environment kind, or its Session to another
    /// environment.
    OtherEnvironment,
    /// The remote is not Windows on x64 or ARM64, or its installation is unavailable.
    Unsupported,
    /// The remote Session supports only generations newer than this Client's.
    ClientTooOld,
    /// The remote Session supports only older generations, or predates the handshake.
    SessionTooOld,
    /// Development builds from the same checkout path whose protocol fingerprints differ.
    DevelopmentBuildMismatch,
}

impl Refusal {
    /// The refusal an attempt's error carries, if any.
    pub fn of(error: &io::Error) -> Option<Self> {
        error.get_ref()?.downcast_ref::<Self>().copied()
    }

    /// The machine picker message that explains this refusal.
    pub fn key(self) -> &'static str {
        match self {
            Self::OtherEnvironment => "refused-environment",
            Self::Unsupported => "refused-unsupported",
            Self::ClientTooOld => "refused-client-old",
            Self::SessionTooOld => "refused-session-old",
            Self::DevelopmentBuildMismatch => "refused-build",
        }
    }
}

impl From<Refusal> for io::Error {
    /// Permission denied keeps the failure permanent, so discovery never retries it on its own.
    fn from(refusal: Refusal) -> Self {
        Self::new(io::ErrorKind::PermissionDenied, refusal)
    }
}

/// English diagnostics only; product copy comes from the picker and notice catalogs.
impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OtherEnvironment => "remote Orchbee belongs to another environment",
            Self::Unsupported => "remote platform or installation is unsupported",
            Self::ClientTooOld => "remote Session refused this Client as too old",
            Self::SessionTooOld => "remote Session is too old for this Client",
            Self::DevelopmentBuildMismatch => "remote development build differs",
        })
    }
}

impl Error for Refusal {}
