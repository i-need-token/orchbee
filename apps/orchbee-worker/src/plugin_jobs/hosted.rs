//! The background programs the Terminal Host runs for this Worker under `background-programs`:
//! starting a configured command or a plugin job there, the label from which a replacement Worker
//! rebuilds its record, and collecting each program's exit and output. The Host keeps the program;
//! this module keeps only the Worker's record of it.

use super::{Failure, Inner, Outcome, Refusal, State, configured, finish_failure, new_log, now_ms};
use orchbee_capability_node_protocol::{
    ConfiguredCommandIntent, MAXIMUM_PLUGIN_OUTPUT_BYTES, OperationId, PluginCommandIntent,
    PluginCommandLog,
};
use orchbee_terminal_protocol::{
    MAXIMUM_PROGRAM_LABEL_BYTES, MAXIMUM_PROGRAM_PAGE, ProgramAnswer, ProgramExit, ProgramExited,
    ProgramId, ProgramRequest, ProgramStream, ProgramSummary, StartProgram,
};

/// A Host's answer to one program request.
pub(crate) enum Reply {
    Answer(ProgramAnswer),
    /// A `TypedFailure`: its code and, for `spawn-failed`, its reason.
    Refused {
        code: String,
        reason: Option<String>,
    },
}

/// The Host connection a program request travels on.
pub(crate) trait ProgramHost {
    /// One request and its reply; `Err` when the exchange failed, so its effect is unknown.
    fn request(&self, request: &ProgramRequest) -> Result<Reply, ()>;
    /// The exits the Host reported since the last call.
    fn take_exits(&self) -> Vec<ProgramExited>;
    /// Whether a new program starts here; one that does not runs as without a Host.
    fn starts(&self) -> bool {
        true
    }
}

/// The most programs one Host lists, and so the most disposals kept for another attempt.
const MAXIMUM_LISTED: usize = 64;

/// Starts a configured `shell` command in the Host, after the caller admitted it.
pub(super) fn start_configured(
    inner: &mut Inner,
    id: OperationId,
    intent: &ConfiguredCommandIntent,
    fingerprint: u64,
    host: &dyn ProgramHost,
) -> Outcome {
    let Some((executable, arguments)) = configured::command_processor() else {
        return Outcome::Refused(Refusal::Unavailable);
    };
    let request = ProgramRequest::Start(Box::new(StartProgram {
        program_id: id.as_bytes(),
        executable,
        arguments,
        raw_argument: Some(intent.command.clone()),
        directory: intent.directory.clone(),
        environment: environment(&intent.environment),
        capture_stdout: false,
        capture_stderr: false,
        label: encode_label(&Label::Configured { fingerprint }),
    }));
    match host.request(&request) {
        Ok(Reply::Answer(ProgramAnswer::Started {
            program_id,
            existing,
            ..
        })) if program_id == id.as_bytes() => {
            configured::admit_hosted(inner, id, fingerprint);
            Outcome::ConfiguredStarted {
                job_id: id,
                existing,
            }
        }
        Ok(Reply::Refused { code, .. }) => Outcome::Refused(refusal(&code)),
        Ok(Reply::Answer(_)) | Err(()) => Outcome::Refused(Refusal::PhysicalUnknown),
    }
}

/// Starts a plugin job in the Host, after the caller admitted it, or answers `None` for one that
/// stays with the Worker: its program does not resolve to an absolute path, or it does not fit a
/// label.
pub(super) fn start_plugin(
    inner: &mut Inner,
    id: OperationId,
    intent: &PluginCommandIntent,
    fingerprint: u64,
    host: &dyn ProgramHost,
) -> Option<Outcome> {
    let executable = plugin_executable(intent)?;
    let log = new_log(inner, id, intent);
    let label = encode_label(&Label::Plugin {
        fingerprint,
        sequence: log.sequence,
        started_ms: log.started_ms,
        plugin_id: log.plugin_id.clone(),
        command_id: log.command_id.clone(),
        command: log.command.clone(),
    });
    // A label must keep at least the program's own path to rebuild a valid log.
    if !matches!(decode_label(&label), Some(Label::Plugin { command, .. }) if !command.is_empty()) {
        return None;
    }
    let request = ProgramRequest::Start(Box::new(StartProgram {
        program_id: id.as_bytes(),
        executable,
        arguments: intent.argv[1..].to_vec(),
        raw_argument: None,
        directory: intent.directory.clone(),
        environment: environment(&intent.environment),
        capture_stdout: true,
        capture_stderr: true,
        label,
    }));
    let state = match host.request(&request) {
        Ok(Reply::Answer(ProgramAnswer::Started { program_id, .. }))
            if program_id == id.as_bytes() =>
        {
            State::Hosted {
                program: program_id,
            }
        }
        Ok(Reply::Refused { code, reason }) if code == "spawn-failed" => {
            let failure = if reason.as_deref() == Some("not-found") {
                Failure::SpawnUnavailable
            } else {
                Failure::SpawnFailed
            };
            let finished = finish_failure(log.clone(), failure);
            super::admit(inner, fingerprint, finished, State::Complete);
            return Some(Outcome::Started {
                log: Box::new(log),
                existing: false,
            });
        }
        Ok(Reply::Refused { code, .. }) => return Some(Outcome::Refused(refusal(&code))),
        Ok(Reply::Answer(_)) | Err(()) => return Some(Outcome::Refused(Refusal::PhysicalUnknown)),
    };
    super::admit(inner, fingerprint, log.clone(), state);
    Some(Outcome::Started {
        log: Box::new(log),
        existing: false,
    })
}

