//! `diagnostics create`: one diagnostic bundle written where the person chooses (ORC-397). The
//! person sends it themselves; nothing is uploaded, and nothing is created in the data root.
//!
//! The bundle carries versions, system facts, `config check`, the Session list, integration
//! status, redacted settings and preferences, and the well-formed records of the last seven daily
//! logs. Every carried document passes through the local-runtime redaction rules; checkpoints,
//! layouts, terminal output, plugin data and saved machines are never read.
//!
//! Collection reads the cohort, the data root and the Session catalog and asks the running default
//! Session for integration status only when an integration may have been enabled; it never starts
//! a Session and creates nothing. The platform adapter names the system release, tells whether a
//! process runs a given program, and names the running Sessions where `session list` cannot.
//! Windows and macOS admit the command; Linux answers the typed unsupported-platform failure.
use super::execution::{Capability, admit};
use super::schema_export::{PublishFailure, publish};
use super::{Failure, FailureKind, Output, failure_output, output};
use super::{config, named_sessions};
use crate::cli_text::CliText;
use crate::invocation::{NamedSessionCommand, OutputFormat};
use crate::localization::Locale;
use orchbee_local_runtime::{
    Cohort, CohortArtifact, DataRoot, DiagnosticRedactor, LogValue, recent_diagnostic_logs,
    redact_settings,
};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
#[path = "diagnostics/windows.rs"]
mod platform;
#[cfg(unix)]
#[path = "diagnostics/unix.rs"]
mod platform;

const SCHEMA: &str = "orchbee.diagnostics.create";
const FAILURE_SCHEMA: &str = "orchbee.cli.failure.diagnostics";
const BUNDLE_SCHEMA: &str = "orchbee.diagnostics.bundle";
const INCLUDED: &str = "included";
const ABSENT: &str = "absent";
const UNAVAILABLE: &str = "unavailable";
/// Larger settings than this are not a file a person wrote by hand.
const MAXIMUM_SETTINGS_BYTES: u64 = 1024 * 1024;

/// The collected entries before the manifest is written, with each section's status token:
/// `included`, `absent` when the source does not exist, or `unavailable` when it exists but could
/// not be read or parsed, so nothing of it is carried.
#[derive(Default)]
struct Collected {
    sections: Vec<(&'static str, &'static str)>,
    entries: Vec<(String, Vec<u8>)>,
    omitted_log_lines: u64,
}

/// One Session as the bundle reports it: its name and state as listed, and its Server's process.
struct Running {
    name: Value,
    state: Value,
    pid: Option<u32>,
}

pub(crate) fn create(path: &str, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = admit(Capability::DiagnosticBundle) {
        return failure_output(failure, Some(format), locale);
    }
    let collected = collect(locale);
    let unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let manifest = json!({
        "schema": BUNDLE_SCHEMA,
        "version": 1,
        "createdAt": unix_seconds,
        "sections": collected.sections.iter().map(|(name, status)| {
            json!({"name": name, "status": status})
        }).collect::<Vec<_>>(),
        "omittedLogLines": collected.omitted_log_lines,
    });
    let mut entries = Vec::with_capacity(collected.entries.len() + 1);
    entries.push((
        "manifest.json".to_owned(),
        serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
    ));
    entries.extend(collected.entries);
    let Ok(archive) = orchbee_local_runtime::diagnostic_archive(&entries, unix_seconds) else {
        return failure_output(
            Failure::new(FailureKind::InternalFailure),
            Some(format),
            locale,
        );
    };
    match publish(std::path::Path::new(path), &archive, "diagnostics") {
        Ok(()) => success(
            path,
            &entries,
            archive.len(),
            &collected.sections,
            format,
            locale,
        ),
        Err(error) => failure(path, &error, format, locale),
    }
}

