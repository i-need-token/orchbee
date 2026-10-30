//! Names why one terminal record was refused: the record family under validation, the stage that
//! refused it, and the static protocol cause when decoding failed. It never retains record bytes,
//! terminal text, identifiers, or environment data, and it does not choose the caller's external
//! failure classification.

use orchbee_terminal_protocol::ProtocolError;
use std::{error::Error, fmt};

/// The internal reason a [`TerminalProjection`](super::TerminalProjection) refused a record.
///
/// Callers keep their external classification (for example `protocol-invalid` or a protocol
/// carrier failure); this value preserves the operation, stage, and chained protocol cause for
/// diagnostics and tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionRejection {
    operation: ProjectionOperation,
    stage: ProjectionStage,
    cause: Option<ProtocolError>,
}

impl ProjectionRejection {
    pub(super) const fn new(operation: ProjectionOperation, stage: ProjectionStage) -> Self {
        Self {
            operation,
            stage,
            cause: None,
        }
    }

    /// Builds the adapter for one decoder call, keeping its static protocol error as the cause.
    pub(super) fn decoding(
        operation: ProjectionOperation,
        stage: ProjectionStage,
    ) -> impl FnOnce(ProtocolError) -> Self {
        move |cause| Self {
            operation,
            stage,
            cause: Some(cause),
        }
    }

    #[must_use]
    pub const fn operation(self) -> ProjectionOperation {
        self.operation
    }

    #[must_use]
    pub const fn stage(self) -> ProjectionStage {
        self.stage
    }

    /// The decoder's static protocol rejection, present only for decode stages.
    #[must_use]
    pub const fn cause(self) -> Option<ProtocolError> {
        self.cause
    }
}

impl fmt::Display for ProjectionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "terminal projection refused {} record at {}",
            self.operation.as_str(),
            self.stage.as_str()
        )
    }
}

impl Error for ProjectionRejection {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_ref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}

/// The record family a projection was validating when it refused the record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionOperation {
    /// The shared transport envelope, before the record family is known.
    Record,
    /// A kind 15 or 19 terminal lifecycle event.
    Lifecycle,
    /// A kind 49 projection-blocked control.
    ProjectionBlocked,
    /// A kind 53 history-changed control.
    HistoryChanged,
    /// A kind 41 snapshot or kind 42 delta terminal frame.
    Frame,
}

impl ProjectionOperation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Lifecycle => "lifecycle",
            Self::ProjectionBlocked => "projection-blocked",
            Self::HistoryChanged => "history-changed",
            Self::Frame => "frame",
        }
    }
}

/// The validation stage that refused the record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionStage {
    /// The transport envelope could not be decoded.
    EnvelopeDecode,
    /// The envelope version, role, correlation, epoch, optionality, or encoding does not route to
    /// this projection.
    EnvelopeRoute,
    /// The record kind or payload encoding is not one this projection admits.
    UnexpectedKind,
    /// The control or frame payload could not be decoded.
    PayloadDecode,
    /// The decoded control is a different message than this exchange admits.
    UnexpectedMessage,
    /// The record names another attachment, Host epoch, or terminal.
    Identity,
    /// A lifecycle end contradicts the end already recorded.
    ConflictingEnd,
    /// A projection-blocked revision does not advance.
    StaleRevision,
    /// A frame sequence does not advance, or its snapshot or delta shape contradicts its kind.
    FrameRoute,
    /// The bound attachment identity cannot form viewer feedback.
    FeedbackIdentity,
}

impl ProjectionStage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnvelopeDecode => "envelope-decode",
            Self::EnvelopeRoute => "envelope-route",
            Self::UnexpectedKind => "unexpected-kind",
            Self::PayloadDecode => "payload-decode",
            Self::UnexpectedMessage => "unexpected-message",
            Self::Identity => "identity",
            Self::ConflictingEnd => "conflicting-end",
            Self::StaleRevision => "stale-revision",
            Self::FrameRoute => "frame-route",
            Self::FeedbackIdentity => "feedback-identity",
        }
    }
}
