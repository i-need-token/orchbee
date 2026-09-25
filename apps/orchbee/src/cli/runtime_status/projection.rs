//! Pure projection of independently observed process and local cohort facts.
use super::*;
use crate::cli::{config, escape::escape, output};
use orchbee_local_client_protocol::{
    SERVER_FEATURE_AGENT_OBSERVATION, SERVER_FEATURE_CLIENT_VIEW, SERVER_FEATURE_LIVE_HANDOFF,
    SERVER_FEATURE_RENDER_ROWS, SERVER_KNOWN_FEATURES, ServerMetadata, ServerMode,
};
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
pub(super) enum Cohort {
    Available(String),
    Unavailable(CohortIssue),
}
impl Cohort {
    fn json(&self) -> Value {
        match self {
            Self::Available(id) => json!({"state":"available","identity":id}),
            Self::Unavailable(reason) => json!({"state":"unavailable","reason":reason.token()}),
        }
    }
}
pub(super) enum Absence {
    Runtime,
    Endpoint,
}
pub(super) enum Server {
    Observed(ServerMetadata),
    Absent(Absence),
}
pub(super) struct ServerReport {
    pub(super) scope: String,
    pub(super) runtime: PathBuf,
    pub(super) endpoint: Option<PathBuf>,
    pub(super) server: Server,
    pub(super) cohort: Cohort,
}
pub(super) enum Report {
    Client { executable: PathBuf, cohort: Cohort },
    Server(ServerReport),
}
fn path(value: &Path) -> Value {
    Value::Object(serde_json::Map::from_iter([config::path_detail(value)]))
}
fn protocols(terminal: u8) -> Value {
    json!({"admission":orchbee_local_client_protocol::PROTOCOL_VERSION,"terminal":terminal})
}
fn features(value: u32, mode: ServerMode) -> Value {
    json!({"terminalRenderRows":value&SERVER_FEATURE_RENDER_ROWS!=0,
        "clientViewControl":value&SERVER_FEATURE_CLIENT_VIEW!=0,
        "agentObservation":value&SERVER_FEATURE_AGENT_OBSERVATION!=0,
        "liveHandoff":value&SERVER_FEATURE_LIVE_HANDOFF!=0,
        "managedStop":mode!=ServerMode::Unmanaged,
        "unknownBits":value & !SERVER_KNOWN_FEATURES})
}
impl Report {
    pub(super) fn document(&self) -> Value {
        let (schema, result) = match self {
            Self::Client { executable, cohort } => (
                "orchbee.status.client",
                json!({
                    "version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,
                    "buildProfile":if cfg!(debug_assertions) {"debug"} else {"release"},
                    "delivery":"development-cohort","updateChannel":null,"executable":path(executable),
                    "protocols":protocols(orchbee_client_server_protocol::PROTOCOL_VERSION),"cohort":cohort.json(),
                }),
            ),
            Self::Server(report) => {
                let (server, protocol, cohort) = match &report.server {
                    Server::Absent(reason) => (
                        json!({"state":"not-running","reason":match reason {Absence::Runtime=>"runtime-absent",Absence::Endpoint=>"endpoint-absent"}}),
                        None,
                        None,
                    ),
                    Server::Observed(info) => {
                        let mode = info.mode();
                        let cohort = match &report.cohort {
                            Cohort::Available(id) => Some(id == info.cohort()),
                            Cohort::Unavailable(_) => None,
                        };
                        (
                            json!({"state":"running","version":info.version(),"processId":info.process_id().get(),"cohort":info.cohort(),
                            "protocols":protocols(info.terminal_protocol().get()),"mode":if mode==ServerMode::Unmanaged {"unmanaged"}else{"managed"},
                            "stopping":mode==ServerMode::ManagedStopping,"capabilities":features(info.features(),mode)}),
                            Some(
                                info.terminal_protocol().get()
                                    == orchbee_client_server_protocol::PROTOCOL_VERSION,
                            ),
                            cohort,
                        )
                    }
                };
                let compatible = if protocol == Some(false) || cohort == Some(false) {
                    Some(false)
                } else if protocol == Some(true) && cohort == Some(true) {
                    Some(true)
                } else {
                    None
                };
                (
                    "orchbee.status.server",
                    json!({"scope":report.scope,"runtimeDirectory":path(&report.runtime),
                    "endpoint":report.endpoint.as_deref().map(path),"server":server,
                    "clientVersion":env!("CARGO_PKG_VERSION"),"clientProtocols":protocols(orchbee_client_server_protocol::PROTOCOL_VERSION),
                    "localCohort":report.cohort.json(),"compatibility":{"admissionProtocol":protocol.map(|_|true),"terminalProtocol":protocol,"cohort":cohort,
                        "compatible":compatible,"restartRequired":compatible.map(|value|!value)}}),
                )
            }
        };
        json!({"schema":schema,"version":1,"source":config::local_runtime_source(),"result":result})
    }
}

pub(super) fn emit(report: Report, format: OutputFormat, locale: Locale) -> Output {
    let document = report.document();
    let bytes = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => output::render_text(|out| {
            let value = &document["result"];
            let entries = match &report {
                Report::Client { .. } => vec![
                    ("client", &value["version"]),
                    ("platform", &value["platform"]),
                    ("architecture", &value["architecture"]),
                    ("profile", &value["buildProfile"]),
                    ("delivery", &value["delivery"]),
                    ("channel", &value["updateChannel"]),
                    ("executable", &value["executable"]),
                    ("protocols", &value["protocols"]),
                    ("local-cohort", &value["cohort"]),
                ],
                Report::Server(_) => vec![
                    ("server", &value["server"]["state"]),
                    ("version", &value["server"]["version"]),
                    ("process", &value["server"]["processId"]),
                    ("mode", &value["server"]["mode"]),
                    ("stopping", &value["server"]["stopping"]),
                    ("scope", &value["scope"]),
                    ("endpoint", &value["endpoint"]),
                    ("protocols", &value["server"]["protocols"]),
                    ("server-cohort", &value["server"]["cohort"]),
                    ("local-cohort", &value["localCohort"]),
                    ("compatibility", &value["compatibility"]),
                    ("capabilities", &value["server"]["capabilities"]),
                ],
            };
            for (label, value) in entries {
                let text = display(label, value, locale);
                out.write(format_args!(
                    "{}: {}\n",
                    locale.cli_runtime_status_label(label),
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
        Value::Null => locale.cli_runtime_status_unknown().to_owned(),
        Value::Bool(value) => locale
            .cli_runtime_status_value(if *value { "true" } else { "false" })
            .to_owned(),
        Value::String(value) => escape(
            if matches!(
                label,
                "server" | "state" | "reason" | "mode" | "profile" | "delivery"
            ) {
                locale.cli_runtime_status_value(value)
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
                        locale.cli_runtime_status_label(name),
                        display(name, value, locale)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        }
        _ => escape(&value.to_string()),
    }
}