/// Ends a running Host program's Job; the Host then reports it as cancelled.
pub(super) fn cancel(host: Option<&dyn ProgramHost>, program: ProgramId) -> Outcome {
    match host.map(|host| host.request(&ProgramRequest::Cancel(program))) {
        Some(Ok(Reply::Answer(ProgramAnswer::Cancelled {
            program_id,
            running,
        }))) if program_id == program => Outcome::Cancellation {
            already_complete: !running,
        },
        _ => Outcome::Refused(Refusal::PhysicalUnknown),
    }
}

/// Collects the exits the Host reported, and disposes of each collected program. An exit whose
/// output could not be read is tried once more at the next call.
pub(super) fn settle(inner: &mut Inner, host: &dyn ProgramHost) {
    for exit in std::mem::take(&mut inner.pending_exits) {
        finish(inner, host, &exit, true);
    }
    for exit in host.take_exits() {
        finish(inner, host, &exit, false);
    }
    let disposals = std::mem::take(&mut inner.disposals);
    for program in disposals {
        dispose(inner, host, program);
    }
}

/// Rebuilds the records of the programs a Host already runs for the Worker this one replaces, and
/// collects those that exited. It stops at a page the Host does not answer.
pub(super) fn adopt(inner: &mut Inner, host: &dyn ProgramHost) {
    let mut after = None;
    let mut exited = Vec::new();
    for _ in 0..=MAXIMUM_LISTED / MAXIMUM_PROGRAM_PAGE {
        let Ok(Reply::Answer(ProgramAnswer::Page { programs, next })) =
            host.request(&ProgramRequest::List { after })
        else {
            break;
        };
        for summary in &programs {
            rebuild(inner, summary);
            if let Some(exit) = summary.exit {
                exited.push(ProgramExited {
                    program_id: summary.program_id,
                    process: summary.process,
                    exit,
                    stdout: summary.stdout,
                    stderr: summary.stderr,
                });
            }
        }
        if next.is_none() {
            break;
        }
        after = next;
    }
    for exit in &exited {
        finish(inner, host, exit, false);
    }
}

fn rebuild(inner: &mut Inner, summary: &ProgramSummary) {
    let Ok(id) = OperationId::from_bytes(summary.program_id) else {
        return;
    };
    if inner.records.iter().any(|record| record.log.job_id == id)
        || inner.commands.iter().any(|record| record.id == id)
    {
        return;
    }
    let program = summary.program_id;
    if let Some(Label::Plugin {
        fingerprint,
        sequence,
        started_ms,
        plugin_id,
        command_id,
        command,
    }) = decode_label(&summary.label)
    {
        let log = PluginCommandLog {
            job_id: id,
            sequence,
            plugin_id,
            command_id,
            command,
            started_ms,
            finished_ms: None,
            exit_code: None,
            failure: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        };
        if log.validate().is_ok() {
            inner.next_sequence = inner.next_sequence.max(sequence.saturating_add(1));
            super::admit(inner, fingerprint, log, State::Hosted { program });
            return;
        }
    }
    // A configured command, or a label this build cannot read, which still counts as running.
    let fingerprint = match decode_label(&summary.label) {
        Some(Label::Configured { fingerprint }) => fingerprint,
        _ => 0,
    };
    configured::admit_hosted(inner, id, fingerprint);
}

/// Finishes the record of an exited program and disposes of it. Unless this is the `last` attempt,
/// an exit whose output the Host did not answer for is kept for the next one.
fn finish(inner: &mut Inner, host: &dyn ProgramHost, exit: &ProgramExited, last: bool) {
    let program = exit.program_id;
    let hosted = inner.records.iter().position(
        |record| matches!(record.state, State::Hosted { program: held } if held == program),
    );
    if let Some(index) = hosted {
        let started = inner.records[index].log.clone();
        let log = match collect(host, started.clone(), exit) {
            Ok(log) => log,
            Err(Unread) if !last && inner.pending_exits.len() < MAXIMUM_LISTED => {
                inner.pending_exits.push(*exit);
                return;
            }
            Err(Unread) => failed(started),
        };
        let record = &mut inner.records[index];
        record.log = log;
        record.state = State::Complete;
    } else {
        configured::finish_hosted(inner, program);
    }
    dispose(inner, host, program);
}

