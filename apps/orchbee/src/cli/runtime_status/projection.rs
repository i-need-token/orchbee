//! Pure projection of the Session's process facts and this executable's local facts.
use super::{Failure, FailureKind, Locale, Output, OutputFormat, failed};
use crate::cli::output;
use crate::cli_text::CliText;
use crate::cli_text::escape::escape;
use orchbee_connection_handshake::{ProcessEntry, ProcessRole, QueryResult};
use orchbee_local_runtime::{CohortArtifact, CohortError};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) enum CohortIssue {
    Missing,
    Invalid,
    Denied,
    TimedOut,
    Io,
}
impl CohortIssue {
    pub(super) fn token(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Denied => "denied",
            Self::TimedOut => "timed-out",
            Self::Io => "io-error",
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct CohortDiagnostic {
    io_kind: CohortIoKind,
    os_error: Option<i32>,
    missing_artifact: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CohortIoKind {
    NotFound,
    PermissionDenied,
    InvalidInput,
    InvalidData,
    TimedOut,
    Unsupported,
    Interrupted,
    UnexpectedEof,
    WouldBlock,
    OutOfMemory,
    Other,
}
impl CohortIoKind {
    fn from_error_kind(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::NotFound => Self::NotFound,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::InvalidInput => Self::InvalidInput,
            std::io::ErrorKind::InvalidData => Self::InvalidData,
            std::io::ErrorKind::TimedOut => Self::TimedOut,
            std::io::ErrorKind::Unsupported => Self::Unsupported,
            std::io::ErrorKind::Interrupted => Self::Interrupted,
            std::io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            std::io::ErrorKind::WouldBlock => Self::WouldBlock,
            std::io::ErrorKind::OutOfMemory => Self::OutOfMemory,
            _ => Self::Other,
        }
    }

    const fn token(self) -> &'static str {
        match self {
            Self::NotFound => "not-found",
            Self::PermissionDenied => "permission-denied",
            Self::InvalidInput => "invalid-input",
            Self::InvalidData => "invalid-data",
            Self::TimedOut => "timed-out",
            Self::Unsupported => "unsupported",
            Self::Interrupted => "interrupted",
            Self::UnexpectedEof => "unexpected-eof",
            Self::WouldBlock => "would-block",
            Self::OutOfMemory => "out-of-memory",
            Self::Other => "other",
        }
    }
}

impl CohortDiagnostic {
    pub(super) fn from_error(error: &CohortError) -> Self {
        Self {
            io_kind: CohortIoKind::from_error_kind(error.kind()),
            os_error: error.raw_os_error(),
            missing_artifact: error.missing_artifact().map(CohortArtifact::file_name),
        }
    }

    fn json(&self) -> Value {
        let mut value = json!({"ioKind":self.io_kind.token(),"osError":self.os_error});
        if let Some(artifact) = &self.missing_artifact {
            value["missingArtifact"] = json!(artifact);
        }
        value
    }
}
pub(super) enum Cohort {
    Available(String),
    Unavailable(CohortIssue, Option<CohortDiagnostic>),
}
impl Cohort {
    fn json(&self, include_diagnostic: bool) -> Value {
        match self {
            Self::Available(id) => json!({"state":"available","identity":id}),
            Self::Unavailable(reason, diagnostic) => {
                let mut value = json!({"state":"unavailable","reason":reason.token()});
                if include_diagnostic && let Some(diagnostic) = diagnostic {
                    value["diagnostic"] = diagnostic.json();
                }
                value
            }
        }
    }
}
pub(super) enum Absence {
    Runtime,
    Endpoint,
}
pub(super) enum Server {
    Observed(QueryResult),
    Absent(Absence),
}
pub(super) struct ServerReport {
    pub(super) scope: String,
    pub(super) runtime: PathBuf,
    pub(super) endpoint: Option<PathBuf>,
    pub(super) server: Server,
}
pub(super) enum Report {
    Client {
        executable: PathBuf,
        cohort: Cohort,
        installation: crate::cli::installation::Installation,
    },
    Server(ServerReport),
}
fn path(value: &Path) -> Value {
    Value::Object(serde_json::Map::from_iter([crate::native_path::detail(
        value,
    )]))
}
fn protocols(terminal: u8) -> Value {
    json!({"admission":orchbee_local_client_protocol::PROTOCOL_VERSION,"terminal":terminal})
}
/// One reported process; its role is the text label, so the text form leaves it out.
fn process(entry: &ProcessEntry, with_role: bool) -> Value {
    let mut value = json!({"version":entry.product_version.as_str(),"processId":entry.pid.get(),
        "state":entry.state.as_str(),"terminals":entry.terminals});
    if with_role {
        value["role"] = json!(entry.role.as_str());
    }
    value
}
/// The text label naming a process role.
fn role_label(role: ProcessRole) -> &'static str {
    match role {
        ProcessRole::Server => "role-server",
        ProcessRole::Worker => "role-worker",
        ProcessRole::TerminalHost => "role-terminal-host",
        ProcessRole::Unknown => "role-unknown",
    }
}
impl Report {
    pub(super) fn document(&self) -> Value {
        let (schema, result) = match self {
            Self::Client {
                executable,
                cohort,
                installation,
            } => (
                "orchbee.status.client",
                json!({
                    "version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,
                    "buildProfile":if cfg!(debug_assertions) {"debug"} else {"release"},
                    "delivery":installation.delivery(),"updateChannel":installation.channel(),"installation":installation.json(),"executable":path(executable),
                    "protocols":protocols(orchbee_client_server_protocol::PROTOCOL_VERSION),"cohort":cohort.json(true),
                }),
            ),
            Self::Server(report) => {
                let server = match &report.server {
                    Server::Absent(reason) => json!({"state":"not-running","reason":match reason {
                        Absence::Runtime => "runtime-absent",
                        Absence::Endpoint => "endpoint-absent",
                    }}),
                    Server::Observed(result) => json!({"state":"running",
                        "restartKeepsTerminals":result.restart_keeps_terminals(),
                        "processes":result.processes().iter().map(|entry| process(entry, true)).collect::<Vec<_>>()}),
                };
                (
                    "orchbee.status.server",
                    json!({"scope":report.scope,"runtimeDirectory":path(&report.runtime),
                    "endpoint":report.endpoint.as_deref().map(path),"server":server,
                    "clientVersion":crate::local_client::CLIENT_VERSION}),
                )
            }
        };
        json!({"schema":schema,"version":1,"source":crate::cli_outcome::local_runtime_source(),"result":result})
    }
}

