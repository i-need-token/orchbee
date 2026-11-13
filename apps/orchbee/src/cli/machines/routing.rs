//! One routed native CLI command: the near side sends its packet through the machine hub, which
//! carries the Machine; the far side answers it. A lost connection never authorizes a retry.
use super::{Context, Error};
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
#[cfg(windows)]
use orchbee_local_runtime::CommandFailure;
#[cfg(any(windows, test))]
use orchbee_local_runtime::SessionName;
use orchbee_machine_protocol::hub::StopSessionOutcome;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
#[cfg(windows)]
use std::{io, process::Command};
#[cfg(windows)]
#[path = "routing/hub.rs"]
mod hub;
/// No machine hub runs where it is not built.
#[cfg(not(windows))]
mod hub {
    use super::{Context, Error, Outcome, Refusal, mapped};

    pub(super) fn route(
        _machine: &str,
        _packet: &[u8],
        _limit: usize,
        _context: &mut Context,
        _outcome: &mut Outcome,
    ) -> Result<Vec<u8>, Refusal> {
        Err(mapped(Error::SshUnavailable))
    }
}
/// The far side answers a routed command only where the link helper runs.
#[cfg(windows)]
#[path = "routing/windows.rs"]
mod platform;
pub(crate) mod stop;

const REQUEST_LIMIT: usize = 2 * 1024 * 1024;
const REPLY_LIMIT: usize = 24 * 1024 * 1024;
#[cfg(any(windows, test))]
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
    #[cfg(windows)]
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
    #[cfg(windows)]
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
fn select<'a>(catalog: &'a Catalog, selector: &str) -> Result<&'a Profile, Error> {
    let profile = catalog.resolve(selector).map_err(|error| match error {
        CatalogError::Ambiguous => Error::Catalog(CatalogError::Ambiguous),
        _ => Error::Catalog(CatalogError::NotFound),
    })?;
    if !profile.enabled {
        return Err(Error::DisabledMachine);
    }
    if !profile.draft.connection.supported() {
        return Err(Error::UnsupportedMachine);
    }
    Ok(profile)
}
/// Why a routed command failed: its closed reason and the failure kind it has while nothing was
/// submitted. [`reported_kind`] then applies the remote outcome, and the exit code follows from
/// the reported kind alone.
type Refusal = (&'static str, FailureKind);
/// The local input could not be acquired, or the packet carrying it is over the request bound.
const INPUT_SOURCE: Refusal = ("input-source", FailureKind::Rejected);
/// The far side's reply names no completed command.
const REMOTE_REJECTED: Refusal = (
    "remote-rejected-or-interrupted",
    FailureKind::ProtocolInvalid,
);
fn mapped(error: Error) -> Refusal {
    (error.code(), kind(error))
}
/// The failure kind of a machine error that ends a routed command before anything was submitted.
/// A hub `cancelled`, the end of the attempt the command waited for, and an install the hub did not
/// finish leave the Machine unable to serve the command now.
fn kind(error: Error) -> FailureKind {
    match error {
        Error::Catalog(
            CatalogError::Configuration | CatalogError::InvalidCatalog | CatalogError::Storage,
        ) => FailureKind::CheckFailed,
        Error::Catalog(CatalogError::NotFound) => FailureKind::NotFound,
        Error::Catalog(CatalogError::Ambiguous) => FailureKind::Ambiguous,
        Error::DisabledMachine => FailureKind::Rejected,
        Error::AuthFailed | Error::HostUntrusted | Error::HostKeyChanged => FailureKind::Denied,
        Error::Catalog(
            CatalogError::Busy | CatalogError::Capacity | CatalogError::RevisionExhausted,
        )
        | Error::SshUnavailable
        | Error::RemoteNotInstalled
        | Error::RemoteNotReady
        | Error::UnsupportedMachine
        | Error::HubUnavailable
        | Error::HostUnreachable
        | Error::AuthRequired
        | Error::AttemptEnded
        | Error::RemoteInstallFailed
        | Error::NotYetAvailable => FailureKind::ServiceUnavailable,
        Error::SshFailed => FailureKind::TransportLoss,
        Error::RemoteProtocol => FailureKind::ProtocolInvalid,
        Error::RemoteUnsupported | Error::RemoteOtherEnvironment | Error::VersionIncompatible => {
            FailureKind::Incompatible
        }
        Error::Timeout => FailureKind::Timeout,
        Error::PhysicalUnknown => FailureKind::OutcomeUnknown,
        Error::Cancelled => FailureKind::Cancelled,
        Error::Internal => FailureKind::InternalFailure,
    }
}
/// The exit code of `--machine <selector> server stop` for the hub's `stop-session` outcome, from
/// the outcome alone. A Session that was not running exits as a local `server stop` that finds
/// none; the hub's `failed` may follow a stop the Session accepted and partly carried out, so it
/// exits as a failed result rather than as a refusal that applied nothing.
fn stop_status(outcome: StopSessionOutcome) -> u8 {
    match outcome {
        StopSessionOutcome::Stopped => 0,
        StopSessionOutcome::NotRunning
        | StopSessionOutcome::NotOnline
        | StopSessionOutcome::Stopping => FailureKind::ServiceUnavailable.exit_code(),
        StopSessionOutcome::NotFound => FailureKind::NotFound.exit_code(),
        StopSessionOutcome::Unknown => FailureKind::OutcomeUnknown.exit_code(),
        StopSessionOutcome::Failed => 9,
    }
}
/// The kind a routed failure reports. An interrupt and an elapsed deadline keep theirs; any other
/// failure once the packet may have reached the far side is an unknown outcome, never a kind whose
/// exit code states that nothing ran.
fn reported_kind(kind: FailureKind, outcome: Outcome) -> FailureKind {
    match (kind, outcome) {
        (FailureKind::Cancelled | FailureKind::Timeout, _)
        | (_, Outcome::NotSubmitted | Outcome::Completed) => kind,
        (_, Outcome::Unknown) => FailureKind::OutcomeUnknown,
    }
}
fn failed((code, kind): Refusal, outcome: Outcome, format: OutputFormat, locale: Locale) -> Output {
    let kind = reported_kind(kind, outcome);
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
            | "auth-required"
            | "host-untrusted"
            | "host-key-changed"
            | "version-incompatible"
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
        Err(_) => {
            return failed(
                mapped(Error::Internal),
                Outcome::NotSubmitted,
                route.format,
                locale,
            );
        }
    };
    let mut context = Context {
        signals,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    let mut outcome = Outcome::NotSubmitted;
    let result = execute(&route, locale, &mut context, &mut outcome);
    let mut output = match result {
        Ok(output) => output,
        Err(refusal) => failed(refusal, outcome, route.format, locale),
    };
    // Preserve a completed remote receipt even if cancellation arrives afterwards.
    output.finalization = Some(context.signals.into_finalization());
    output
}
fn execute(
    route: &Route,
    locale: Locale,
    context: &mut Context,
    outcome: &mut Outcome,
) -> Result<Output, Refusal> {
    let catalog = Store::configured()
        .and_then(|s| s.read())
        .map_err(|e| mapped(Error::Catalog(e)))?;
    let profile = select(&catalog, &route.selector).map_err(mapped)?;
    // Only an SSH machine is carried; the hub would refuse another kind after the input was read.
    if profile.draft.ssh_target().is_none() {
        return Err(mapped(Error::UnsupportedMachine));
    }
    let mut arguments = route.arguments.clone();
    let input = if let Some(input) = &route.input {
        let (path, paste) = match input {
            TerminalSendInput::File { path, paste, .. } => (Some(path.as_str()), *paste),
            TerminalSendInput::Stdin { paste, .. } => (None, *paste),
            _ => return Err(mapped(Error::Internal)),
        };
        let limit = if paste {
            invocation::MAXIMUM_SEND_PASTE_BYTES
        } else {
            invocation::MAXIMUM_SEND_BYTES_BYTES
        };
        let bytes = source::acquire_document(path, limit, context.deadline, &mut context.signals)
            .map_err(|error| match error {
            source::SourceFailure::Cancelled { .. } => mapped(Error::Cancelled),
            source::SourceFailure::Timeout { .. } => mapped(Error::Timeout),
            source::SourceFailure::Standard(_) => INPUT_SOURCE,
        })?;
        if path.is_some() {
            let index = arguments
                .iter()
                .position(|a| a == "--file" || a.starts_with("--file="))
                .ok_or_else(|| mapped(Error::Internal))?;
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
    let packet = Packet {
        version: generation(),
        session: profile.draft.session.as_str().into(),
        arguments,
        input,
        locale: locale_name(locale).into(),
        extensions: Extensions::new(),
    };
    let bytes = serde_json::to_vec(&packet).map_err(|_| mapped(Error::Internal))?;
    if bytes.len() > REQUEST_LIMIT {
        return Err(INPUT_SOURCE);
    }
    let reply = hub::route(profile.id.as_str(), &bytes, REPLY_LIMIT, context, outcome)?;
    let reply: Reply = serde_json::from_slice(&reply).map_err(|_| mapped(Error::RemoteProtocol))?;
    match reply.output() {
        Ok(output) => {
            *outcome = Outcome::Completed;
            Ok(output)
        }
        Err(known) => {
            *outcome = known;
            Err(REMOTE_REJECTED)
        }
    }
}
#[cfg(windows)]
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
#[cfg(any(windows, test))]
enum Admission {
    Run(Packet, Vec<u8>),
    /// The ordinary usage failure for the routed arguments, rendered without running anything.
    Usage(Output),
}
/// The locale of an admitted packet. A language this build does not ship, including one a later
/// sender adds, reads as en-US rather than refusing the command.
#[cfg(any(windows, test))]
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
#[cfg(any(windows, test))]
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
/// packet's JSON, at most 24 MiB. `cancelled` is
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
#[cfg(windows)]
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
        .env_remove("ORCHBEE_WORKER_ID")
        // A far end that a development stand-in starts inherits the hub's askpass pairing, which a
        // far end over `ssh` never has; the command it runs must not become the askpass helper.
        .env_remove(crate::cli::machines::askpass::VARIABLE);
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
