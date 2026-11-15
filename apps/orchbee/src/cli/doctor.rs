//! `doctor`: six fixed checks of whether Orchbee works here. Each check folds facts a collector
//! already gathers for another command into one status, one closed reason and those facts
//! unchanged. It never connects as a Client, starts nothing and writes nothing; each collector
//! keeps its own bound.

use serde_json::{Map, Value, json};
use std::time::{Duration, Instant};

use super::console_interrupt::{ArmedSignals, Finish};
use super::results::local_report::CheckReport;
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::cli_text::CliText;
use crate::cli_text::escape::{display_width, escape};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

/// The budget of each collector that has no bound of its own.
const COLLECTOR_BUDGET: Duration = Duration::from_secs(5);

/// One folded check: `pass`, `warning`, `fail` or `not-applicable`, a closed reason token for
/// any but a pass, and the collector's facts.
struct Check {
    status: &'static str,
    reason: Option<String>,
    details: Value,
}

impl Check {
    fn new(status: &'static str, reason: Option<&str>, details: Value) -> Self {
        Self {
            status,
            reason: reason.map(str::to_owned),
            details,
        }
    }
}

/// The checks in report order; `None` is a check this platform cannot make.
type Checks = [(&'static str, Option<Check>); 6];

pub(crate) fn run(format: OutputFormat, locale: Locale) -> Output {
    let failed = |failure: Failure| {
        failure_output(
            failure.source(crate::cli_outcome::local_runtime_source()),
            Some(format),
            locale,
        )
    };
    let Ok(signals) = ArmedSignals::arm_interrupts_or_inherit() else {
        return failed(Failure::new(FailureKind::InternalFailure));
    };
    let checks = collect(&signals);
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled { finalization }) => {
            let mut output = failed(Failure::new(FailureKind::Cancelled));
            output.finalization = Some(finalization);
            return output;
        }
        Err(_) => return failed(Failure::new(FailureKind::InternalFailure)),
    }
    match checks {
        Ok(checks) => emit(&checks, format, locale),
        Err(failure) => failed(failure),
    }
}

/// Runs the collectors in turn and stops early once an interrupt was requested; the caller then
/// reports the cancellation.
fn collect(signals: &ArmedSignals) -> Result<Checks, Failure> {
    let installation = installation()?;
    let settings = settings();
    let records = Some(records());
    let session = if signals.requested() {
        None
    } else {
        session(signals)
    };
    let session_answers = session
        .as_ref()
        .is_some_and(|check| check.status != "not-applicable");
    let runtime = runtime(session.is_some(), session_answers)?;
    let integrations = if signals.requested() {
        None
    } else {
        integrations()
    };
    Ok([
        ("installation", installation),
        ("settings", settings),
        ("records", records),
        ("runtime", runtime),
        ("session", session),
        ("integrations", integrations),
    ])
}

/// A collector the platform refused as unsupported has no check; any other failure is the
/// command's own.
fn supported<T>(result: Result<T, Failure>) -> Result<Option<T>, Failure> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(failure)
            if failure
                .details
                .as_ref()
                .and_then(|details| details.get("stage"))
                .and_then(Value::as_str)
                == Some("platform") =>
        {
            Ok(None)
        }
        Err(failure) => Err(failure),
    }
}

/// This executable and its installation files, as `status client` reads them.
fn installation() -> Result<Option<Check>, Failure> {
    if super::execution::admit(super::execution::Capability::RuntimeStatus).is_err() {
        return Ok(None);
    }
    let Some(details) = supported(super::runtime_status::client_facts(
        Instant::now() + COLLECTOR_BUDGET,
    ))?
    else {
        return Ok(None);
    };
    let cohort = &details["cohort"];
    Ok(Some(if cohort["state"] == "unavailable" {
        let reason = cohort["reason"].as_str().map(str::to_owned);
        Check {
            status: "fail",
            reason,
            details,
        }
    } else if details["delivery"] == "unknown" {
        Check::new("warning", Some("delivery-unknown"), details)
    } else {
        Check::new("pass", None, details)
    }))
}

