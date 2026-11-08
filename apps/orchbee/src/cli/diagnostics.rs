//! `diagnostics create`: one diagnostic bundle written where the person chooses (ORC-397). The
//! person sends it themselves; nothing is uploaded, and nothing is created in the data root.
//!
//! The bundle carries versions, system facts, `config check`, the Session list, integration
//! status, redacted settings and preferences, and the well-formed records of the last seven daily
//! logs. Every carried document passes through the local-runtime redaction rules; checkpoints,
//! layouts, terminal output, plugin data and saved machines are never read.
use super::schema_export::{PublishFailure, publish};
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::cli_text::CliText;
use crate::invocation::OutputFormat;
use crate::localization::Locale;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
#[path = "diagnostics/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "diagnostics/unsupported.rs"]
mod platform;

const SCHEMA: &str = "orchbee.diagnostics.create";
const FAILURE_SCHEMA: &str = "orchbee.cli.failure.diagnostics";
const BUNDLE_SCHEMA: &str = "orchbee.diagnostics.bundle";

/// The collected entries before the manifest is written, with each section's status token:
/// `included`, `absent` when the source does not exist, or `unavailable` when it exists but could
/// not be read or parsed, so nothing of it is carried.
#[derive(Default)]
struct Collected {
    sections: Vec<(&'static str, &'static str)>,
    entries: Vec<(String, Vec<u8>)>,
    omitted_log_lines: u64,
}

pub(crate) fn create(path: &str, format: OutputFormat, locale: Locale) -> Output {
    let collected = match platform::collect(locale) {
        Ok(collected) => collected,
        Err(failure) => return failure_output(failure, Some(format), locale),
    };
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
