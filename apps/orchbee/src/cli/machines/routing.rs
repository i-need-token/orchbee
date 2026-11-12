//! One-shot remote native CLI relay. SSH failure never authorizes a retry.
use super::{Context, Error, ssh};
use crate::cli_text::CliText;
use crate::{
    cli::{
        self, Failure, FailureKind, Output,
        console_interrupt::{self as source, ArmedSignals},
    },
    invocation::{self, OutputFormat, TerminalSendInput, machine_routing::Route},
    localization::Locale,
    machines::{Catalog, Error as CatalogError, Profile, Store},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use orchbee_connection_handshake::{Delivery, Extensions, Generation};
use orchbee_local_runtime::{CommandFailure, SessionName};
use serde::{Deserialize, Serialize};
use std::{
    io,
    process::Command,
    time::{Duration, Instant},
};
#[cfg(windows)]
#[path = "routing/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "routing/unix.rs"]
mod platform;

const REQUEST_LIMIT: usize = 2 * 1024 * 1024;
const REPLY_LIMIT: usize = 24 * 1024 * 1024;
const ARGUMENT_LIMIT: usize = 64 * 1024;
/// The routed command packet. `version` is its `machine-route` generation; `extensions` is the
/// only open member, whose content a receiver ignores.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Packet {
    version: u16,
    session: String,
    arguments: Vec<String>,
    input: Option<String>,
    locale: String,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}