/// The configuration file, as the `user-config` row of `config check` reads it.
fn settings() -> Option<Check> {
    let row = super::config::settings_check()?;
    let reason = match row.status {
        "fail" => row.details.get("reason").and_then(Value::as_str),
        "warning" => row.details["diagnostics"][0]["reason"].as_str(),
        _ => None,
    }
    .map(str::to_owned);
    Some(Check {
        status: row.status,
        reason,
        details: Value::Object(row.details),
    })
}

/// Records this version cannot use, as the `records` row of `config check` reads them.
fn records() -> Check {
    let row = super::config::records_check();
    let reason = row.details["records"][0]["problem"]
        .as_str()
        .filter(|_| row.status == "warning")
        .map(str::to_owned);
    Check {
        status: row.status,
        reason,
        details: Value::Object(row.details),
    }
}

/// The selected Session, as `status server` asks it with one handshake `query`.
fn session(signals: &ArmedSignals) -> Option<Check> {
    super::execution::admit(super::execution::Capability::RuntimeStatus).ok()?;
    match super::runtime_status::server_facts(Instant::now() + COLLECTOR_BUDGET, signals) {
        Ok(details) => {
            if details["server"]["state"] != "running" {
                return Some(Check::new("not-applicable", None, details));
            }
            let state = details["server"]["processes"]
                .as_array()
                .and_then(|processes| processes.iter().find(|process| process["role"] == "server"))
                .and_then(|process| process["state"].as_str())
                .filter(|state| matches!(*state, "draining" | "stopping"))
                .map(str::to_owned);
            Some(match state {
                Some(state) => Check::new("warning", Some(&state), details),
                None => Check::new("pass", None, details),
            })
        }
        Err(failure) => {
            let details = failure.details.unwrap_or_default();
            let stated = details.get("reason").and_then(Value::as_str);
            let check = |status, reason: &str| {
                Some(Check::new(
                    status,
                    Some(reason),
                    Value::Object(details.clone()),
                ))
            };
            match (failure.kind, stated) {
                (FailureKind::ServiceUnavailable, Some("stopping")) => check("warning", "stopping"),
                (FailureKind::Incompatible, Some(reason)) => check("fail", reason),
                (FailureKind::Timeout, _) => check("fail", "timed-out"),
                (kind, _) => check("fail", kind.as_str()),
            }
        }
    }
}

/// The runtime folder report with the `runtime-base`, `runtime-directory` and `sockets` rows of
/// `config check`: an unsafe or unreadable path fails, and an advisory warns only while no
/// Session answers.
fn runtime(session_known: bool, session_answers: bool) -> Result<Option<Check>, Failure> {
    if super::execution::admit(super::execution::Capability::RuntimeInspection).is_err() {
        return Ok(None);
    }
    let Some(folder) = supported(super::runtime_folder::collect())? else {
        return Ok(None);
    };
    let rows = super::config::runtime_checks()?;
    if let Some(reason) = rows.iter().find_map(failed_row_reason) {
        return Ok(Some(Check::new("fail", Some(&reason), folder.result)));
    }
    Ok(Some(match folder.advisories.first() {
        Some(advisory) if session_known && !session_answers => {
            Check::new("warning", Some(advisory), folder.result)
        }
        _ => Check::new("pass", None, folder.result),
    }))
}

/// The reason of a failed path row, or of the failed entry of the `sockets` row.
fn failed_row_reason(row: &CheckReport) -> Option<String> {
    if row.status != "fail" {
        return None;
    }
    let reason = row
        .details
        .get("reason")
        .and_then(Value::as_str)
        .or_else(|| {
            row.details
                .values()
                .find(|entry| entry["status"] == "fail")
                .and_then(|entry| entry["reason"].as_str())
        });
    Some(reason.unwrap_or("unsafe").to_owned())
}

/// Every provider, as `agent integration list` reads them. Integrations are opt-in, so only one
/// that needs an update warns.
fn integrations() -> Option<Check> {
    let Some(details) = super::integration::list_facts()? else {
        return Some(Check::new("warning", Some("home-unavailable"), Value::Null));
    };
    let outdated = details["integrations"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["state"] == "update-required"));
    Some(if outdated {
        Check::new("warning", Some("update-required"), details)
    } else {
        Check::new("pass", None, details)
    })
}

