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
    WORKBENCH_MAGIC, WorkbenchRequest, WorkbenchWireError, read_workbench_request,
    write_workbench_response,
};

use crate::agent_service::AgentReadFault;
use crate::workbench_service::WorkbenchExecution;

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
