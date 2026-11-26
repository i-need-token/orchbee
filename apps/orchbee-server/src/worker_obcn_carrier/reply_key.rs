//! Names the identity a Worker reply must echo to answer one request: the correlation that
//! response validation checks first. A failed request keeps only this key, so the wire can drop a
//! late reply to it without holding the request's payload or mistaking it for another result.

use std::num::NonZeroU64;

use orchbee_capability_node_protocol::{
    AdoptHostPageKey, CapabilityNodeMessage, DeclinedRequestKind, LifecycleStepOrdinal,
    OperationId, WorkerRequestDeclined,
};

use super::WorkerOperationRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReplyKey {
    #[cfg(any(windows, test))]
    Handoff(OperationId),
    Shutdown(OperationId),
    CwdValidation(OperationId),
    HostQuery(OperationId),
    Lifecycle {
        request_id: OperationId,
        close_operation_id: OperationId,
        attempt_generation: NonZeroU64,
        step_ordinal: LifecycleStepOrdinal,
    },
    Control(OperationId),
    Viewer(OperationId),
    GraphicsDecode {
        request_id: OperationId,
        attachment_id: OperationId,
    },
    Launch(OperationId),
    Start(OperationId),
    Send(OperationId),
    Titles(OperationId),
    TitleChanges(OperationId),
    AgentDrain {
        token: NonZeroU64,
        acknowledged: u64,
    },
    PluginJob(OperationId),
    WorktreeMutation(OperationId),
    WorktreeJob(OperationId),
    AgentIntegrationJob(OperationId),
    AgentProbeJob(OperationId),
    AdoptHost(AdoptHostPageKey),
    MoveTurn(OperationId),
}

impl ReplyKey {
    pub(super) fn of(request: &WorkerOperationRequest) -> Self {
        match request {
            #[cfg(any(windows, test))]
            WorkerOperationRequest::Handoff(request) => Self::Handoff(request.operation_id),
            WorkerOperationRequest::Shutdown(request) => Self::Shutdown(request.operation_id),
            WorkerOperationRequest::CwdValidation(request) => {
                Self::CwdValidation(request.operation_id)
            }
            WorkerOperationRequest::HostQuery(request) => Self::HostQuery(request.request_id),
            WorkerOperationRequest::Lifecycle(request) => Self::Lifecycle {
                request_id: request.request_id,
                close_operation_id: request.close_operation_id,
                attempt_generation: request.attempt_generation,
                step_ordinal: request.step_ordinal,
            },
            WorkerOperationRequest::Control(request) => Self::Control(request.request_id),
            WorkerOperationRequest::Viewer(request) => Self::Viewer(request.request_id),
            WorkerOperationRequest::GraphicsDecode(request) => Self::GraphicsDecode {
                request_id: request.request_id,
                attachment_id: request.attachment_id,
            },
            WorkerOperationRequest::Launch(request) => {
                Self::Launch(request.authorization().operation_id())
            }
            WorkerOperationRequest::Start(request) => {
                Self::Start(request.authorization().operation_id())
            }
            WorkerOperationRequest::Send(request) => Self::Send(request.request_id),
            WorkerOperationRequest::Titles(request) => Self::Titles(request.request_id),
            WorkerOperationRequest::TitleChanges(request) => Self::TitleChanges(request.request_id),
            WorkerOperationRequest::AgentDrain(request) => Self::AgentDrain {
                token: request.token,
                acknowledged: request.acknowledged,
            },
            WorkerOperationRequest::PluginJob(request) => Self::PluginJob(request.operation_id),
            WorkerOperationRequest::WorktreeMutation(request) => {
                Self::WorktreeMutation(request.operation_id)
            }
            WorkerOperationRequest::WorktreeJob(request) => Self::WorktreeJob(request.operation_id),
            WorkerOperationRequest::AgentIntegrationJob(request) => {
                Self::AgentIntegrationJob(request.operation_id)
            }
            WorkerOperationRequest::AgentProbeJob(request) => {
                Self::AgentProbeJob(request.operation_id)
            }
            WorkerOperationRequest::AdoptHost(request) => Self::AdoptHost(request.key()),
            WorkerOperationRequest::MoveTurn(request) => Self::MoveTurn(request.request_id),
        }
    }