fn success(
    path: &str,
    entries: &[(String, Vec<u8>)],
    bytes: usize,
    sections: &[(&'static str, &'static str)],
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&json!({
            "schema": SCHEMA,
            "version": 1,
            "source": crate::cli_outcome::local_runtime_source(),
            "result": {
                "path": path,
                "bytes": bytes,
                "entries": entries.iter().map(|(name, _)| name).collect::<Vec<_>>(),
                "sections": sections.iter().map(|(name, status)| {
                    json!({"name": name, "status": status})
                }).collect::<Vec<_>>(),
            },
        })),
        OutputFormat::Text => output::encode_text(&CliText::new(locale).diagnostics_created(
            path,
            entries.len(),
            bytes,
        )),
    };
    match encoded {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}

fn failure(path: &str, error: &PublishFailure, format: OutputFormat, locale: Locale) -> Output {
    let document = json!({
        "schema": FAILURE_SCHEMA,
        "version": 1,
        "source": crate::cli_outcome::local_runtime_source(),
        "failure": {
            "kind": "write-failed",
            "details": {
                "stage": error.stage.as_str(),
                "path": path,
                "temporaryPath": error.residue.as_deref(),
            }
        }
    });
    let stdout = if format == OutputFormat::Json {
        match output::encode_document(&document) {
            Ok(bytes) => bytes,
            Err(_) => {
                return failure_output(
                    Failure::new(FailureKind::InternalFailure),
                    Some(format),
                    locale,
                );
            }
        }
    } else {
        Vec::new()
    };
    Output {
        status: 1,
        stdout,
        stderr: CliText::new(locale).diagnostics_failure(
            error.stage.as_str(),
            path,
            error.residue.as_deref(),
        ),
        finalization: None,
    }
}

fn collect(locale: Locale) -> Collected {
    let redactor = DiagnosticRedactor::from_process_environment();
    let environment = |name: &str| std::env::var_os(name);
    let root = DataRoot::resolve(environment).ok();
    let mut collected = Collected::default();

    let sessions = answered(
        &named_sessions::run(&NamedSessionCommand::List, OutputFormat::Json, locale),
        &redactor,
    );
    let running = listed(sessions.as_ref());
    collected.document("versions.json", Some(versions(running.as_deref())));
    collected.document("system.json", Some(system()));
    collected.document(
        "config-check.json",
        answered(&config::check(OutputFormat::Json, locale), &redactor),
    );
    collected.document("sessions.json", sessions);
    let settings = match orchbee_local_runtime::user_settings_path(environment) {
        Ok(Some(path)) => match read_bounded(&path, MAXIMUM_SETTINGS_BYTES) {
            Ok(Some(text)) => String::from_utf8(text)
                .ok()
                .and_then(|text| redact_settings(&text, &redactor))
                .ok_or(UNAVAILABLE),
            Ok(None) => Err(ABSENT),
            Err(_) => Err(UNAVAILABLE),
        },
        Ok(None) => Err(ABSENT),
        Err(_) => Err(UNAVAILABLE),
    };
    collected.file("config.toml", settings.map(String::into_bytes));

    match root.as_ref().map(recent_diagnostic_logs) {
        Some(Ok(recent)) => {
            collected.omitted_log_lines = recent.omitted_lines;
            let status = if recent.files.is_empty() {
                ABSENT
            } else {
                INCLUDED
            };
            for (name, bytes) in recent.files {
                collected.entries.push((format!("logs/{name}"), bytes));
            }
            collected.sections.push(("logs", status));
        }
        Some(Err(_)) | None => collected.sections.push(("logs", UNAVAILABLE)),
    }
    collected
}

impl Collected {
    fn document(&mut self, name: &'static str, document: Option<Value>) {
        self.file(
            name,
            document
                .and_then(|document| serde_json::to_vec_pretty(&document).ok())
                .ok_or(UNAVAILABLE),
        );
    }

    fn file(&mut self, name: &'static str, bytes: Result<Vec<u8>, &'static str>) {
        match bytes {
            Ok(bytes) => {
                self.entries.push((name.to_owned(), bytes));
                self.sections.push((name, INCLUDED));
            }
            Err(status) => self.sections.push((name, status)),
        }
    }
}

