//! States how one Worker wire ended as daily-log tokens: the exit, the failure of the request in
//! flight when that request ended the wire, and whether the Worker had answered a shutdown first.
//! It also records a request that failed while the wire kept serving. It observes the carrier's
//! own outcomes and changes none of them.

use std::time::Duration;

use orchbee_capability_node_protocol::ProtocolError;

use super::{
    FramingBreak, WorkerOperationRequest, WorkerRequestError, WorkerResponseFailure,
    WorkerTransportFailure, WorkerWireExit,
};
use crate::fault_records::io_token;

/// The last request the wire completed in the current turn: its kind, its failure, and whether its
/// requester received the completion.
#[derive(Clone, Copy, Debug)]
pub(super) struct EndedRequest {
    pub(super) kind: &'static str,
    pub(super) failure: Option<WorkerRequestError>,
    pub(super) delivered: bool,
    pub(super) elapsed: Duration,
}

impl EndedRequest {
    /// Records this request's failure when the wire kept serving after it.
    pub(super) fn record_contained(&self) {
        if let Some(error) = self.failure {
            crate::fault_records::worker_request_failed(
                self.kind,
                request_failure(error),
                request_error(error),
                self.elapsed,
            );
        }
    }
}

/// How one Worker wire ended.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkerWireEnd {
    pub(crate) exit: WorkerWireExit,
    /// The request completed in the turn that ended the wire; `None` when it ended between
    /// requests.
    pub(super) request: Option<EndedRequest>,
    pub(super) after_shutdown: bool,
}

impl WorkerWireEnd {
    pub(crate) const fn exit_token(&self) -> &'static str {
        match self.exit {
            WorkerWireExit::Shutdown => "shutdown",
            WorkerWireExit::PeerClosed => "peer-closed",
            WorkerWireExit::BrokenFraming(_) => "broken-framing",
            WorkerWireExit::Transport(_) => "transport",
            WorkerWireExit::RequestFailed => "request-failed",
        }
    }

    /// The failure that ended the wire, `none` for an ordinary end.
    pub(crate) const fn failure_token(&self) -> &'static str {
        if let Some(request) = self.request {
            if let Some(error) = request.failure {
                return request_failure(error);
            }
            if !request.delivered {
                return "completion-undeliverable";
            }
        }
        match self.exit {
            WorkerWireExit::Transport(failure) => transport_failure(failure),
            WorkerWireExit::BrokenFraming(FramingBreak::Header(_)) => "broken-framing.header",
            WorkerWireExit::BrokenFraming(FramingBreak::Incomplete) => "broken-framing.incomplete",
            WorkerWireExit::Shutdown
            | WorkerWireExit::PeerClosed
            | WorkerWireExit::RequestFailed => "none",
        }
    }

    /// The failure's I/O error kind or protocol error, `none` without one.
    pub(crate) fn error_token(&self) -> &'static str {
        if let Some(EndedRequest {
            failure: Some(error),
            ..
        }) = self.request
        {
            return request_error(error);
        }
        match self.exit {
            WorkerWireExit::Transport(WorkerTransportFailure::Io(kind)) => io_token(kind),
            WorkerWireExit::BrokenFraming(FramingBreak::Header(error)) => protocol_error(error),
            _ => "none",
        }
    }

    /// The kind of request that ended the wire and how long it had run since dispatch.
    pub(crate) fn request(&self) -> Option<(&'static str, Duration)> {
        self.request.map(|request| (request.kind, request.elapsed))
    }

    pub(crate) const fn after_shutdown(&self) -> bool {
        self.after_shutdown
    }
}

impl WorkerOperationRequest {
    /// The request's kind as a daily-log token.
    pub(super) const fn kind_token(&self) -> &'static str {
        match self {
            #[cfg(any(windows, test))]
            Self::Handoff(_) => "handoff",
            Self::Shutdown(_) => "shutdown",
            Self::CwdValidation(_) => "cwd-validation",
            Self::Lifecycle(_) => "lifecycle",
            Self::Control(_) => "control",
            Self::Launch(_) => "launch",
            Self::Start(_) => "start",
            Self::Viewer(_) => "viewer",
            Self::GraphicsDecode(_) => "graphics-decode",
            Self::Send(_) => "send",
            Self::WorktreeJob(_) => "worktree-job",
            Self::PluginJob(_) => "plugin-job",
            Self::AgentIntegrationJob(_) => "agent-integration-job",
            Self::AgentProbeJob(_) => "agent-probe-job",
            Self::FilesProcess(_) => "files-process",
            Self::WorktreeMutation(_) => "worktree-mutation",
            Self::AgentDrain(_) => "agent-drain",
            Self::Titles(_) => "titles",
            Self::TitleChanges(_) => "title-changes",
            Self::HostQuery(_) => "host-query",
            Self::AdoptHost(_) => "adopt-host",
            Self::MoveTurn(_) => "move-turn",
        }
    }
}

