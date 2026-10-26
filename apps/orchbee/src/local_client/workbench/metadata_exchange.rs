//! Shared metadata transport failures; individual request owners retain payload and retry policy.
/// Exact failed exchange stage; it does not independently establish whether an effect occurred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MetadataExchangeReason {
    ConnectionUnavailable,
    Encoding,
    Deadline,
    NotSubmitted,
    Write,
    Read,
    ResponseDeadline,
    Correlation,
    Protocol,
}
impl MetadataExchangeReason {
    /// Stable English diagnostic code for the failed exchange stage.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ConnectionUnavailable => "session-unavailable",
            Self::Encoding => "request-encoding",
            Self::Deadline => "request-deadline",
            Self::NotSubmitted => "request-not-submitted",
            Self::Write => "request-write",
            Self::Read => "response-read",
            Self::ResponseDeadline => "response-deadline",
            Self::Correlation => "response-correlation",
            Self::Protocol => "response-protocol",
        }
    }
}
/// Submission uncertainty and its failure stage, retained without fabricating a domain outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MetadataExchangeError {
    pub(crate) submitted: bool,
    pub(crate) reason: MetadataExchangeReason,
}