pub(super) fn emit(report: Report, format: OutputFormat, locale: Locale) -> Output {
    let document = report.document();
    let bytes = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => output::render_text(|out| {
            let value = &document["result"];
            let client_cohort_text = match &report {
                Report::Client { cohort, .. } => Some(cohort.json(false)),
                Report::Server(_) => None,
            };
            // Each process is one line labelled by its role.
            let processes = match &report {
                Report::Server(ServerReport {
                    server: Server::Observed(result),
                    ..
                }) => result
                    .processes()
                    .iter()
                    .map(|entry| (role_label(entry.role), process(entry, false)))
                    .collect(),
                _ => Vec::new(),
            };
            let entries = match &report {
                Report::Client { .. } => vec![
                    ("client", &value["version"]),
                    ("platform", &value["platform"]),
                    ("architecture", &value["architecture"]),
                    ("profile", &value["buildProfile"]),
                    ("delivery", &value["delivery"]),
                    ("channel", &value["updateChannel"]),
                    ("installation", &value["installation"]),
                    ("executable", &value["executable"]),
                    ("protocols", &value["protocols"]),
                    (
                        "local-cohort",
                        client_cohort_text
                            .as_ref()
                            .expect("client text cohort projection"),
                    ),
                ],
                Report::Server(_) => {
                    let mut entries = vec![
                        ("server", &value["server"]["state"]),
                        ("scope", &value["scope"]),
                        ("endpoint", &value["endpoint"]),
                    ];
                    entries.extend(processes.iter().map(|(label, value)| (*label, value)));
                    if !processes.is_empty() {
                        entries.push((
                            "restartKeepsTerminals",
                            &value["server"]["restartKeepsTerminals"],
                        ));
                    }
                    entries.push(("client", &value["clientVersion"]));
                    entries
                }
            };
            for (label, value) in entries {
                let text = display(label, value, locale);
                out.write(format_args!(
                    "{}: {}\n",
                    CliText::new(locale).cli_runtime_status_label(label),
                    text
                ))?;
            }
            Ok(())
        }),
    };
    match bytes {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failed(
            Failure::new(FailureKind::OutputTooLarge).details(serde_json::Map::from_iter([(
                "limitBytes".into(),
                json!(output::LIMIT),
            )])),
            format,
            locale,
        ),
    }
}

fn display(label: &str, value: &Value, locale: Locale) -> String {
    match value {
        Value::Null => CliText::new(locale).cli_runtime_status_unknown().to_owned(),
        Value::Bool(value) => CliText::new(locale)
            .cli_runtime_status_value(if *value { "true" } else { "false" })
            .to_owned(),
        Value::String(value) => escape(
            if matches!(
                label,
                "server" | "state" | "reason" | "mode" | "profile" | "delivery"
            ) {
                CliText::new(locale).cli_runtime_status_value(value)
            } else {
                value
            },
        ),
        Value::Object(fields) => {
            if let Some(path) = fields.get("path") {
                return display("path", path, locale);
            }
            fields
                .iter()
                .map(|(name, value)| {
                    format!(
                        "{}: {}",
                        CliText::new(locale).cli_runtime_status_label(name),
                        display(name, value, locale)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        }
        _ => escape(&value.to_string()),
    }
}