/// The Sessions a `session list` document names, or `None` when it names none.
fn listed(sessions: Option<&Value>) -> Option<Vec<Running>> {
    let sessions = sessions?.pointer("/result/sessions")?.as_array()?;
    Some(
        sessions
            .iter()
            .map(|session| Running {
                name: session["name"].clone(),
                state: session["state"].clone(),
                pid: session["pid"]
                    .as_u64()
                    .and_then(|pid| u32::try_from(pid).ok()),
            })
            .collect(),
    )
}

/// The machine document a command answered on standard output, scrubbed, or `None` when it
/// answered nothing parseable.
fn answered(output: &Output, redactor: &DiagnosticRedactor) -> Option<Value> {
    let mut document: Value = serde_json::from_slice(&output.stdout).ok()?;
    redactor.scrub_json(&mut document);
    Some(document)
}

/// This build's product and cohort, and for each Session whether its running Server is this
/// cohort's executable.
fn versions(running: Option<&[Running]>) -> Value {
    let cohort = Cohort::current(Instant::now() + Duration::from_secs(5));
    let build = match &cohort {
        Ok(cohort) => json!({
            "identity": cohort.identity(),
            "artifacts": {
                "client": cohort.artifact_sha256(CohortArtifact::Client),
                "server": cohort.artifact_sha256(CohortArtifact::Server),
                "worker": cohort.artifact_sha256(CohortArtifact::Worker),
                "terminalHost": cohort.artifact_sha256(CohortArtifact::TerminalHost),
            },
        }),
        Err(error) => json!({"unavailable": kind_token(error.kind())}),
    };
    let running = running.map(|sessions| {
        sessions
            .iter()
            .map(|session| {
                json!({
                    "session": session.name,
                    "state": session.state,
                    "pid": session.pid,
                    "server": session.pid.map_or("not-running", |pid| {
                        server_build(pid, cohort.as_ref().ok())
                    }),
                })
            })
            .collect::<Vec<_>>()
    });
    json!({
        "product": env!("CARGO_PKG_VERSION"),
        "environment": orchbee_local_runtime::environment_stem(),
        "release": orchbee_local_runtime::is_release(),
        "build": build,
        "sessions": running,
    })
}

/// `current` when the process runs this cohort's Server executable, `other` when it runs another
/// program, and `unknown` when it cannot be inspected.
fn server_build(pid: u32, cohort: Option<&Cohort>) -> &'static str {
    let Some(cohort) = cohort else {
        return "unknown";
    };
    match platform::runs(pid, &cohort.executable(CohortArtifact::Server)) {
        Some(true) => "current",
        Some(false) => "other",
        None => "unknown",
    }
}

/// The operating system, architecture, the platform's release, the selected locale and its source
/// variable, and the terminal as a closed token. No variable value is carried.
fn system() -> Value {
    let (locale, source) = crate::localization::locale_source();
    let mut system = serde_json::Map::new();
    system.insert("os".to_owned(), json!(std::env::consts::OS));
    system.insert("architecture".to_owned(), json!(std::env::consts::ARCH));
    if let Some((name, release)) = platform::release() {
        system.insert(name.to_owned(), release);
    }
    system.insert("locale".to_owned(), json!(locale));
    system.insert("localeSource".to_owned(), json!(source));
    system.insert("terminal".to_owned(), json!(terminal()));
    Value::Object(system)
}

fn terminal() -> &'static str {
    if std::env::var_os("WT_SESSION").is_some() {
        return "windows-terminal";
    }
    match std::env::var("TERM_PROGRAM").ok().as_deref() {
        Some("vscode") => "vscode",
        Some("WezTerm") => "wezterm",
        Some(_) => "other",
        None => "unknown",
    }
}

fn kind_token(kind: io::ErrorKind) -> &'static str {
    match LogValue::from(kind) {
        LogValue::Token(token) => token,
        LogValue::Count(_) | LogValue::Flag(_) => "other",
    }
}

/// A file's bytes up to `limit`, `None` when it does not exist, and an error when it is larger.
fn read_bounded(path: &Path, limit: u64) -> io::Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Some(bytes))
}
