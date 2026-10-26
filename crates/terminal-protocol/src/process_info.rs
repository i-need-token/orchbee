//! Encodes and validates bounded attachment-scoped process samples, preserving unavailable
//! process facts and UTF-16 text limits. It does not query the operating system or turn
//! sampled process identity into control authority.

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, AttachmentTarget, MessageRole, ProtocolError, ProtocolErrorCode, TerminalId,
    control::{RequiredNullable, SafeInteger, decode_wire, preflight_json, serialize_wire},
    terminal_fact::canonical_u64,
};

pub const READ_PROCESS_INFO_KIND: u16 = 79;
pub const MAXIMUM_PROCESS_SAMPLE_ROWS: usize = 64;
pub const MAXIMUM_PROCESS_TEXT_UTF16_UNITS: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalProcess {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub creation_time: u64,
    pub image: Option<String>,
    pub command_line: Option<String>,
    pub current_directory: Option<String>,
}

/// A sequential observation of a retained Windows Job, not foreground authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalProcessInfo {
    pub attachment_id: AttachmentId,
    pub terminal_id: TerminalId,
    pub root_pid: u32,
    pub processes: Vec<TerminalProcess>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessInfoControl {
    ReadProcessInfo(AttachmentTarget),
    ProcessInfo(TerminalProcessInfo),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedProcessInfoControl {
    pub kind: u16,
    pub role: MessageRole,
    pub payload: Vec<u8>,
}

pub fn decode_process_info_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ProcessInfoControl, ProtocolError> {
    preflight_json(payload)?;
    if kind != READ_PROCESS_INFO_KIND {
        return Err(invalid("message kind does not select process information"));
    }
    match role {
        MessageRole::Request => {
            let wire: TargetWire = decode_wire(payload)?;
            Ok(ProcessInfoControl::ReadProcessInfo(AttachmentTarget {
                attachment_id: AttachmentId::parse(&wire.attachment_id)
                    .map_err(|_| invalid("process attachment identity is invalid"))?,
            }))
        }
        MessageRole::Response => {
            let wire: InfoWire = decode_wire(payload)?;
            if wire.kind != "windows-job" || wire.processes.len() > MAXIMUM_PROCESS_SAMPLE_ROWS {
                return Err(invalid("process sample kind or row bound is invalid"));
            }
            let value = TerminalProcessInfo {
                attachment_id: AttachmentId::parse(&wire.attachment_id)
                    .map_err(|_| invalid("process attachment identity is invalid"))?,
                terminal_id: TerminalId::parse(&wire.terminal_id)
                    .map_err(|_| invalid("process terminal identity is invalid"))?,
                root_pid: pid(wire.root_pid)?,
                processes: wire
                    .processes
                    .into_iter()
                    .map(from_wire)
                    .collect::<Result<_, _>>()?,
            };
            validate(&value)?;
            Ok(ProcessInfoControl::ProcessInfo(value))
        }
        MessageRole::Event => Err(invalid("process information is never an event")),
    }
}

pub fn encode_process_info_control(
    value: &ProcessInfoControl,
) -> Result<EncodedProcessInfoControl, ProtocolError> {
    let (role, payload) = match value {
        ProcessInfoControl::ReadProcessInfo(target) => (
            MessageRole::Request,
            serialize_wire(&TargetWire {
                attachment_id: target.attachment_id.to_text(),
            })?,
        ),
        ProcessInfoControl::ProcessInfo(info) => {
            validate(info)?;
            let wire = InfoWire {
                attachment_id: info.attachment_id.to_text(),
                terminal_id: info.terminal_id.to_text(),
                kind: "windows-job".into(),
                root_pid: SafeInteger::new(u64::from(info.root_pid)),
                processes: info.processes.iter().map(to_wire).collect(),
            };
            (MessageRole::Response, serialize_wire(&wire)?)
        }
    };
    Ok(EncodedProcessInfoControl {
        kind: READ_PROCESS_INFO_KIND,
        role,
        payload,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TargetWire {
    attachment_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InfoWire {
    attachment_id: String,
    terminal_id: String,
    kind: String,
    root_pid: SafeInteger,
    processes: Vec<ProcessWire>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProcessWire {
    pid: SafeInteger,
    #[serde(default)]
    parent_pid: RequiredNullable<SafeInteger>,
    creation_time: String,
    #[serde(default)]
    image: RequiredNullable<String>,
    #[serde(default)]
    command_line: RequiredNullable<String>,
    #[serde(default)]
    current_directory: RequiredNullable<String>,
}

fn from_wire(wire: ProcessWire) -> Result<TerminalProcess, ProtocolError> {
    Ok(TerminalProcess {
        pid: pid(wire.pid)?,
        parent_pid: wire.parent_pid.into_option()?.map(pid).transpose()?,
        creation_time: canonical_u64(&wire.creation_time)?,
        image: wire.image.into_option()?,
        command_line: wire.command_line.into_option()?,
        current_directory: wire.current_directory.into_option()?,
    })
}

fn to_wire(value: &TerminalProcess) -> ProcessWire {
    ProcessWire {
        pid: SafeInteger::new(u64::from(value.pid)),
        parent_pid: RequiredNullable::from_option(
            value.parent_pid.map(|pid| SafeInteger::new(u64::from(pid))),
        ),
        creation_time: value.creation_time.to_string(),
        image: RequiredNullable::from_option(value.image.clone()),
        command_line: RequiredNullable::from_option(value.command_line.clone()),
        current_directory: RequiredNullable::from_option(value.current_directory.clone()),
    }
}

fn validate(value: &TerminalProcessInfo) -> Result<(), ProtocolError> {
    if value.root_pid == 0 || value.processes.len() > MAXIMUM_PROCESS_SAMPLE_ROWS {
        return Err(invalid("invalid root PID or process count"));
    }
    let mut previous = 0;
    for row in &value.processes {
        if row.pid <= previous || row.parent_pid == Some(0) || row.creation_time == 0 {
            return Err(invalid(
                "process rows must have positive identities and strictly increasing PIDs",
            ));
        }
        previous = row.pid;
        for text in [&row.image, &row.command_line, &row.current_directory]
            .into_iter()
            .flatten()
        {
            if text.contains('\0') || text.encode_utf16().count() > MAXIMUM_PROCESS_TEXT_UTF16_UNITS
            {
                return Err(invalid(
                    "process text contains NUL or exceeds its UTF-16 bound",
                ));
            }
        }
    }
    Ok(())
}

fn pid(value: SafeInteger) -> Result<u32, ProtocolError> {
    u32::try_from(value.get())
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid("process PID must be a positive uint32"))
}

const fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, message)
}
