//! One-shot remote native CLI relay. SSH failure never authorizes a retry.
use super::{Context, Error, ssh};
use crate::{
    cli::{
        self, Failure, FailureKind, Output,
        source::{self, ArmedSignals},
    },
    invocation::{self, OutputFormat, TerminalSendInput, machine_routing::Route},
    localization::Locale,
    machines::{Catalog, Profile, Store},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use orchbee_local_runtime::{Cohort, CommandFailure, SessionName, run_windows_command_exchange};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

const REQUEST_LIMIT: usize = 2 * 1024 * 1024;
const REPLY_LIMIT: usize = 24 * 1024 * 1024;
const ARGUMENT_LIMIT: usize = 64 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Packet {
    version: u8,
    cohort: String,
    #[serde(rename = "session")]
    session: String,
    arguments: Vec<String>,
    input: Option<String>,
    locale: String,
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
    version: u8,
    outcome: Outcome,
    status: Option<u8>,
    stdout: String,
    stderr: String,
}
impl Reply {
    fn failure(outcome: Outcome) -> Self {
        Self {
            version: 1,
            outcome,
            status: None,
            stdout: String::new(),
            stderr: String::new(),
        }
    }
    fn output(self) -> Result<Output, Outcome> {
        if self.version != 1 {
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
fn select<'a>(catalog: &'a Catalog, selector: &str) -> Result<&'a Profile, &'static str> {
    let profile = if let Some(profile) = catalog.profiles.iter().find(|p| p.id.as_str() == selector)
    {
        profile
    } else {
        let mut matches = catalog
            .profiles
            .iter()
            .filter(|p| p.draft.label == selector);
        let first = matches.next().ok_or("not-found")?;
        if matches.next().is_some() {
            return Err("ambiguous-machine");
        }
        first
    };
    if !profile.enabled {
        return Err("disabled-machine");
    }
    Ok(profile)
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
        locale.cli_machine_route_failure(outcome == Outcome::Unknown),
        code,
        outcome.token()
    );
    result
}
pub(crate) fn run(route: Route, locale: Locale) -> Output {
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
    let profile = select(&catalog, &route.selector).map_err(|e| (e, 3))?;
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
    let ssh_path =
        PathBuf::from(winsafe::GetSystemDirectory().map_err(|_| ("ssh-unavailable", 4))?)
            .join("OpenSSH/ssh.exe");
    let probe = ssh::exchange(
        &ssh_path,
        &profile.draft.target,
        &ssh::program(None, profile.draft.session.as_str(), "client"),
        context,
    )
    .map_err(mapped)?;
    let client = ssh::client(&probe).map_err(mapped)?;
    let packet = Packet {
        version: 1,
        cohort: client.cohort,
        session: profile.draft.session.as_str().into(),
        arguments,
        input,
        locale: match locale {
            Locale::EnUs => "en_US.UTF-8",
            Locale::ZhCn => "zh_CN.UTF-8",
        }
        .into(),
    };
    let bytes = serde_json::to_vec(&packet).map_err(|_| ("internal", 1))?;
    if bytes.len() > REQUEST_LIMIT {
        return Err(("input-source", 3));
    }
    context.check().map_err(mapped)?;
    *outcome = Outcome::Unknown;
    let command = ssh::command(
        &ssh_path,
        &profile.draft.target,
        &relay_program(&client.executable),
    );
    let result = run_windows_command_exchange(command, Some(&bytes), REPLY_LIMIT, None, || {
        context.interrupted()
    });
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
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(v) => v,
        Err(_) => return encoded(Reply::failure(Outcome::NotSubmitted)),
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let reply = match source::acquire_document(None, REQUEST_LIMIT, deadline, &mut signals) {
        Ok(bytes) => serve_packet(&bytes, deadline, &mut signals),
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
fn admit_packet(bytes: &[u8]) -> Option<(Packet, Vec<u8>)> {
    if bytes.len() > REQUEST_LIMIT {
        return None;
    }
    let packet: Packet = serde_json::from_slice(bytes).ok()?;
    if packet.version != 1
        || packet.arguments.len() > 1024
        || packet.arguments.iter().map(String::len).sum::<usize>() > ARGUMENT_LIMIT
        || !matches!(packet.locale.as_str(), "en_US.UTF-8" | "zh_CN.UTF-8")
        || SessionName::parse(&packet.session).is_err()
        || packet.cohort.len() != 64
        || !packet
            .cohort
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let parsed =
        invocation::parse_selected_request(packet.arguments.iter().map(Into::into)).ok()?;
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
    Some((packet, input))
}
fn serve_packet(bytes: &[u8], deadline: Instant, signals: &mut ArmedSignals) -> Reply {
    let Some((packet, input)) = admit_packet(bytes) else {
        return Reply::failure(Outcome::NotSubmitted);
    };
    let Ok(cohort) = Cohort::current(deadline) else {
        return Reply::failure(Outcome::NotSubmitted);
    };
    if cohort.identity() != packet.cohort {
        return Reply::failure(Outcome::NotSubmitted);
    }
    let Ok(executable) = std::env::current_exe() else {
        return Reply::failure(Outcome::NotSubmitted);
    };
    let mut command = Command::new(executable);
    command
        .args(["--session", &packet.session])
        .args(&packet.arguments)
        .env("LC_ALL", &packet.locale)
        .env_remove("ORCHBEE_TERMINAL_ID")
        .env_remove("ORCHBEE_WORKER_ID");
    let result =
        run_windows_command_exchange(command, Some(&input), cli::output::LIMIT, None, || {
            use source::SignalWatch;
            signals.take().map(|v| v.is_some())
        });
    if result.stdout_truncated || result.stderr_truncated {
        return Reply::failure(Outcome::Unknown);
    }
    if (result.failure.is_none() || result.failure == Some(CommandFailure::ExitFailure))
        && let Some(status) = result.exit_code.and_then(|code| u8::try_from(code).ok())
    {
        return Reply {
            version: 1,
            outcome: Outcome::Completed,
            status: Some(status),
            stdout: STANDARD.encode(result.stdout),
            stderr: STANDARD.encode(result.stderr),
        };
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