fn document(checks: &Checks) -> Value {
    let failed = checks
        .iter()
        .any(|(_, check)| check.as_ref().is_some_and(|check| check.status == "fail"));
    let members = checks
        .iter()
        .map(|(key, check)| {
            let value = check.as_ref().map_or_else(
                || json!({"kind":"unsupported"}),
                |check| {
                    json!({"status":check.status,"reason":check.reason,"details":check.details})
                },
            );
            ((*key).to_owned(), value)
        })
        .collect::<Map<_, _>>();
    json!({
        "schema": "orchbee.doctor",
        "version": 1,
        "source": crate::cli_outcome::local_runtime_source(),
        "result": {"ok": !failed, "checks": members},
    })
}

fn emit(checks: &Checks, format: OutputFormat, locale: Locale) -> Output {
    let failed = checks
        .iter()
        .any(|(_, check)| check.as_ref().is_some_and(|check| check.status == "fail"));
    let bytes = match format {
        OutputFormat::Json => output::encode_document(&document(checks)),
        OutputFormat::Text => output::render_text(|out| text(out, checks, locale)),
    };
    match bytes {
        Ok(stdout) => Output {
            status: u8::from(failed),
            stdout,
            stderr: if failed {
                CliText::new(locale).cli_doctor_failed().to_owned()
            } else {
                String::new()
            },
            finalization: None,
        },
        Err(output::TooLarge) => failure_output(
            Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                "limitBytes".to_owned(),
                json!(output::LIMIT),
            )])),
            Some(format),
            locale,
        ),
    }
}

/// The verdict, one line per check — status word, label, then one fact, or for a warning or a
/// failure the reason and an optional indented command — and the pointer to JSON.
fn text(
    out: &mut output::Bounded,
    checks: &Checks,
    locale: Locale,
) -> Result<(), output::TooLarge> {
    let copy = CliText::new(locale);
    let present = checks
        .iter()
        .filter_map(|(key, check)| check.as_ref().map(|check| (*key, check)))
        .collect::<Vec<_>>();
    let count = |status| {
        present
            .iter()
            .filter(|(_, check)| check.status == status)
            .count()
    };
    out.push(&copy.cli_doctor_verdict(present.len(), count("warning"), count("fail")))?;
    // Every status word fits the column, so a line's label starts where it would on any run.
    let status_width = ["pass", "warning", "fail", "not-applicable"]
        .into_iter()
        .map(|status| display_width(copy.cli_doctor_status(status)))
        .max()
        .unwrap_or(0)
        + 2;
    let label_width = present
        .iter()
        .map(|(key, _)| display_width(copy.cli_doctor_label(key)))
        .max()
        .unwrap_or(0)
        + 2;
    for (key, check) in present {
        let (fact, command) = line(key, check, locale);
        let status = copy.cli_doctor_status(check.status);
        let label = copy.cli_doctor_label(key);
        let mut row = format!(
            "{status}{}{label}",
            " ".repeat(status_width - display_width(status))
        );
        if !fact.is_empty() {
            row.push_str(&" ".repeat(label_width - display_width(label)));
            row.push_str(&fact);
        }
        out.write(format_args!("{row}\n"))?;
        if let Some(command) = command {
            out.write(format_args!("{}{command}\n", " ".repeat(status_width)))?;
        }
    }
    out.push(copy.cli_doctor_details_hint())
}