/// The Host did not answer a request for a program's output.
struct Unread;

fn dispose(inner: &mut Inner, host: &dyn ProgramHost, program: ProgramId) {
    let disposed = matches!(
        host.request(&ProgramRequest::Dispose(program)),
        Ok(Reply::Answer(ProgramAnswer::Disposed(answered))) if answered == program
    );
    if !disposed && inner.disposals.len() < MAXIMUM_LISTED && !inner.disposals.contains(&program) {
        inner.disposals.push(program);
    }
}

/// The finished log of an exited plugin job, with its captured output read from the Host.
fn collect(
    host: &dyn ProgramHost,
    mut log: PluginCommandLog,
    exit: &ProgramExited,
) -> Result<PluginCommandLog, Unread> {
    log.finished_ms = Some(now_ms().max(log.started_ms));
    match exit.exit {
        ProgramExit::Code(code) => {
            log.exit_code = Some(i32::from_ne_bytes(code.to_ne_bytes()));
            log.failure = (code != 0).then_some(Failure::ExitFailure);
        }
        ProgramExit::Cancelled => {
            log.exit_code = None;
            log.failure = Some(Failure::Cancelled);
        }
    }
    for (stream, facts) in [
        (ProgramStream::Stdout, exit.stdout),
        (ProgramStream::Stderr, exit.stderr),
    ] {
        let Some(facts) = facts else { continue };
        let bytes = match read_stream(host, exit.program_id, stream) {
            Ok(bytes) => bytes,
            Err(Read::Unanswered) => return Err(Unread),
            Err(Read::Invalid) => return Ok(failed(log)),
        };
        // A log marks a stream truncated only once it kept the whole bound.
        let truncated = facts.truncated && bytes.len() == MAXIMUM_PLUGIN_OUTPUT_BYTES;
        match stream {
            ProgramStream::Stdout => (log.stdout, log.stdout_truncated) = (bytes, truncated),
            ProgramStream::Stderr => (log.stderr, log.stderr_truncated) = (bytes, truncated),
        }
    }
    Ok(if log.validate().is_ok() {
        log
    } else {
        failed(log)
    })
}

/// A plugin job's log finished without output, as an I/O failure.
fn failed(log: PluginCommandLog) -> PluginCommandLog {
    PluginCommandLog {
        stdout: Vec::new(),
        stderr: Vec::new(),
        stdout_truncated: false,
        stderr_truncated: false,
        ..finish_failure(log, Failure::Io)
    }
}

/// Why a captured stream was not read.
enum Read {
    /// The exchange failed, so a later attempt may still read it.
    Unanswered,
    /// The Host answered out of step with the request.
    Invalid,
}

fn read_stream(
    host: &dyn ProgramHost,
    program: ProgramId,
    stream: ProgramStream,
) -> Result<Vec<u8>, Read> {
    let mut data = Vec::new();
    // The whole bound takes three reads; a Host that sends less than it promised fails sooner.
    for _ in 0..8 {
        let offset = u32::try_from(data.len()).map_err(|_| Read::Invalid)?;
        let reply = host
            .request(&ProgramRequest::ReadOutput {
                program_id: program,
                stream,
                offset,
            })
            .map_err(|()| Read::Unanswered)?;
        let Reply::Answer(ProgramAnswer::Output {
            program_id,
            stream: answered,
            offset: answered_offset,
            data: chunk,
            end,
        }) = reply
        else {
            return Err(Read::Invalid);
        };
        if program_id != program || answered != stream || answered_offset != offset {
            return Err(Read::Invalid);
        }
        data.extend_from_slice(&chunk);
        if data.len() > MAXIMUM_PLUGIN_OUTPUT_BYTES {
            return Err(Read::Invalid);
        }
        if end {
            return Ok(data);
        }
        if chunk.is_empty() {
            return Err(Read::Invalid);
        }
    }
    Err(Read::Invalid)
}

fn refusal(code: &str) -> Refusal {
    match code {
        "program-limit" | "resource-exhausted" => Refusal::Busy,
        _ => Refusal::Unavailable,
    }
}

/// The environment's changes as the Worker's own launch makes them: the invocation's context
/// removed, then the intent's variables, a later name replacing an earlier one ignoring ASCII case.
fn environment(variables: &[(String, String)]) -> Vec<(String, Option<String>)> {
    let mut changes: Vec<(String, Option<String>)> = Vec::new();
    let removals = super::process::context_removals()
        .into_iter()
        .filter_map(|name| name.into_string().ok())
        .map(|name| (name, None));
    let settings = variables
        .iter()
        .map(|(name, value)| (name.clone(), Some(value.clone())));
    for (name, value) in removals.chain(settings) {
        changes.retain(|(earlier, _)| !earlier.eq_ignore_ascii_case(&name));
        changes.push((name, value));
    }
    changes
}