    /// Whether `message` is a result of this request's kind that echoes this request's
    /// correlation, whatever else it carries.
    pub(super) fn answered_by(self, message: &CapabilityNodeMessage) -> bool {
        use CapabilityNodeMessage as Message;
        match (self, message) {
            #[cfg(any(windows, test))]
            (Self::Handoff(id), Message::WorkerHandoffResult(result)) => {
                result.request().operation_id == id
            }
            (Self::Shutdown(id), Message::WorkerShutdownResult(result)) => {
                result.request.operation_id == id
            }
            (Self::CwdValidation(id), Message::TerminalCwdValidationResult(result)) => {
                result.request.operation_id == id
            }
            (Self::HostQuery(id), Message::HostQueryResult(result)) => {
                result.request.request_id == id
            }
            (
                Self::Lifecycle {
                    request_id,
                    close_operation_id,
                    attempt_generation,
                    step_ordinal,
                },
                Message::TerminalLifecycleResult(result),
            ) => {
                let echo = result.request();
                echo.request_id == request_id
                    && echo.close_operation_id == close_operation_id
                    && echo.attempt_generation == attempt_generation
                    && echo.step_ordinal == step_ordinal
            }
            (Self::Control(id), Message::TerminalControlResult(result)) => result.request_id == id,
            (Self::Viewer(id), Message::TerminalViewerResult(result)) => result.request_id == id,
            (
                Self::GraphicsDecode {
                    request_id,
                    attachment_id,
                },
                Message::TerminalViewerResult(result),
            ) => result.request_id == request_id && result.attachment_id == attachment_id,
            (Self::Launch(id), Message::TerminalLaunchResult(result)) => {
                result.authorization().operation_id() == id
            }
            (Self::Start(id), Message::TerminalStartResult(result)) => {
                result.authorization().operation_id() == id
            }
            (Self::Send(id), Message::TerminalSendResult(result)) => result.request_id() == id,
            (Self::Titles(id), Message::TerminalTitlesResult(result)) => result.request_id == id,
            (Self::TitleChanges(id), Message::TerminalTitleChangesResult(result)) => {
                result.request.request_id == id
            }
            (
                Self::AgentDrain {
                    token,
                    acknowledged,
                },
                Message::AgentDrainReply(reply),
            ) => reply.token_echo() == token && reply.acknowledged_echo() == acknowledged,
            (Self::PluginJob(id), Message::PluginJobResult(result)) => result.operation_id == id,
            (Self::WorktreeMutation(id), Message::WorktreeMutationResult(result)) => {
                result.operation_id == id
            }
            (Self::WorktreeJob(id), Message::WorktreeJobResult(result)) => {
                result.operation_id == id
            }
            (Self::AgentIntegrationJob(id), Message::AgentIntegrationJobResult(result)) => {
                result.operation_id == id
            }
            (Self::AgentProbeJob(id), Message::AgentProbeJobResult(result)) => {
                result.operation_id == id
            }
            (Self::AdoptHost(key), Message::AdoptHostResult(result)) => result.key() == key,
            (Self::MoveTurn(id), Message::TerminalMoveTurnResult(result)) => {
                result.request.request_id == id
            }
            (key, Message::WorkerRequestDeclined(declined)) => key.declined() == *declined,
            _ => false,
        }
    }

    /// The decline a Worker that does not serve this request answers with: its kind and the
    /// identity [`WorkerRequestDeclined::of`] reads from the request itself.
    fn declined(self) -> WorkerRequestDeclined {
        use DeclinedRequestKind as Kind;
        let (kind, id) = match self {
            #[cfg(any(windows, test))]
            Self::Handoff(id) => (Kind::Handoff, id),
            Self::Shutdown(id) => (Kind::Shutdown, id),
            Self::CwdValidation(id) => (Kind::CwdValidation, id),
            Self::HostQuery(id) => (Kind::HostQuery, id),
            Self::Lifecycle { request_id, .. } => (Kind::Lifecycle, request_id),
            Self::Control(id) => (Kind::Control, id),
            Self::Viewer(id) => (Kind::Viewer, id),
            Self::GraphicsDecode { request_id, .. } => (Kind::GraphicsDecode, request_id),
            Self::Launch(id) => (Kind::Launch, id),
            Self::Start(id) => (Kind::Start, id),
            Self::Send(id) => (Kind::Send, id),
            Self::Titles(id) => (Kind::Titles, id),
            Self::TitleChanges(id) => (Kind::TitleChanges, id),
            Self::AgentDrain {
                token,
                acknowledged,
            } => return WorkerRequestDeclined::agent_drain(token, acknowledged),
            Self::PluginJob(id) => (Kind::PluginJob, id),
            Self::WorktreeMutation(id) => (Kind::WorktreeMutation, id),
            Self::WorktreeJob(id) => (Kind::WorktreeJob, id),
            Self::AgentIntegrationJob(id) => (Kind::AgentIntegrationJob, id),
            Self::AgentProbeJob(id) => (Kind::AgentProbeJob, id),
            Self::AdoptHost(key) => (Kind::AdoptHost, key.adoption_id),
            Self::MoveTurn(id) => (Kind::MoveTurn, id),
        };
        WorkerRequestDeclined::operation(kind, id)
    }
}
