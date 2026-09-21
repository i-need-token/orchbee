use std::{
    fmt,
    io::{self, Cursor, Read, Write},
};

use orchbee_agent_observation_protocol::{
    AGENT_MAGIC, AgentReadRequest, AgentReadResult, AgentWireError, read_agent_request,
    write_agent_result,
};
use orchbee_client_server_protocol::{
    ClientMessage, FRAME_MAGIC, PASTE_RECORD_MAGIC, ReadMessageError, ServerMessage,
    WriteMessageError, read_client_message, write_server_message,
};
use orchbee_workbench_protocol::{
    WORKBENCH_MAGIC, WorkbenchRequest, WorkbenchWireError, read_workbench_request,
    write_workbench_response,
};

use crate::agent_service::AgentReadFault;
use crate::workbench_service::WorkbenchExecution;

pub(super) fn run<Stream>(
    stream: &mut Stream,
    mut execute: impl FnMut(ClientMessage) -> ServerMessage,
    mut execute_workbench: impl FnMut(WorkbenchRequest) -> WorkbenchExecution,
    mut execute_agent: impl FnMut(AgentReadRequest) -> Result<AgentReadResult, AgentReadFault>,
) -> Result<(), ApplicationSessionError>
where
    Stream: Read + Write,
{
    loop {
        let mut prefix = [0; 4];
        stream
            .read_exact(&mut prefix)
            .map_err(ApplicationSessionError::Prefix)?;
        match prefix {
            FRAME_MAGIC | PASTE_RECORD_MAGIC => {
                let request = read_client_message(&mut Cursor::new(prefix).chain(&mut *stream))
                    .map_err(ApplicationSessionError::Read)?;
                let result = execute(request);
                write_server_message(stream, &result).map_err(ApplicationSessionError::Write)?;
            }
            WORKBENCH_MAGIC => {
                let request = read_workbench_request(&mut Cursor::new(prefix).chain(&mut *stream))
                    .map_err(ApplicationSessionError::Workbench)?;
                let WorkbenchExecution::Response(result) = execute_workbench(request) else {
                    return Err(ApplicationSessionError::WorkbenchOutcomeUnknown);
                };
                write_workbench_response(stream, &result)
                    .map_err(ApplicationSessionError::Workbench)?;
            }
            AGENT_MAGIC => {
                let request = read_agent_request(&mut Cursor::new(prefix).chain(&mut *stream))
                    .map_err(ApplicationSessionError::Agent)?;
                let result = execute_agent(request).map_err(ApplicationSessionError::AgentFault)?;
                write_agent_result(stream, &result).map_err(ApplicationSessionError::Agent)?;
            }
            _ => return Err(ApplicationSessionError::UnknownProtocol),
        }
    }
}

#[derive(Debug)]
pub(super) enum ApplicationSessionError {
    Prefix(io::Error),
    UnknownProtocol,
    WorkbenchOutcomeUnknown,
    Workbench(WorkbenchWireError),
    Agent(AgentWireError),
    AgentFault(AgentReadFault),
    Read(ReadMessageError),
    Write(WriteMessageError),
}

impl fmt::Display for ApplicationSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

impl std::error::Error for ApplicationSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
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
