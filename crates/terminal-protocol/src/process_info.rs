//! Encodes and validates bounded attachment-scoped process samples as compact positional rows,
//! preserving opaque rows, console attachment, unavailable process facts, the Host's own launch
//! kind when asked, and UTF-16 text limits. It does not query the operating system or turn
//! sampled process identity into control authority.

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, AttachmentTarget, MessageRole, ProtocolError, ProtocolErrorCode, TerminalId,
    control::{SafeInteger, decode_wire, preflight_json, serialize_wire},
    terminal_fact::canonical_u64,
};

pub const READ_PROCESS_INFO_KIND: u16 = 79;
pub const MAXIMUM_PROCESS_SAMPLE_ROWS: usize = 256;
pub const MAXIMUM_PROCESS_TEXT_UTF16_UNITS: usize = 4096;

/// Whether a member shares the Terminal root's console, as the Host observed it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalProcessConsole {
    /// Attached to the Terminal's console.
    Attached,
    /// Not attached to it: another console, such as one opened by `start`, or a GUI-subsystem
    /// program, which never reports a console host.
    Detached,
    /// A console program without any console, such as one started with `DETACHED_PROCESS`.
    None,
    /// Not observable, including every opaque row and a sample whose root has exited.
    Unknown,
}

/// How the Host launched the Terminal root, from its own launch record rather than a command
/// line. Every launch off Windows is `Other`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalLaunchKind {
    /// PowerShell given the managed startup, which marks its own prompt.
    ManagedPowershell,
    /// An interactive cmd given prompt marks.
    Cmd,
    Other,
}

/// One live Job member. A row without `image` is opaque: the member denied limited query
/// rights, so only its identity and parent are known. Consumers never treat it as absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalProcess {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub creation_time: u64,
    pub console: TerminalProcessConsole,
    pub image: Option<String>,
    pub command_line: Option<String>,
    pub current_directory: Option<String>,
}

/// A sequential observation of a retained Windows Job, not foreground authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalProcessInfo {
    pub attachment_id: AttachmentId,
    pub terminal_id: TerminalId,
    /// Present only when the request asked for it with `include_launch_kind`.
    pub launch_kind: Option<TerminalLaunchKind>,
    pub root_pid: u32,
    pub processes: Vec<TerminalProcess>,
}

/// A process sample request. Without `include_launch_kind` the response keeps the shape that
/// predates the launch kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadProcessInfo {
    pub target: AttachmentTarget,
    pub include_launch_kind: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessInfoControl {
    ReadProcessInfo(ReadProcessInfo),
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
            let wire: ReadWire = decode_wire(payload)?;
            Ok(ProcessInfoControl::ReadProcessInfo(ReadProcessInfo {
                target: AttachmentTarget {
                    attachment_id: AttachmentId::parse(&wire.attachment_id)
                        .map_err(|_| invalid("process attachment identity is invalid"))?,
                },
                include_launch_kind: wire.include_launch_kind.unwrap_or(false),
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
                launch_kind: wire.launch_kind,
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
        ProcessInfoControl::ReadProcessInfo(read) => (
            MessageRole::Request,
            serialize_wire(&ReadWire {
                attachment_id: read.target.attachment_id.to_text(),
                include_launch_kind: read.include_launch_kind.then_some(true),
            })?,
        ),
        ProcessInfoControl::ProcessInfo(info) => {
            validate(info)?;
            let wire = InfoWire {
                attachment_id: info.attachment_id.to_text(),
                terminal_id: info.terminal_id.to_text(),
                kind: "windows-job".into(),
                launch_kind: info.launch_kind,
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

/// `includeLaunchKind` is written only when true, so a request that does not ask keeps its
/// earlier bytes; a reader accepts an explicit `false`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadWire {
    attachment_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    include_launch_kind: Option<bool>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InfoWire {
    attachment_id: String,
    terminal_id: String,
    kind: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    launch_kind: Option<TerminalLaunchKind>,
    root_pid: SafeInteger,
    processes: Vec<ProcessWire>,
}

/// `[pid, parentPid, creationTime, console, image, commandLine, currentDirectory]`; a JSON
/// array of any other length is rejected.
#[derive(Deserialize, Serialize)]
struct ProcessWire(
    SafeInteger,
    Option<SafeInteger>,
    String,
    TerminalProcessConsole,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// An absent optional member is `None`; a present one, including `null`, must hold a value.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn from_wire(wire: ProcessWire) -> Result<TerminalProcess, ProtocolError> {
    let ProcessWire(pid_value, parent_pid, creation_time, console, image, command, directory) =
        wire;
    Ok(TerminalProcess {
        pid: pid(pid_value)?,
        parent_pid: parent_pid.map(pid).transpose()?,
        creation_time: canonical_u64(&creation_time)?,
        console,
        image,
        command_line: command,
        current_directory: directory,
    })
}

fn to_wire(value: &TerminalProcess) -> ProcessWire {
    ProcessWire(
        SafeInteger::new(u64::from(value.pid)),
        value.parent_pid.map(|pid| SafeInteger::new(u64::from(pid))),
        value.creation_time.to_string(),
        value.console,
        value.image.clone(),
        value.command_line.clone(),
        value.current_directory.clone(),
    )
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
