//! Windows bundle collection. It reads the cohort, the data root and the Session catalog and asks
//! the running default Session for integration status only when an integration may have been
//! enabled; it never starts a Session and creates nothing.
use super::super::{Failure, Output, config, integration, named_sessions};
use super::Collected;
use crate::invocation::{AgentIntegrationCommand, NamedSessionCommand, OutputFormat};
use crate::localization::Locale;
use orchbee_local_runtime::{
    Cohort, CohortArtifact, DataRoot, DiagnosticRedactor, LogValue, StateRecord,
    recent_diagnostic_logs, redact_preferences, redact_settings,
};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
    time::{Duration, Instant},
};
use winsafe::{self as w, co};

const INCLUDED: &str = "included";
const ABSENT: &str = "absent";
const UNAVAILABLE: &str = "unavailable";
/// Larger settings than this are not a file a person wrote by hand.
const MAXIMUM_SETTINGS_BYTES: u64 = 1024 * 1024;

pub(super) fn collect(locale: Locale) -> Result<Collected, Failure> {
    let redactor = DiagnosticRedactor::from_process_environment();
    let environment = |name: &str| std::env::var_os(name);
    let root = DataRoot::resolve(environment).ok();
    let mut collected = Collected::default();

    let sessions = answered(
        &named_sessions::run(&NamedSessionCommand::List, OutputFormat::Json, locale),
        &redactor,
    );
    collected.document("versions.json", Some(versions(sessions.as_ref())));
    collected.document("system.json", Some(system()));
    collected.document(
        "config-check.json",
        answered(&config::check(OutputFormat::Json, locale), &redactor),
    );
    let default_running = sessions
        .as_ref()
        .and_then(|document| document.pointer("/result/sessions"))
        .and_then(Value::as_array)
        .is_some_and(|sessions| {
            sessions
                .iter()
                .any(|session| session["default"] == true && session["running"] == true)
        });
    collected.document("sessions.json", sessions);
    // Without any installed Capsule no provider entry names this environment: provider commands
    // and configuration stay untouched.
    let integrations = if !orchbee_local_runtime::agent_integration_may_have_been_enabled() {
        Some(json!({"state": "never-enabled"}))
    } else if default_running {
        answered(
            &integration::run(
                &AgentIntegrationCommand::List {
                    outdated_only: false,
                    provider: None,
                    adapter_source: None,
                },
                OutputFormat::Json,
                locale,
            ),
            &redactor,
        )
    } else {
        Some(json!({"state": "session-not-running"}))
    };
    collected.document("integrations.json", integrations);

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

    let preferences = match &root {
        Some(root) => match read_bounded(
            &root.state().join(StateRecord::Preferences.file_name()),
            orchbee_workbench_protocol::MAXIMUM_USER_PREFERENCES_BYTES as u64,
        ) {
            Ok(Some(bytes)) => redact_preferences(&bytes, &redactor)
                .and_then(|document| serde_json::to_vec_pretty(&document).ok())
                .ok_or(UNAVAILABLE),
            Ok(None) => Err(ABSENT),
            Err(_) => Err(UNAVAILABLE),
        },
        None => Err(UNAVAILABLE),
    };
    collected.file("preferences.json", preferences);

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
    Ok(collected)
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

/// The machine document a command answered on standard output, scrubbed, or `None` when it
/// answered nothing parseable.
fn answered(output: &Output, redactor: &DiagnosticRedactor) -> Option<Value> {
    let mut document: Value = serde_json::from_slice(&output.stdout).ok()?;
    redactor.scrub_json(&mut document);
    Some(document)
}

/// This build's product and cohort, and for each Session whether its running Server is this
/// cohort's executable.
fn versions(sessions: Option<&Value>) -> Value {
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
    let running = sessions
        .and_then(|document| document.pointer("/result/sessions"))
        .and_then(Value::as_array)
        .map(|sessions| {
            sessions
                .iter()
                .map(|session| {
                    let pid = session["pid"]
                        .as_u64()
                        .and_then(|pid| u32::try_from(pid).ok());
                    json!({
                        "session": session["name"],
                        "state": session["state"],
                        "pid": pid,
                        "server": pid.map_or("not-running", |pid| {
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
/// image, and `unknown` when it cannot be inspected.
fn server_build(pid: u32, cohort: Option<&Cohort>) -> &'static str {
    let Some(cohort) = cohort else {
        return "unknown";
    };
    let image = w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, pid)
        .and_then(|process| process.QueryFullProcessImageName(co::PROCESS_NAME::WIN32));
    match image {
        Ok(image) => {
            let expected = cohort.executable(CohortArtifact::Server);
            if expected
                .to_str()
                .is_some_and(|expected| expected.eq_ignore_ascii_case(&image))
            {
                "current"
            } else {
                "other"
            }
        }
        Err(_) => "unknown",
    }
}

/// The operating system, architecture, selected locale and its source variable, and the terminal
/// as a closed token. No variable value is carried.
fn system() -> Value {
    let windows = orchbee_local_runtime::read_windows_version();
    let (locale, source) = crate::localization::locale_source();
    json!({
        "os": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "windows": {
            "product": windows.product,
            "display": windows.display,
            "version": windows.version,
        },
        "locale": locale,
        "localeSource": source,
        "terminal": terminal(),
    })
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
