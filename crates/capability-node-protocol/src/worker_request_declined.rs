//! The Worker's one answer to a whole, well-formed Server request it does not serve: a kind its
//! platform has no branch for, a terminal request before any Launch, or a request it refuses before
//! any effect. Like JSON-RPC's "method not
//! found" or gRPC's `UNIMPLEMENTED`, it is generic, so a request kind that reaches no Worker branch
//! is still answered and never ends the Worker. It echoes the request's kind and identity, so the
//! Server completes exactly that request and knows the Worker did nothing for it. A whole request
//! frame the Worker cannot decode has no kind or identity to echo, so
//! [`CapabilityNodeMessage::WorkerRequestUnreadable`] answers it by position instead.

use std::num::NonZeroU64;

use crate::{CapabilityNodeMessage, OperationId};

/// The kind of Server request a decline answers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclinedRequestKind {
    Launch,
    Start,
    Viewer,
    Control,
    Lifecycle,
    CwdValidation,
    Send,
    WorktreeJob,
    AgentDrain,
    Shutdown,
    WorktreeMutation,
    Titles,
    Handoff,
    AgentIntegrationJob,
    AgentProbeJob,
    HostQuery,
    TitleChanges,
    PluginJob,
    GraphicsDecode,
    AdoptHost,
    MoveTurn,
    FilesProcess,
}

/// A decline: the request's kind and its identity, the request or operation ID it carries, or for
/// an Agent drain its token and acknowledged sequence, each big-endian.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerRequestDeclined {
    pub kind: DeclinedRequestKind,
    pub identity: [u8; 16],
}

impl WorkerRequestDeclined {
    /// The decline of the request of `kind` whose identity is `id`.
    pub const fn operation(kind: DeclinedRequestKind, id: OperationId) -> Self {
        Self {
            kind,
            identity: id.as_bytes(),
        }
    }

    /// The decline of the Agent drain with `token` that acknowledged `acknowledged`.
    pub fn agent_drain(token: NonZeroU64, acknowledged: u64) -> Self {
        let mut identity = [0; 16];
        identity[..8].copy_from_slice(&token.get().to_be_bytes());
        identity[8..].copy_from_slice(&acknowledged.to_be_bytes());
        Self {
            kind: DeclinedRequestKind::AgentDrain,
            identity,
        }
    }

    /// The decline that answers `message`, or `None` for a message only a Worker sends, which
    /// a Worker never answers.
    pub fn of(message: &CapabilityNodeMessage) -> Option<Self> {
        use CapabilityNodeMessage as Message;
        use DeclinedRequestKind as Kind;
        let (kind, id) = match message {
            Message::AuthorizedTerminalLaunch(request) => {
                (Kind::Launch, request.authorization().operation_id())
            }
            Message::AuthorizedTerminalStart(request) => {
                (Kind::Start, request.authorization().operation_id())
            }
            Message::TerminalViewerRequest(request) => (Kind::Viewer, request.request_id),
            Message::TerminalControlRequest(request) => (Kind::Control, request.request_id),
            Message::TerminalLifecycleRequest(request) => (Kind::Lifecycle, request.request_id),
            Message::TerminalCwdValidationRequest(request) => {
                (Kind::CwdValidation, request.operation_id)
            }
            Message::TerminalSendRequest(request) => (Kind::Send, request.request_id),
            Message::WorktreeJobRequest(request) => (Kind::WorktreeJob, request.operation_id),
            Message::AgentDrainRequest(request) => {
                return Some(Self::agent_drain(request.token, request.acknowledged));
            }
            Message::WorkerShutdownRequest(request) => (Kind::Shutdown, request.operation_id),
            Message::WorktreeMutationRequest(request) => {
                (Kind::WorktreeMutation, request.operation_id)
            }
            Message::TerminalTitlesRequest(request) => (Kind::Titles, request.request_id),
            Message::WorkerHandoffRequest(request) => (Kind::Handoff, request.operation_id),
            Message::AgentIntegrationJobRequest(request) => {
                (Kind::AgentIntegrationJob, request.operation_id)
            }
            Message::AgentProbeJobRequest(request) => (Kind::AgentProbeJob, request.operation_id),
            Message::HostQueryRequest(request) => (Kind::HostQuery, request.request_id),
            Message::TerminalTitleChangesRequest(request) => {
                (Kind::TitleChanges, request.request_id)
            }
            Message::PluginJobRequest(request) => (Kind::PluginJob, request.operation_id),
            Message::TerminalGraphicsDecodeRequest(request) => {
                (Kind::GraphicsDecode, request.request_id)
            }
            Message::AdoptHostRequest(request) => (Kind::AdoptHost, request.key().adoption_id),
            Message::TerminalMoveTurnRequest(request) => (Kind::MoveTurn, request.request_id),
            Message::FilesProcessRequest(request) => (Kind::FilesProcess, request.operation_id),
            Message::AdoptHostResult(_)
            | Message::FilesProcessResult(_)
            | Message::TerminalMoveTurnResult(_)
            | Message::WorkerHandoffResult(_)
            | Message::WorkerHandoffHello(_)
            | Message::WorkerHandoffReady(_)
            | Message::WorkerHandoffOwned(_)
            | Message::TerminalTitlesResult(_)
            | Message::TerminalTitlesChanged(_)
            | Message::TerminalTitleChangesResult(_)
            | Message::PluginJobResult(_)
            | Message::WorktreeMutationResult(_)
            | Message::WorkerShutdownResult(_)
            | Message::TerminalLifecycleResult(_)
            | Message::TerminalControlResult(_)
            | Message::TerminalCwdValidationResult(_)
            | Message::TerminalLaunchResult(_)
            | Message::TerminalStartResult(_)
            | Message::TerminalViewerResult(_)
            | Message::TerminalViewerRecordReady(_)
            | Message::TerminalSendResult(_)
            | Message::WorktreeJobResult(_)
            | Message::AgentDrainReply(_)
            | Message::AgentIntegrationJobResult(_)
            | Message::AgentProbeJobResult(_)
            | Message::HostQueryResult(_)
            | Message::WorkerRequestDeclined(_)
            | Message::WorkerRequestUnreadable => return None,
        };
        Some(Self::operation(kind, id))
    }
}
