//! Offline settings repair; this command never connects to or activates a Session.
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{invocation::OutputFormat, localization::Locale};
use orchbee_local_runtime::{KeysResetOutcome, KeysResetReport, reset_keybindings};
use serde_json::json;

pub(crate) fn run(format: OutputFormat, locale: Locale) -> Output {
    if crate::tui_capabilities::Capability::ConfigurationFile
        .require()
        .is_err()
    {
        return super::unsupported_platform(Some(format), locale);
    }
    emit(
        reset_keybindings(|name| std::env::var_os(name)),
        format,
        locale,
    )
}

fn emit(report: KeysResetReport, format: OutputFormat, locale: Locale) -> Output {
    let (state, backup, error, recovery) = match &report.outcome {
        KeysResetOutcome::Missing => ("missing", None, None, false),
        KeysResetOutcome::NoKeys => ("no-keys", None, None, false),
        KeysResetOutcome::Reset { backup_path } => ("reset", Some(backup_path), None, false),
        KeysResetOutcome::Failed {
            error,
            backup_path,
            recovery_required,
        } => (
            "failed",
            backup_path.as_ref(),
            Some(error),
            *recovery_required,
        ),
    };
    let text = locale.cli_config_reset(&report);
    let failed = error.is_some();
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&json!({
            "schema": "orchbee.config.reset-keys", "version": 1,
            "source": { "kind": "local-configuration" },
            "result": {
                "state": state,
                "path": report.path.as_deref().and_then(std::path::Path::to_str),
                "backupPath": backup.and_then(|path| path.to_str()),
                "recoveryRequired": recovery,
                "error": error.map(|error| json!({"field": error.field, "reason": error.reason})),
            }
        })),
        OutputFormat::Text if failed => Ok(Vec::new()),
        OutputFormat::Text => output::encode_text(&text),
    };
    match encoded {
        Ok(stdout) => Output {
            status: u8::from(failed),
            stdout,
            stderr: if failed { text } else { String::new() },
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::InternalFailure),
            Some(format),
            locale,
        ),
    }
}

#[cfg(test)]
mod tests;
