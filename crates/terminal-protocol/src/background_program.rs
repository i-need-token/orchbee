//! Defines the `background-programs` requests a Worker sends a Windows Terminal Host to run a
//! program without a terminal, the answers it reads, and `ProgramExited`. It transports them
//! without deciding which programs run in the Host or what a label records.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::{
    HandoverProcess, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{SafeInteger, decode_wire, preflight_json, serialize_wire},
    handover::{ProcessWire, hex, process_from_wire},
};

pub const START_PROGRAM_KIND: u16 = 110;
pub const PROGRAM_EXITED_KIND: u16 = 111;
pub const CANCEL_PROGRAM_KIND: u16 = 112;
pub const READ_PROGRAM_OUTPUT_KIND: u16 = 113;
pub const DISPOSE_PROGRAM_KIND: u16 = 114;
pub const LIST_PROGRAMS_KIND: u16 = 115;
/// A label's longest text, in characters of the base64 alphabet.
pub const MAXIMUM_PROGRAM_LABEL_BYTES: usize = 2048;
/// The most bytes of one stream a `ReadProgramOutput` answer carries.
pub const MAXIMUM_PROGRAM_OUTPUT_CHUNK_BYTES: usize = 24 * 1024;
/// The most programs one `ListPrograms` page lists.
pub const MAXIMUM_PROGRAM_PAGE: usize = 16;

/// A program's id, which its Worker chooses: 16 bytes, not all zero.
pub type ProgramId = [u8; 16];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramStream {
    Stdout,
    Stderr,
}