/// The generation this build writes.
fn generation() -> u16 {
    Delivery::MachineRoute
        .supported_generations()
        .highest()
        .get()
}
/// Whether a received `version` names a generation this build reads.
fn supported(version: u16) -> bool {
    Generation::new(version).is_some_and(|generation| {
        Delivery::MachineRoute
            .supported_generations()
            .contains(generation)
    })
}
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Outcome {
    NotSubmitted,
    Completed,
    Unknown,
}
impl Outcome {
    fn token(self) -> &'static str {
        match self {
            Self::NotSubmitted => "not-submitted",
            Self::Completed => "completed",
            Self::Unknown => "unknown",
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    version: u16,
    outcome: Outcome,
    status: Option<u8>,
    stdout: String,
    stderr: String,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}
impl Reply {
    fn failure(outcome: Outcome) -> Self {
        Self {
            version: generation(),
            outcome,
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            extensions: Extensions::new(),
        }
    }
    /// The remote CLI's exit status and output, reported exactly as the remote produced them.
    fn completed(status: u8, stdout: &[u8], stderr: &[u8]) -> Self {
        Self {
            version: generation(),
            outcome: Outcome::Completed,
            status: Some(status),
            stdout: STANDARD.encode(stdout),
            stderr: STANDARD.encode(stderr),
            extensions: Extensions::new(),
        }
    }
    fn output(self) -> Result<Output, Outcome> {
        if !supported(self.version) {
            return Err(Outcome::Unknown);
        }
        if self.outcome != Outcome::Completed || self.status.is_none() {
            return Err(
                if self.outcome == Outcome::NotSubmitted
                    && self.status.is_none()
                    && self.stdout.is_empty()
                    && self.stderr.is_empty()
                {
                    Outcome::NotSubmitted
                } else {
                    Outcome::Unknown
                },
            );
        }
        let decode = |value: &str| {
            STANDARD
                .decode(value)
                .ok()
                .filter(|v| v.len() <= cli::output::LIMIT)
        };
        let stdout = decode(&self.stdout).ok_or(Outcome::Unknown)?;
        let stderr = String::from_utf8(decode(&self.stderr).ok_or(Outcome::Unknown)?)
            .map_err(|_| Outcome::Unknown)?;
        Ok(Output {
            status: self.status.unwrap(),
            stdout,
            stderr,
            finalization: None,
        })
    }
}
/// The machine `--machine` names, by the catalog's one `<id|label>` rule, when it may be routed to.
fn select<'a>(catalog: &'a Catalog, selector: &str) -> Result<&'a Profile, &'static str> {
    let profile = catalog.resolve(selector).map_err(|error| match error {
        CatalogError::Ambiguous => "ambiguous-machine",
        _ => "not-found",
    })?;
    if !profile.enabled {
        return Err("disabled-machine");
    }
    if !profile.draft.connection.supported() {
        return Err("unsupported-machine");
    }
    Ok(profile)
}
/// A machine this build cannot reach is unavailable here; every other selection refusal is 3.
fn selection_status(reason: &str) -> u8 {
    if reason == "unsupported-machine" {
        4
    } else {
        3
    }
}
fn failed(
    code: &str,
    status: u8,
    outcome: Outcome,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let kind = if outcome == Outcome::Unknown {
        FailureKind::OutcomeUnknown
    } else {
        match status {
            3 => FailureKind::InvalidUsage,
            6 => FailureKind::Incompatible,
            7 => FailureKind::Timeout,
            130 => FailureKind::Cancelled,
            _ => FailureKind::ServiceUnavailable,
        }
    };
    let mut result = cli::failure_output(
        Failure::new(kind)
            .failure_schema("orchbee.cli.failure.machine-route")
            .details(serde_json::Map::from_iter([
                ("stage".into(), serde_json::json!("machine-route")),
                ("reason".into(), serde_json::json!(code)),
                ("remoteOutcome".into(), serde_json::json!(outcome.token())),
            ])),
        Some(format),
        locale,
    );
    result.status = status;
    result.stderr = format!(
        "{} [{}; remoteOutcome={}]\n",
        CliText::new(locale).cli_machine_route_failure(outcome == Outcome::Unknown),
        code,
        outcome.token()
    );
    // The remote or this build refused until one side changes, so name what must change.
    if matches!(
        code,
        "unsupported-machine"
            | "remote-unsupported"
            | "remote-not-installed"
            | "remote-other-environment"
    ) {
        result
            .stderr
            .push_str(CliText::new(locale).cli_machine_message(code));
        result.stderr.push('\n');
    }
    result
}
pub(crate) fn run(route: Route, locale: Locale) -> Output {
    if crate::tui_capabilities::Capability::SavedMachines
        .require()
        .is_err()
    {
        return cli::unsupported_platform(Some(route.format), locale);
    }
    let signals = match ArmedSignals::arm_interrupts() {
        Ok(v) => v,
        Err(_) => return failed("internal", 1, Outcome::NotSubmitted, route.format, locale),
    };
    let mut context = Context {
        signals,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    let mut outcome = Outcome::NotSubmitted;
    let result = execute(&route, locale, &mut context, &mut outcome);
    let mut output = match result {
        Ok(output) => output,
        Err((code, status)) => failed(code, status, outcome, route.format, locale),
    };
    // Preserve a completed remote receipt even if cancellation arrives afterwards.
    output.finalization = Some(context.signals.into_finalization());
    output
}
fn mapped(error: Error) -> (&'static str, u8) {
    (error.code(), error.status())
}
fn execute(
    route: &Route,
    locale: Locale,
    context: &mut Context,
    outcome: &mut Outcome,
) -> Result<Output, (&'static str, u8)> {
    let catalog = Store::configured()
        .and_then(|s| s.read())
        .map_err(|e| mapped(Error::Catalog(e)))?;
    let profile = select(&catalog, &route.selector).map_err(|e| (e, selection_status(e)))?;
    let target = profile
        .draft
        .ssh_target()
        .ok_or(("unsupported-machine", 4))?;
    let mut arguments = route.arguments.clone();
    let input = if let Some(input) = &route.input {
        let (path, paste) = match input {
            TerminalSendInput::File { path, paste, .. } => (Some(path.as_str()), *paste),
            TerminalSendInput::Stdin { paste, .. } => (None, *paste),
            _ => return Err(("internal", 1)),
        };
        let limit = if paste {
            invocation::MAXIMUM_SEND_PASTE_BYTES
        } else {
            invocation::MAXIMUM_SEND_BYTES_BYTES
        };
        let bytes = source::acquire_document(path, limit, context.deadline, &mut context.signals)
            .map_err(|error| match error {
            source::SourceFailure::Cancelled { .. } => ("cancelled", 130),
            source::SourceFailure::Timeout { .. } => ("timeout", 7),
            _ => ("input-source", 3),
        })?;
        if path.is_some() {
            let index = arguments
                .iter()
                .position(|a| a == "--file" || a.starts_with("--file="))
                .ok_or(("internal", 1))?;
            let separate = arguments[index] == "--file";
            arguments[index] = "--stdin".into();
            if separate {
                arguments.remove(index + 1);
            }
        }
        Some(STANDARD.encode(bytes))
    } else {
        None
    };
    context.deadline = Instant::now() + Duration::from_secs(60);
    context.check().map_err(mapped)?;
    let ssh_path = platform::ssh_path().map_err(|_| ("ssh-unavailable", 4))?;
    let probe = ssh::exchange(
        &ssh_path,
        target,
        &ssh::program(None, profile.draft.session.as_str(), "client"),
        context,
    )
    .map_err(mapped)?;
    let client = ssh::client(&probe).map_err(mapped)?;
    let packet = Packet {
        version: generation(),
        session: profile.draft.session.as_str().into(),
        arguments,
        input,
        locale: locale_name(locale).into(),
        extensions: Extensions::new(),
    };
    let bytes = serde_json::to_vec(&packet).map_err(|_| ("internal", 1))?;
    if bytes.len() > REQUEST_LIMIT {
        return Err(("input-source", 3));
    }
    context.check().map_err(mapped)?;
    *outcome = Outcome::Unknown;
    let command = ssh::command(&ssh_path, target, &relay_program(&client.executable));
    let result = platform::exchange(command, Some(&bytes), REPLY_LIMIT, None, || {
        context.interrupted()
    })
    .map_err(|_| {
        *outcome = Outcome::NotSubmitted;
        ("ssh-unavailable", 4)
    })?;
    if matches!(
        result.failure,
        Some(CommandFailure::SpawnUnavailable | CommandFailure::SpawnFailed)
    ) {
        *outcome = Outcome::NotSubmitted;
    }
    if let Some(failure) = result.failure {
        return Err(match failure {
            CommandFailure::Cancelled => ("cancelled", 130),
            CommandFailure::Timeout => ("timeout", 7),
            CommandFailure::PhysicalUnknown => ("physical-unknown", 8),
            _ => ("ssh-failed", 5),
        });
    }
    if result.exit_code != Some(0) || result.stdout_truncated {
        return Err(("remote-protocol", 5));
    }
    let reply: Reply =
        serde_json::from_slice(&result.stdout).map_err(|_| ("remote-protocol", 5))?;
    match reply.output() {
        Ok(output) => {
            *outcome = Outcome::Completed;
            Ok(output)
        }
        Err(known) => {
            *outcome = known;
            Err(("remote-rejected-or-interrupted", 5))
        }
    }
}
fn relay_program(executable: &str) -> String {
    // Only the admitted executable is a shell literal. All command arguments and
    // payload are pipe data; PowerShell never constructs their command line.
    let script = format!(
        "$ErrorActionPreference='Stop';try{{& {} '--machine-relay-v1';exit $LASTEXITCODE}}catch{{exit 125}}",
        ssh::literal(executable)
    );
    let encoded = STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    format!("powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {encoded}")
}
pub(crate) fn serve(locale: Locale) -> Output {
    if crate::tui_capabilities::Capability::SavedMachines
        .require()
        .is_err()
    {
        return cli::unsupported_platform(None, locale);
    }
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(v) => v,
        Err(_) => return encoded(Reply::failure(Outcome::NotSubmitted)),
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let reply = match source::acquire_document(None, REQUEST_LIMIT, deadline, &mut signals) {
        Ok(bytes) => serve_packet(&bytes, || signals.take().map(|v| v.is_some())),
        Err(_) => Reply::failure(Outcome::NotSubmitted),
    };
    let _ = locale; // Packet locale belongs to the spawned business CLI.
    let mut result = encoded(reply);
    result.finalization = Some(signals.into_finalization());
    result
}
fn encoded(reply: Reply) -> Output {
    // Raw streams are bounded before base64; the resulting envelope fits 24 MiB.
    match serde_json::to_vec(&reply) {
        Ok(mut stdout) if stdout.len() <= REPLY_LIMIT => {
            stdout.push(b'\n');
            Output {
                status: 0,
                stdout,
                stderr: String::new(),
                finalization: None,
            }
        }
        _ => Output {
            status: 1,
            stdout: Vec::new(),
            stderr: String::new(),
            finalization: None,
        },
    }
}
/// An admitted packet: a routable request to run, or arguments this build's grammar refuses.
enum Admission {
    Run(Packet, Vec<u8>),
    /// The ordinary usage failure for the routed arguments, rendered without running anything.
    Usage(Output),
}
/// The locale of an admitted packet. A language this build does not ship, including one a later
/// sender adds, reads as en-US rather than refusing the command.
fn packet_locale(locale: &str) -> Locale {
    if locale == "zh_CN.UTF-8" {
        Locale::ZhCn
    } else {
        Locale::EnUs
    }
}
/// The packet spelling of a locale, which the routed command also runs under.
const fn locale_name(locale: Locale) -> &'static str {
    match locale {
        Locale::EnUs => "en_US.UTF-8",
        Locale::ZhCn => "zh_CN.UTF-8",
    }
}
/// Admits one packet of a generation this build reads; `extensions` content is ignored. The
/// routed arguments are parsed under this build's own grammar: a parse failure is answered as the
/// usual usage failure, while a parsed request that is not routable is refused.
fn admit_packet(bytes: &[u8]) -> Option<Admission> {
    if bytes.len() > REQUEST_LIMIT {
        return None;
    }
    let packet: Packet = serde_json::from_slice(bytes).ok()?;
    if !supported(packet.version)
        || packet.arguments.len() > 1024
        || packet.arguments.iter().map(String::len).sum::<usize>() > ARGUMENT_LIMIT
        || SessionName::parse(&packet.session).is_err()
    {
        return None;
    }
    let parsed = match invocation::parse_selected_request(packet.arguments.iter().map(Into::into)) {
        Ok(parsed) => parsed,
        Err(failure) => {
            return Some(Admission::Usage(cli::parse_failure(
                failure,
                packet_locale(&packet.locale),
            )));
        }
    };
    if parsed.session.is_some() {
        return None;
    }
    let (_, source) = invocation::machine_routing::admit(&parsed.request)?;
    let input = match (source, &packet.input) {
        (None, None) => Vec::new(),
        (Some(TerminalSendInput::Stdin { paste, .. }), Some(value)) => {
            let bytes = STANDARD.decode(value).ok()?;
            let limit = if paste {
                invocation::MAXIMUM_SEND_PASTE_BYTES
            } else {
                invocation::MAXIMUM_SEND_BYTES_BYTES
            };
            if bytes.is_empty() || bytes.len() > limit {
                return None;
            }
            bytes
        }
        _ => return None,
    };
    Some(Admission::Run(packet, input))
}
/// One request packet answered as the `machine-link` `route` channel carries it: the reply
/// packet's JSON without the line end of `--machine-relay-v1`, at most 24 MiB. `cancelled` is
/// polled while the command runs and ends it through its process tree when it turns true.
#[cfg(windows)]
pub(crate) fn answer_packet(bytes: &[u8], mut cancelled: impl FnMut() -> bool) -> Vec<u8> {
    let reply = serve_packet(bytes, || Ok(cancelled()));
    let mut packet = encoded(reply).stdout;
    if packet.pop() != Some(b'\n') {
        packet =
            serde_json::to_vec(&Reply::failure(Outcome::Unknown)).expect("a failure reply encodes");
    }
    packet
}
fn serve_packet(bytes: &[u8], mut interrupted: impl FnMut() -> io::Result<bool>) -> Reply {
    let (packet, input) = match admit_packet(bytes) {
        Some(Admission::Run(packet, input)) => (packet, input),
        Some(Admission::Usage(usage)) => {
            return Reply::completed(usage.status, &usage.stdout, usage.stderr.as_bytes());
        }
        None => return Reply::failure(Outcome::NotSubmitted),
    };
    let Ok(executable) = std::env::current_exe() else {
        return Reply::failure(Outcome::NotSubmitted);
    };
    let mut command = Command::new(executable);
    command
        .args(["--session", &packet.session])
        .args(&packet.arguments)
        .env("LC_ALL", locale_name(packet_locale(&packet.locale)))
        .env_remove("ORCHBEE_TERMINAL_ID")
        .env_remove("ORCHBEE_WORKER_ID");
    let result = match platform::exchange(
        command,
        Some(&input),
        cli::output::LIMIT,
        None,
        &mut interrupted,
    ) {
        Ok(result) => result,
        Err(_) => return Reply::failure(Outcome::NotSubmitted),
    };
    if result.stdout_truncated || result.stderr_truncated {
        return Reply::failure(Outcome::Unknown);
    }
    if (result.failure.is_none() || result.failure == Some(CommandFailure::ExitFailure))
        && let Some(status) = result.exit_code.and_then(|code| u8::try_from(code).ok())
    {
        return Reply::completed(status, &result.stdout, &result.stderr);
    }
    Reply::failure(
        if matches!(
            result.failure,
            Some(CommandFailure::SpawnFailed | CommandFailure::SpawnUnavailable)
        ) {
            Outcome::NotSubmitted
        } else {
            Outcome::Unknown
        },
    )
}

#[cfg(test)]
mod tests;