const fn request_failure(error: WorkerRequestError) -> &'static str {
    match error {
        WorkerRequestError::Unavailable => "unavailable",
        WorkerRequestError::Backpressure => "backpressure",
        WorkerRequestError::WrongWorker => "wrong-worker",
        WorkerRequestError::WrongGeneration => "wrong-generation",
        WorkerRequestError::InvalidRequest(_) => "invalid-request",
        WorkerRequestError::Declined => "declined",
        WorkerRequestError::NotDelivered(failure) => match failure {
            WorkerTransportFailure::TimedOut => "not-delivered.timed-out",
            WorkerTransportFailure::Closed => "not-delivered.closed",
            WorkerTransportFailure::UnexpectedPeerData => "not-delivered.unexpected-peer-data",
            WorkerTransportFailure::Io(_) => "not-delivered.io",
        },
        WorkerRequestError::PostDeliveryOutcomeUnknown(failure) => match failure {
            WorkerResponseFailure::TimedOut => "outcome-unknown.timed-out",
            WorkerResponseFailure::Disconnected => "outcome-unknown.disconnected",
            WorkerResponseFailure::CarrierStopped => "outcome-unknown.carrier-stopped",
            WorkerResponseFailure::Io(_) => "outcome-unknown.io",
            WorkerResponseFailure::InvalidFrame(_) => "outcome-unknown.invalid-frame",
            WorkerResponseFailure::WrongDirection => "outcome-unknown.wrong-direction",
            WorkerResponseFailure::WrongResultKind => "outcome-unknown.wrong-result-kind",
            WorkerResponseFailure::MismatchedCorrelation => {
                "outcome-unknown.mismatched-correlation"
            }
            WorkerResponseFailure::MismatchedAuthorization => {
                "outcome-unknown.mismatched-authorization"
            }
            WorkerResponseFailure::MismatchedTarget => "outcome-unknown.mismatched-target",
        },
    }
}

/// A request failure's I/O error kind or protocol error, `none` without one.
fn request_error(error: WorkerRequestError) -> &'static str {
    match error {
        WorkerRequestError::InvalidRequest(error)
        | WorkerRequestError::PostDeliveryOutcomeUnknown(WorkerResponseFailure::InvalidFrame(
            error,
        )) => protocol_error(error),
        WorkerRequestError::NotDelivered(WorkerTransportFailure::Io(kind))
        | WorkerRequestError::PostDeliveryOutcomeUnknown(WorkerResponseFailure::Io(kind)) => {
            io_token(kind)
        }
        _ => "none",
    }
}

const fn transport_failure(failure: WorkerTransportFailure) -> &'static str {
    match failure {
        WorkerTransportFailure::TimedOut => "transport.timed-out",
        WorkerTransportFailure::Closed => "transport.closed",
        WorkerTransportFailure::UnexpectedPeerData => "transport.unexpected-peer-data",
        WorkerTransportFailure::Io(_) => "transport.io",
    }
}

pub(super) const fn protocol_error(error: ProtocolError) -> &'static str {
    match error {
        ProtocolError::InvalidMagic => "invalid-magic",
        ProtocolError::IncompatibleVersion { .. } => "incompatible-version",
        ProtocolError::PayloadTooLarge { .. } => "payload-too-large",
        ProtocolError::InvalidPayload => "invalid-payload",
        ProtocolError::InvalidIdentity => "invalid-identity",
        ProtocolError::InvalidCwd => "invalid-cwd",
        ProtocolError::InvalidGeometry => "invalid-geometry",
        ProtocolError::ContradictoryHostObservation => "contradictory-host-observation",
    }
}