/// `StartProgram`: an absolute executable and directory, the environment's changes, which streams
/// reach the Host, and the Worker's opaque label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartProgram {
    pub program_id: ProgramId,
    pub executable: String,
    pub arguments: Vec<String>,
    pub raw_argument: Option<String>,
    pub directory: String,
    /// Each name with its new value, or `None` to remove it.
    pub environment: Vec<(String, Option<String>)>,
    pub capture_stdout: bool,
    pub capture_stderr: bool,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramRequest {
    Start(Box<StartProgram>),
    Cancel(ProgramId),
    ReadOutput {
        program_id: ProgramId,
        stream: ProgramStream,
        offset: u32,
    },
    Dispose(ProgramId),
    /// The page after the program id, or the first.
    List {
        after: Option<ProgramId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramExit {
    Code(u32),
    Cancelled,
}

/// What a captured stream kept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramStreamFacts {
    pub bytes: u32,
    pub truncated: bool,
}

/// A listed program; `exit` and the stream facts are present once it exited.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramSummary {
    pub program_id: ProgramId,
    pub process: HandoverProcess,
    pub label: String,
    pub exit: Option<ProgramExit>,
    pub stdout: Option<ProgramStreamFacts>,
    pub stderr: Option<ProgramStreamFacts>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramExited {
    pub program_id: ProgramId,
    pub process: HandoverProcess,
    pub exit: ProgramExit,
    pub stdout: Option<ProgramStreamFacts>,
    pub stderr: Option<ProgramStreamFacts>,
}

/// The successful answer to a [`ProgramRequest`] of the same kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramAnswer {
    Started {
        program_id: ProgramId,
        process: HandoverProcess,
        existing: bool,
    },
    /// Whether the program was still running and is now being ended.
    Cancelled {
        program_id: ProgramId,
        running: bool,
    },
    Output {
        program_id: ProgramId,
        stream: ProgramStream,
        offset: u32,
        data: Vec<u8>,
        end: bool,
    },
    Disposed(ProgramId),
    Page {
        programs: Vec<ProgramSummary>,
        next: Option<ProgramId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedProgramRequest {
    pub kind: u16,
    pub payload: Vec<u8>,
}

pub fn encode_program_request(
    value: &ProgramRequest,
) -> Result<EncodedProgramRequest, ProtocolError> {
    let (kind, payload) = match value {
        ProgramRequest::Start(start) => {
            (START_PROGRAM_KIND, serialize_wire(&start_to_wire(start)?)?)
        }
        ProgramRequest::Cancel(id) => (CANCEL_PROGRAM_KIND, serialize_wire(&program_wire(id)?)?),
        ProgramRequest::ReadOutput {
            program_id,
            stream,
            offset,
        } => (
            READ_PROGRAM_OUTPUT_KIND,
            serialize_wire(&ReadOutputWire {
                offset: SafeInteger::new(u64::from(*offset)),
                program_id: id_text(program_id)?,
                stream: stream_text(*stream).to_owned(),
            })?,
        ),
        ProgramRequest::Dispose(id) => (DISPOSE_PROGRAM_KIND, serialize_wire(&program_wire(id)?)?),
        ProgramRequest::List { after } => (
            LIST_PROGRAMS_KIND,
            serialize_wire(&ListWire {
                after: after.as_ref().map(id_text).transpose()?,
            })?,
        ),
    };
    Ok(EncodedProgramRequest { kind, payload })
}

/// Decodes the successful answer of kind `kind` a Worker reads for its own request.
pub fn decode_program_answer(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ProgramAnswer, ProtocolError> {
    preflight_json(payload)?;
    if role != MessageRole::Response {
        return Err(invalid_field("a program answer is a response"));
    }
    match kind {
        START_PROGRAM_KIND => {
            let wire = decode_wire::<StartedWire>(payload)?;
            Ok(ProgramAnswer::Started {
                program_id: id_from_text(&wire.program_id)?,
                process: process_from_wire(&wire.process)?,
                existing: wire.existing,
            })
        }
        CANCEL_PROGRAM_KIND => {
            let wire = decode_wire::<CancelledWire>(payload)?;
            let running = match wire.outcome.as_str() {
                "cancelling" => true,
                "exited" => false,
                _ => return Err(invalid_field("a cancellation names no outcome")),
            };
            Ok(ProgramAnswer::Cancelled {
                program_id: id_from_text(&wire.program_id)?,
                running,
            })
        }
        READ_PROGRAM_OUTPUT_KIND => {
            let wire = decode_wire::<OutputWire>(payload)?;
            let data = base64::engine::general_purpose::STANDARD
                .decode(&wire.data)
                .map_err(|_error| invalid_field("program output is not base64"))?;
            if data.len() > MAXIMUM_PROGRAM_OUTPUT_CHUNK_BYTES {
                return Err(invalid_field("program output exceeds its chunk"));
            }
            Ok(ProgramAnswer::Output {
                program_id: id_from_text(&wire.program_id)?,
                stream: stream_from_text(&wire.stream)?,
                offset: u32_from(wire.offset)?,
                data,
                end: wire.end,
            })
        }
        DISPOSE_PROGRAM_KIND => {
            let wire = decode_wire::<ProgramWire>(payload)?;
            Ok(ProgramAnswer::Disposed(id_from_text(&wire.program_id)?))
        }
        LIST_PROGRAMS_KIND => {
            let wire = decode_wire::<PageWire>(payload)?;
            if wire.programs.len() > MAXIMUM_PROGRAM_PAGE
                || (wire.next.is_some() && wire.programs.is_empty())
            {
                return Err(invalid_field("a program page is out of bounds"));
            }
            let programs = wire
                .programs
                .iter()
                .map(summary_from_wire)
                .collect::<Result<Vec<_>, _>>()?;
            if programs
                .windows(2)
                .any(|pair| pair[0].program_id >= pair[1].program_id)
            {
                return Err(invalid_field("a program page is not in ascending order"));
            }
            Ok(ProgramAnswer::Page {
                programs,
                next: wire.next.as_deref().map(id_from_text).transpose()?,
            })
        }
        _ => Err(invalid_field(
            "message kind does not select a program answer",
        )),
    }
}

pub fn decode_program_exited(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ProgramExited, ProtocolError> {
    preflight_json(payload)?;
    if (kind, role) != (PROGRAM_EXITED_KIND, MessageRole::Event) {
        return Err(invalid_field(
            "message kind and role do not select ProgramExited",
        ));
    }
    let wire = decode_wire::<ExitedWire>(payload)?;
    Ok(ProgramExited {
        program_id: id_from_text(&wire.program_id)?,
        process: process_from_wire(&wire.process)?,
        exit: exit_from_wire(&wire.exit)?,
        stdout: wire.stdout.map(facts_from_wire).transpose()?,
        stderr: wire.stderr.map(facts_from_wire).transpose()?,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureWire {
    stderr: bool,
    stdout: bool,
}

#[derive(Serialize)]
struct VariableWire {
    name: String,
    value: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartWire {
    arguments: Vec<String>,
    capture: CaptureWire,
    directory: String,
    environment: Vec<VariableWire>,
    executable: String,
    label: String,
    program_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_argument: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProgramWire {
    program_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadOutputWire {
    offset: SafeInteger,
    program_id: String,
    stream: String,
}

#[derive(Serialize)]
struct ListWire {
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartedWire {
    existing: bool,
    process: ProcessWire,
    program_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CancelledWire {
    outcome: String,
    program_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OutputWire {
    data: String,
    end: bool,
    offset: SafeInteger,
    program_id: String,
    stream: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExitWire {
    #[serde(default)]
    code: Option<SafeInteger>,
    #[serde(default)]
    cancelled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactsWire {
    bytes: SafeInteger,
    truncated: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SummaryWire {
    #[serde(default)]
    exit: Option<ExitWire>,
    label: String,
    process: ProcessWire,
    program_id: String,
    state: String,
    #[serde(default)]
    stderr: Option<FactsWire>,
    #[serde(default)]
    stdout: Option<FactsWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageWire {
    #[serde(default)]
    next: Option<String>,
    programs: Vec<SummaryWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExitedWire {
    exit: ExitWire,
    process: ProcessWire,
    program_id: String,
    #[serde(default)]
    stderr: Option<FactsWire>,
    #[serde(default)]
    stdout: Option<FactsWire>,
}

fn start_to_wire(value: &StartProgram) -> Result<StartWire, ProtocolError> {
    if !valid_label(&value.label) {
        return Err(invalid_field(
            "a program label is more than 2,048 characters of the base64 alphabet",
        ));
    }
    Ok(StartWire {
        arguments: value.arguments.clone(),
        capture: CaptureWire {
            stderr: value.capture_stderr,
            stdout: value.capture_stdout,
        },
        directory: value.directory.clone(),
        environment: value
            .environment
            .iter()
            .map(|(name, value)| VariableWire {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        executable: value.executable.clone(),
        label: value.label.clone(),
        program_id: id_text(&value.program_id)?,
        raw_argument: value.raw_argument.clone(),
    })
}

fn summary_from_wire(wire: &SummaryWire) -> Result<ProgramSummary, ProtocolError> {
    let exit = wire.exit.as_ref().map(exit_from_wire).transpose()?;
    let consistent = match wire.state.as_str() {
        "running" => exit.is_none() && wire.stdout.is_none() && wire.stderr.is_none(),
        "exited" => exit.is_some(),
        _ => false,
    };
    if !consistent || !valid_label(&wire.label) {
        return Err(invalid_field("a listed program is inconsistent"));
    }
    Ok(ProgramSummary {
        program_id: id_from_text(&wire.program_id)?,
        process: process_from_wire(&wire.process)?,
        label: wire.label.clone(),
        exit,
        stdout: wire.stdout.as_ref().map(facts_ref).transpose()?,
        stderr: wire.stderr.as_ref().map(facts_ref).transpose()?,
    })
}

fn exit_from_wire(wire: &ExitWire) -> Result<ProgramExit, ProtocolError> {
    match (wire.code, wire.cancelled) {
        (Some(code), None) => Ok(ProgramExit::Code(u32_from(code)?)),
        (None, Some(true)) => Ok(ProgramExit::Cancelled),
        _ => Err(invalid_field("a program exit is a code or a cancellation")),
    }
}

fn facts_from_wire(wire: FactsWire) -> Result<ProgramStreamFacts, ProtocolError> {
    facts_ref(&wire)
}

fn facts_ref(wire: &FactsWire) -> Result<ProgramStreamFacts, ProtocolError> {
    Ok(ProgramStreamFacts {
        bytes: u32_from(wire.bytes)?,
        truncated: wire.truncated,
    })
}

fn program_wire(id: &ProgramId) -> Result<ProgramWire, ProtocolError> {
    Ok(ProgramWire {
        program_id: id_text(id)?,
    })
}

fn id_text(id: &ProgramId) -> Result<String, ProtocolError> {
    if *id == [0; 16] {
        return Err(invalid_field("a program id is zero"));
    }
    Ok(hex(id))
}

fn id_from_text(text: &str) -> Result<ProgramId, ProtocolError> {
    let bytes = text.as_bytes();
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    let mut id = [0; 16];
    if bytes.len() != 32 {
        return Err(invalid_field("a program id is not 32 hexadecimal digits"));
    }
    for (index, byte) in id.iter_mut().enumerate() {
        let high = digit(bytes[index * 2]);
        let low = digit(bytes[index * 2 + 1]);
        let (Some(high), Some(low)) = (high, low) else {
            return Err(invalid_field("a program id is not 32 hexadecimal digits"));
        };
        *byte = (high << 4) | low;
    }
    if id == [0; 16] {
        return Err(invalid_field("a program id is zero"));
    }
    Ok(id)
}

const fn stream_text(stream: ProgramStream) -> &'static str {
    match stream {
        ProgramStream::Stdout => "stdout",
        ProgramStream::Stderr => "stderr",
    }
}

fn stream_from_text(text: &str) -> Result<ProgramStream, ProtocolError> {
    match text {
        "stdout" => Ok(ProgramStream::Stdout),
        "stderr" => Ok(ProgramStream::Stderr),
        _ => Err(invalid_field("a program stream is stdout or stderr")),
    }
}

fn valid_label(label: &str) -> bool {
    label.len() <= MAXIMUM_PROGRAM_LABEL_BYTES
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
}

fn u32_from(value: SafeInteger) -> Result<u32, ProtocolError> {
    u32::try_from(value.get()).map_err(|_error| invalid_field("a program count exceeds 32 bits"))
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