#[cfg(windows)]
fn plugin_executable(intent: &PluginCommandIntent) -> Option<String> {
    let program = orchbee_local_runtime::resolve_windows_program(
        intent.argv.first()?,
        std::path::Path::new(&intent.directory),
    );
    program
        .is_absolute()
        .then(|| program.into_os_string().into_string().ok())
        .flatten()
}

#[cfg(not(windows))]
fn plugin_executable(_intent: &PluginCommandIntent) -> Option<String> {
    None
}

/// What a label records: the kind of program, the fingerprint of the intent that started it, and
/// for a plugin job its log's header with as many whole arguments of its command as fit.
#[derive(Debug, Eq, PartialEq)]
enum Label {
    Configured {
        fingerprint: u64,
    },
    Plugin {
        fingerprint: u64,
        sequence: u64,
        started_ms: u64,
        plugin_id: String,
        command_id: String,
        command: Vec<String>,
    },
}

const LABEL_VERSION: u8 = 1;
const CONFIGURED_LABEL: u8 = 1;
const PLUGIN_LABEL: u8 = 2;
/// A label is lowercase hexadecimal, which the base64 alphabet holds, two digits to a byte.
const LABEL_BYTES: usize = MAXIMUM_PROGRAM_LABEL_BYTES / 2;

fn encode_label(label: &Label) -> String {
    let mut bytes = vec![LABEL_VERSION];
    match label {
        Label::Configured { fingerprint } => {
            bytes.push(CONFIGURED_LABEL);
            bytes.extend(fingerprint.to_le_bytes());
        }
        Label::Plugin {
            fingerprint,
            sequence,
            started_ms,
            plugin_id,
            command_id,
            command,
        } => {
            bytes.push(PLUGIN_LABEL);
            bytes.extend(fingerprint.to_le_bytes());
            bytes.extend(sequence.to_le_bytes());
            bytes.extend(started_ms.to_le_bytes());
            push_text(&mut bytes, plugin_id);
            push_text(&mut bytes, command_id);
            let count_at = bytes.len();
            bytes.extend([0, 0]);
            let mut count: u16 = 0;
            for argument in command {
                if bytes.len() + 2 + argument.len() > LABEL_BYTES {
                    break;
                }
                push_text(&mut bytes, argument);
                count += 1;
            }
            bytes[count_at..count_at + 2].copy_from_slice(&count.to_le_bytes());
        }
    }
    bytes.truncate(LABEL_BYTES);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_label(text: &str) -> Option<Label> {
    if !text.len().is_multiple_of(2) || text.len() > MAXIMUM_PROGRAM_LABEL_BYTES {
        return None;
    }
    let bytes = (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let mut reader = bytes.as_slice();
    if take(&mut reader, 1)? != [LABEL_VERSION] {
        return None;
    }
    let kind = take(&mut reader, 1)?[0];
    let fingerprint = take_u64(&mut reader)?;
    let label = match kind {
        CONFIGURED_LABEL => Label::Configured { fingerprint },
        PLUGIN_LABEL => {
            let sequence = take_u64(&mut reader)?;
            let started_ms = take_u64(&mut reader)?;
            let plugin_id = take_text(&mut reader)?;
            let command_id = take_text(&mut reader)?;
            let count = u16::from_le_bytes(take(&mut reader, 2)?.try_into().ok()?);
            let command = (0..count)
                .map(|_| take_text(&mut reader))
                .collect::<Option<Vec<_>>>()?;
            Label::Plugin {
                fingerprint,
                sequence,
                started_ms,
                plugin_id,
                command_id,
                command,
            }
        }
        _ => return None,
    };
    reader.is_empty().then_some(label)
}

fn push_text(bytes: &mut Vec<u8>, text: &str) {
    let length = u16::try_from(text.len()).unwrap_or(u16::MAX);
    bytes.extend(length.to_le_bytes());
    bytes.extend(&text.as_bytes()[..usize::from(length)]);
}

fn take<'a>(reader: &mut &'a [u8], count: usize) -> Option<&'a [u8]> {
    let (taken, rest) = reader.split_at_checked(count)?;
    *reader = rest;
    Some(taken)
}

fn take_u64(reader: &mut &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(take(reader, 8)?.try_into().ok()?))
}

fn take_text(reader: &mut &[u8]) -> Option<String> {
    let length = u16::from_le_bytes(take(reader, 2)?.try_into().ok()?);
    String::from_utf8(take(reader, usize::from(length))?.to_vec()).ok()
}
