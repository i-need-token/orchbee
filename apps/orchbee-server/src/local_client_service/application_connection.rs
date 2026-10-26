//! Dispatches admitted application records to typed terminal, Agent, and Workbench service
//! handlers and encodes their correlated outcomes. It owns protocol-family dispatch without
//! authenticating the connection or deciding domain authorization itself.

use std::{
    fmt,
    io::{self, Cursor, Read, Write},
};

use orchbee_agent_observation_protocol::{
    AGENT_MAGIC, AgentRequest, AgentResult, AgentWireError, read_agent_request, write_agent_result,
};
use orchbee_client_server_protocol::{
    ClientMessage, FRAME_MAGIC, PASTE_RECORD_MAGIC, ReadMessageError, SEND_RECORD_MAGIC,
    ServerMessage, WriteMessageError, read_client_message, write_server_message,
};
use orchbee_workbench_protocol::{
    MultiplexLaneId, WORKBENCH_MAGIC, WorkbenchRequest, WorkbenchWireError, read_workbench_request,
    write_workbench_response,
};

use crate::agent_service::AgentReadFault;
use crate::workbench_service::WorkbenchExecution;

#[derive(Clone)]
pub(super) struct ApplicationRequest {
    kind: ApplicationRequestKind,
    encoded_length: usize,
}
#[derive(Clone)]
enum ApplicationRequestKind {
    Terminal(ClientMessage),
    Workbench(WorkbenchRequest),
    Agent(AgentRequest),
}
impl ApplicationRequest {
    pub(super) fn workbench(&self) -> Option<&WorkbenchRequest> {
        match &self.kind {
            ApplicationRequestKind::Workbench(request) => Some(request),
            _ => None,
        }
    }
    pub(super) fn same_subscription_owner(&self, other: &Self) -> bool {
        match (self.workbench(), other.workbench()) {
            (Some(WorkbenchRequest::WindowTitle(_)), Some(WorkbenchRequest::WindowTitle(_)))
            | (Some(WorkbenchRequest::Notification(_)), Some(WorkbenchRequest::Notification(_))) => {
                true
            }
            _ => false,
        }
    }
    pub(super) fn shares_event_cursor(&self, other: &Self) -> bool {
        matches!((self.workbench(), other.workbench()),
            (Some(WorkbenchRequest::Events(left)), Some(WorkbenchRequest::Events(right)))
                if left.action.subscription_id() == right.action.subscription_id())
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, ApplicationConnectionError> {
        let prefix = bytes
            .get(..4)
            .ok_or(ApplicationConnectionError::UnknownProtocol)?;
        let mut reader = Cursor::new(bytes);
        let kind =
            if prefix == FRAME_MAGIC || prefix == PASTE_RECORD_MAGIC || prefix == SEND_RECORD_MAGIC
            {
                ApplicationRequestKind::Terminal(
                    read_client_message(&mut reader).map_err(ApplicationConnectionError::Read)?,
                )
            } else if prefix == WORKBENCH_MAGIC {
                ApplicationRequestKind::Workbench(
                    read_workbench_request(&mut reader)
                        .map_err(ApplicationConnectionError::Workbench)?,
                )
            } else if prefix == AGENT_MAGIC {
                ApplicationRequestKind::Agent(
                    read_agent_request(&mut reader).map_err(ApplicationConnectionError::Agent)?,
                )
            } else {
                return Err(ApplicationConnectionError::UnknownProtocol);
            };
        if reader.position() != bytes.len() as u64 {
            return Err(ApplicationConnectionError::UnknownProtocol);
        }
        Ok(Self {
            kind,
            encoded_length: bytes.len(),
        })
    }
    /// Waiting belongs to a subscription, never the sole connection dispatcher.
    pub(super) fn multiplex_safe(&self) -> bool {
        !matches!(self.workbench(), Some(WorkbenchRequest::Events(request)) if matches!(request.action, orchbee_workbench_protocol::SemanticEventsAction::Wait { .. }))
    }
    pub(super) fn continuation(&self) -> Self {
        let mut next = self.clone();
        if let ApplicationRequestKind::Workbench(WorkbenchRequest::Events(request)) = &mut next.kind
            && let orchbee_workbench_protocol::SemanticEventsAction::Subscribe {
                subscription_id,
                ..
            } = &request.action
        {
            request.action = orchbee_workbench_protocol::SemanticEventsAction::Read {
                subscription_id: subscription_id.clone(),
                limit: orchbee_workbench_protocol::MAXIMUM_EVENT_PAGE as u16,
            };
        }
        next
    }
    pub(super) fn fresh_snapshot(&self) -> Self {
        let mut next = self.clone();
        if let ApplicationRequestKind::Workbench(WorkbenchRequest::Events(request)) = &mut next.kind
            && let orchbee_workbench_protocol::SemanticEventsAction::Subscribe { after, .. } =
                &mut request.action
        {
            *after = None;
        }
        next
    }
    pub(super) fn has_event(record: &[u8]) -> bool {
        use orchbee_workbench_protocol::{
            NotificationOutcome, SemanticEventsOutcome, WorkbenchResponse, read_workbench_response,
        };
        match read_workbench_response(&mut Cursor::new(record)) {
            Ok(WorkbenchResponse::Notification(reply)) => matches!(
                reply.outcome,
                NotificationOutcome::Listening {
                    notification: Some(_),
                    ..
                }
            ),
            Ok(WorkbenchResponse::Events(reply)) => {
                matches!(reply.outcome, SemanticEventsOutcome::Events { events, .. } if !events.is_empty())
            }
            _ => false,
        }
    }
    pub(super) fn event_page(record: &[u8]) -> (bool, bool) {
        use orchbee_workbench_protocol::{
            SemanticEventsOutcome, WorkbenchResponse, read_workbench_response,
        };
        match read_workbench_response(&mut Cursor::new(record)) {
            Ok(WorkbenchResponse::Events(reply)) => match reply.outcome {
                SemanticEventsOutcome::Opened { .. } => (true, false),
                SemanticEventsOutcome::Events { more, gap, .. } => (more, gap.is_some()),
                SemanticEventsOutcome::Gap { .. } => (false, true),
                _ => (false, false),
            },
            _ => (false, false),
        }
    }
    pub(super) fn encoded_length(&self) -> usize {
        self.encoded_length
    }
    pub(super) fn subscribable(&self) -> bool {
        match &self.kind {
            // Synchronize may transfer a Queued control to Delivered even without a completion.
            // It stays a one-shot request; consumers subscribe to side-effect-free invalidations.
            ApplicationRequestKind::Workbench(WorkbenchRequest::ClientView(request))
                if matches!(
                    request.action,
                    orchbee_workbench_protocol::ClientViewAction::Synchronize { .. }
                ) =>
            {
                false
            }
            ApplicationRequestKind::Workbench(WorkbenchRequest::Events(request)) => matches!(
                request.action,
                orchbee_workbench_protocol::SemanticEventsAction::Subscribe { .. }
            ),
            ApplicationRequestKind::Workbench(WorkbenchRequest::PaneGraphics(request))
                if matches!(
                    request.action,
                    orchbee_workbench_protocol::PaneGraphicsAction::Observe { .. }
                ) =>
            {
                false
            }
            ApplicationRequestKind::Workbench(request) => {
                crate::workbench_service::subscription_request(request)
            }
            ApplicationRequestKind::Agent(request) => {
                matches!(request, AgentRequest::Read(_))
            }
            ApplicationRequestKind::Terminal(ClientMessage::TerminalViewer(request)) => matches!(
                request.action(),
                orchbee_client_server_protocol::TerminalViewerAction::Poll { .. }
                    | orchbee_client_server_protocol::TerminalViewerAction::ReadProcessInfo { .. }
            ),
            ApplicationRequestKind::Terminal(_) => false,
        }
    }
    pub(super) fn requires_sampling(&self) -> bool {
        // Host output and external filesystem/process state currently expose snapshot reads,
        // not change notifications. Semantic state subscriptions have no periodic deadline.
        if let Some(WorkbenchRequest::Events(request)) = self.workbench()
            && let orchbee_workbench_protocol::SemanticEventsAction::Subscribe { filters, .. } =
                &request.action
        {
            use orchbee_workbench_protocol::{
                SemanticEventFilter as Filter, SemanticEventKind as Kind,
            };
            return filters.iter().any(|filter| !matches!(filter, Filter::Event { event, .. } if !matches!(event, Kind::PaneOutputChanged | Kind::PaneExited | Kind::PaneAgentDetected | Kind::PaneAgentStatusChanged)));
        }
        matches!(
            &self.kind,
            ApplicationRequestKind::Terminal(_)
                | ApplicationRequestKind::Workbench(
                    WorkbenchRequest::WorkspaceGit(_)
                        | WorkbenchRequest::WorktreeQuery(_)
                        | WorkbenchRequest::WorktreeListRead(_)
                        | WorkbenchRequest::Plugin(_)
                        | WorkbenchRequest::TerminalEditor(_)
                )
        )
    }
}

pub(super) fn run_network<Permit>(
    stream: &orchbee_local_ipc::LocalStream,
    mut execute: impl FnMut(MultiplexLaneId, ClientMessage) -> ServerMessage,
    mut execute_workbench: impl FnMut(MultiplexLaneId, WorkbenchRequest) -> WorkbenchExecution,
    mut execute_agent: impl FnMut(AgentRequest) -> Result<AgentResult, AgentReadFault>,
    mut admit: impl FnMut() -> Option<Permit>,
    watch: impl FnOnce(&std::sync::Arc<orchbee_server::SessionChangeSignal>),
    mut cancel: impl FnMut(&WorkbenchRequest),
    close_lane: impl FnMut(MultiplexLaneId),
    maintenance: impl FnMut() -> io::Result<Option<std::time::Instant>>,
) -> Result<(), ApplicationConnectionError> {
    let mut records = super::record_stream::ClientRecordStream::new(stream)
        .map_err(ApplicationConnectionError::Prefix)?;
    let mut header = [0; 12];
    records
        .read_exact(&mut header)
        .map_err(ApplicationConnectionError::Prefix)?;
    let mut prefixed = super::multiplex::PrefixedStream {
        prefix: Cursor::new(header.to_vec()),
        stream: &mut records,
    };
    if header[..4] != WORKBENCH_MAGIC
        || !matches!(
            u16::from_be_bytes([header[6], header[7]]),
            orchbee_workbench_protocol::MULTIPLEX_OPEN_KIND | 70
        )
    {
        return run_guarded(
            &mut prefixed,
            |request| execute(MultiplexLaneId::ROOT, request),
            |request| execute_workbench(MultiplexLaneId::ROOT, request),
            execute_agent,
            admit,
        );
    }
    let first = orchbee_workbench_protocol::read_multiplex_message(&mut prefixed)
        .map_err(ApplicationConnectionError::Prefix)?;
    let signal = std::sync::Arc::new(orchbee_server::SessionChangeSignal::default());
    watch(&signal);
    super::multiplex::run(
        stream,
        first,
        signal,
        |lane, request| {
            let _permit = admit().ok_or(ApplicationConnectionError::Stopping)?;
            let mut bytes = Vec::new();
            match &request.kind {
                ApplicationRequestKind::Terminal(request) => {
                    write_server_message(&mut bytes, &execute(lane, request.clone()))
                        .map_err(ApplicationConnectionError::Write)?
                }
                ApplicationRequestKind::Workbench(request) => {
                    let WorkbenchExecution::Response(response) =
                        execute_workbench(lane, request.clone())
                    else {
                        return Err(ApplicationConnectionError::WorkbenchOutcomeUnknown);
                    };
                    write_workbench_response(&mut bytes, &response)
                        .map_err(ApplicationConnectionError::Workbench)?;
                }
                ApplicationRequestKind::Agent(request) => {
                    let response = execute_agent(request.clone())
                        .map_err(ApplicationConnectionError::AgentFault)?;
                    write_agent_result(&mut bytes, &response)
                        .map_err(ApplicationConnectionError::Agent)?;
                }
            }
            Ok(bytes)
        },
        close_lane,
        |request| {
            if let Some(request) = request.workbench() {
                cancel(request);
            }
        },
        maintenance,
    )
    .map_err(ApplicationConnectionError::Prefix)
}

#[cfg(test)]
pub(super) fn run<Stream>(
    stream: &mut Stream,
    execute: impl FnMut(ClientMessage) -> ServerMessage,
    execute_workbench: impl FnMut(WorkbenchRequest) -> WorkbenchExecution,
    execute_agent: impl FnMut(AgentRequest) -> Result<AgentResult, AgentReadFault>,
) -> Result<(), ApplicationConnectionError>
where
    Stream: Read + Write,
{
    run_guarded(stream, execute, execute_workbench, execute_agent, || {
        Some(())
    })
}

pub(super) fn run_guarded<Stream, Permit>(
    stream: &mut Stream,
    mut execute: impl FnMut(ClientMessage) -> ServerMessage,
    mut execute_workbench: impl FnMut(WorkbenchRequest) -> WorkbenchExecution,
    mut execute_agent: impl FnMut(AgentRequest) -> Result<AgentResult, AgentReadFault>,
    mut admit: impl FnMut() -> Option<Permit>,
) -> Result<(), ApplicationConnectionError>
where
    Stream: Read + Write,
{
    loop {
        let mut prefix = [0; 4];
        stream
            .read_exact(&mut prefix)
            .map_err(ApplicationConnectionError::Prefix)?;
        match prefix {
            FRAME_MAGIC | PASTE_RECORD_MAGIC | SEND_RECORD_MAGIC => {
                let request = read_client_message(&mut Cursor::new(prefix).chain(&mut *stream))
                    .map_err(ApplicationConnectionError::Read)?;
                let _permit = admit().ok_or(ApplicationConnectionError::Stopping)?;
                let result = execute(request);
                write_server_message(stream, &result).map_err(ApplicationConnectionError::Write)?;
            }
            WORKBENCH_MAGIC => {
                let request = read_workbench_request(&mut Cursor::new(prefix).chain(&mut *stream))
                    .map_err(ApplicationConnectionError::Workbench)?;
                let _permit = admit().ok_or(ApplicationConnectionError::Stopping)?;
                let WorkbenchExecution::Response(result) = execute_workbench(request) else {
                    return Err(ApplicationConnectionError::WorkbenchOutcomeUnknown);
                };
                write_workbench_response(stream, &result)
                    .map_err(ApplicationConnectionError::Workbench)?;
            }
            AGENT_MAGIC => {
                let request = read_agent_request(&mut Cursor::new(prefix).chain(&mut *stream))
                    .map_err(ApplicationConnectionError::Agent)?;
                let _permit = admit().ok_or(ApplicationConnectionError::Stopping)?;
                let result =
                    execute_agent(request).map_err(ApplicationConnectionError::AgentFault)?;
                write_agent_result(stream, &result).map_err(ApplicationConnectionError::Agent)?;
            }
            _ => return Err(ApplicationConnectionError::UnknownProtocol),
        }
    }
}

#[derive(Debug)]
pub(super) enum ApplicationConnectionError {
    Stopping,
    Prefix(io::Error),
    UnknownProtocol,
    WorkbenchOutcomeUnknown,
    Workbench(WorkbenchWireError),
    Agent(AgentWireError),
    AgentFault(AgentReadFault),
    Read(ReadMessageError),
    Write(WriteMessageError),
}

impl fmt::Display for ApplicationConnectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopping => formatter.write_str("Session is stopping"),
            Self::Prefix(error) => {
                write!(formatter, "Application record prefix read failed: {error}")
            }
            Self::UnknownProtocol => formatter.write_str("Unknown application protocol"),
            Self::WorkbenchOutcomeUnknown => {
                formatter.write_str("Workbench command outcome is unknown")
            }
            Self::Workbench(error) => error.fmt(formatter),
            Self::Agent(error) => write!(formatter, "OBAP record exchange failed: {error}"),
            Self::AgentFault(error) => {
                write!(formatter, "Agent read could not be answered: {error}")
            }
            Self::Read(error) => write!(formatter, "OBCS request read failed: {error}"),
            Self::Write(error) => write!(formatter, "OBCS result write failed: {error}"),
        }
    }
}

impl std::error::Error for ApplicationConnectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stopping => None,
            Self::Prefix(error) => Some(error),
            Self::UnknownProtocol | Self::WorkbenchOutcomeUnknown => None,
            Self::Workbench(error) => Some(error),
            Self::Agent(error) => Some(error),
            Self::AgentFault(error) => Some(error),
            Self::Read(error) => Some(error),
            Self::Write(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests;
