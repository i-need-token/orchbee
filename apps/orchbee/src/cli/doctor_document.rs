//! `agent doctor` process answer: the versioned machine document over the
//! workflow's typed report, its text rendering, and the shared output bound.

use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, Output, failure_output, output};
use crate::agent_doctor::Report;
use crate::cli_text::CliText;
use crate::invocation::OutputFormat;
use crate::localization::Locale;

/// `orchbee agent doctor [--provider <id>]... [--asset-root <path>]...
/// [--format text|json]` — one offline diagnostic document; exit 0 whenever
/// a complete or partial document was produced.
pub(super) fn run(
    providers: &[String],
    asset_roots: &[String],
    format: OutputFormat,
    locale: Locale,
) -> Output {
    match crate::agent_doctor::run(providers, asset_roots) {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok(report) => emit(report, format, locale),
    }
}

/// Assembles and emits the document — exit 0 for complete and partial
/// documents alike (a partial document is itself the answer). Hint text in
/// the document is always the canonical `en-US` statement.
fn emit(report: Report, format: OutputFormat, locale: Locale) -> Output {
    let hints: Vec<Value> = report
        .hints
        .iter()
        .map(|hint| {
            json!({
                "id": hint.id,
                "evidence": hint.evidence,
                "text": CliText::new(Locale::EnUs).cli_agent_doctor_hint_text(hint.id, &hint.subjects),
            })
        })
        .collect();
    let result = json!({
        "complete": report.complete,
        "providers": report.providers,
        "endpoint": report.endpoint,
        "asset": { "roots": report.roots },
        "environment": report.environment,
        "hints": hints,
    });
    let too_large = || {
        failure_output(
            Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                "limitBytes".to_owned(),
                json!(output::LIMIT),
            )])),
            Some(format),
            locale,
        )
    };
    match format {
        OutputFormat::Json => {
            let mut document = json!({
                "schema": "orchbee.agent.doctor",
                "version": 1,
                "source": crate::cli_outcome::local_runtime_source(),
                "result": result,
            });
            if report.machine_socket_unsupported {
                document["result"]["endpoint"]["socket"] = json!({"kind":"unsupported"});
            }
            match output::encode_document(&document) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(),
            }
        }
        OutputFormat::Text => {
            match output::render_text(|out| super::present::agent_doctor(out, &result, locale)) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(),
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests;
