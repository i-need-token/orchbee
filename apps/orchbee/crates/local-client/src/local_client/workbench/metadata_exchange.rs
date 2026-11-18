//! Shared metadata transport failures; individual request owners retain payload and retry policy.
/// Exact failed exchange stage; it does not independently establish whether an effect occurred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataExchangeReason {
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
    pub const fn as_str(self) -> &'static str {
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
pub struct MetadataExchangeError {
    pub submitted: bool,
    pub reason: MetadataExchangeReason,
}

impl crate::local_client::LocalClient {
    pub(super) fn finish_metadata_exchange<Value>(
        &mut self,
        attempt: crate::local_client::ExchangeAttempt<
            orchbee_workbench_protocol::WorkbenchResponse,
            orchbee_workbench_protocol::WorkbenchWireError,
        >,
        accept: impl FnOnce(orchbee_workbench_protocol::WorkbenchResponse) -> Option<Value>,
    ) -> Result<Value, MetadataExchangeError> {
        let keep_connection = Self::keep_connection_after_attempt(&attempt);
        let reason = match attempt {
            crate::local_client::ExchangeAttempt::Result(response) => match accept(response) {
                Some(value) => return Ok(value),
                None => MetadataExchangeReason::Correlation,
            },
            crate::local_client::ExchangeAttempt::NotSubmitted(_) => {
                return Err(MetadataExchangeError {
                    submitted: false,
                    reason: MetadataExchangeReason::NotSubmitted,
                });
            }
            crate::local_client::ExchangeAttempt::WriteFailed(_) => MetadataExchangeReason::Write,
            crate::local_client::ExchangeAttempt::ReadFailed(
                orchbee_workbench_protocol::WorkbenchWireError::Contract(_)
                | orchbee_workbench_protocol::WorkbenchWireError::InvalidHeader,
            ) => MetadataExchangeReason::Protocol,
            crate::local_client::ExchangeAttempt::ReadFailed(
                orchbee_workbench_protocol::WorkbenchWireError::Io(error),
            ) if error.kind() == std::io::ErrorKind::TimedOut => {
                MetadataExchangeReason::ResponseDeadline
            }
            crate::local_client::ExchangeAttempt::ReadFailed(_) => MetadataExchangeReason::Read,
            crate::local_client::ExchangeAttempt::ReadDeadline(_) => {
                MetadataExchangeReason::ResponseDeadline
            }
        };
        if !keep_connection {
            self.connection = None;
        }
        Err(MetadataExchangeError {
            submitted: true,
            reason,
        })
    }
}