/// The one fact a line shows, or for a warning or a failure its reason, and the command to run.
fn line(key: &str, check: &Check, locale: Locale) -> (String, Option<String>) {
    let copy = CliText::new(locale);
    let details = &check.details;
    let reason = check.reason.as_deref().unwrap_or_default();
    let path = |value: &Value| {
        value["path"]
            .as_str()
            .map(crate::native_path::display)
            .unwrap_or_default()
    };
    let config_check = || Some("orchbee config check".to_owned());
    match (key, check.status) {
        ("installation", "fail") => (
            format!(
                "{}: {}",
                copy.cli_runtime_status_label("local-cohort"),
                copy.cli_runtime_status_value(reason)
            ),
            None,
        ),
        ("installation", "warning") => (copy.cli_doctor_delivery_unknown().to_owned(), None),
        ("installation", _) => {
            let release = details["updateChannel"].as_str().unwrap_or_else(|| {
                copy.cli_runtime_status_value(details["delivery"].as_str().unwrap_or_default())
            });
            (
                [
                    escape(details["version"].as_str().unwrap_or_default()),
                    release.to_owned(),
                    path(&details["executable"]),
                ]
                .join(" · "),
                None,
            )
        }
        ("settings", "fail") => {
            let syntax = details["line"].as_u64().zip(details["column"].as_u64());
            let fact = match syntax {
                Some((line, column)) => format!(
                    "{}:{line}:{column}: {}",
                    path(details),
                    copy.cli_configuration_syntax(reason, line)
                ),
                None => format!(
                    "{}: {reason}",
                    escape(details["field"].as_str().unwrap_or_default())
                ),
            };
            (fact, config_check())
        }
        ("settings", "warning") => {
            let diagnostic = &details["diagnostics"][0];
            (
                format!(
                    "{}: {reason} ({})",
                    escape(diagnostic["field"].as_str().unwrap_or_default()),
                    diagnostic["disposition"].as_str().unwrap_or_default()
                ),
                config_check(),
            )
        }
        ("settings", _) => (path(details), None),
        ("records", "warning") => {
            let records = details["records"].as_array().map_or(&[][..], Vec::as_slice);
            let notice = records.first().and_then(|record| {
                Some(orchbee_local_runtime::RecordNotice {
                    record: orchbee_local_runtime::VersionedRecord::from_token(
                        record["record"].as_str()?,
                    )?,
                    problem: if record["problem"] == "not-updated" {
                        orchbee_local_runtime::RecordProblem::NotUpdated
                    } else {
                        orchbee_local_runtime::RecordProblem::Newer
                    },
                    version: record["version"].as_u64().unwrap_or_default(),
                    path: std::path::PathBuf::from(record["path"].as_str().unwrap_or_default()),
                })
            });
            let mut fact =
                notice.map_or_else(|| reason.to_owned(), |notice| locale.record_notice(&notice));
            if records.len() > 1 {
                fact.push_str(&format!(" (+{})", records.len() - 1));
            }
            (fact, None)
        }
        ("records", _) => (String::new(), None),
        ("runtime", "fail") => {
            // The base when it is the unsafe or unreadable one, else the Session folder.
            let row = if matches!(
                details["runtimeBase"]["state"].as_str(),
                Some("unsafe" | "uninspectable")
            ) {
                "runtimeBase"
            } else {
                "runtimeDirectory"
            };
            (format!("{}: {reason}", path(&details[row])), None)
        }
        ("runtime", "warning") => (copy.cli_doctor_runtime_advisory(reason).to_owned(), None),
        ("runtime", _) => (path(&details["runtimeDirectory"]), None),
        ("session", "fail") => (copy.cli_doctor_session_failure(reason), None),
        ("session", "warning") => (copy.cli_runtime_status_value(reason).to_owned(), None),
        ("session", "not-applicable") => (copy.cli_doctor_not_running().to_owned(), None),
        ("session", _) => {
            let version = details["server"]["processes"]
                .as_array()
                .and_then(|processes| processes.iter().find(|process| process["role"] == "server"))
                .and_then(|process| process["version"].as_str())
                .unwrap_or_default();
            (copy.cli_doctor_running(&escape(version)), None)
        }
        (_, "warning") if reason == "home-unavailable" => {
            (copy.cli_doctor_home_unavailable().to_owned(), None)
        }
        (_, "warning") => {
            let outdated = details["integrations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|item| item["state"] == "update-required")
                .filter_map(|item| item["integrationId"].as_str())
                .collect::<Vec<_>>();
            let command = match outdated.as_slice() {
                [provider] => format!("orchbee agent integration enable --provider {provider}"),
                _ => "orchbee agent integration list --outdated-only".to_owned(),
            };
            (copy.cli_doctor_update_required(&outdated), Some(command))
        }
        _ => {
            let enabled = details["integrations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|item| item["state"] == "enabled")
                .count();
            (copy.cli_doctor_enabled(enabled), None)
        }
    }
}
