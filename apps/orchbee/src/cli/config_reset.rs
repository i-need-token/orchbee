//! Offline settings repair; this command never connects to or activates a Site.
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{invocation::OutputFormat, localization::Locale};
use orchbee_local_site_runtime::{
    WindowsKeysResetOutcome, WindowsKeysResetReport, reset_windows_keybindings,
};
use serde_json::json;

pub(crate) fn run(format: OutputFormat, locale: Locale) -> Output {
    emit(
        reset_windows_keybindings(|name| std::env::var_os(name)),
        format,
        locale,
    )
}

fn emit(report: WindowsKeysResetReport, format: OutputFormat, locale: Locale) -> Output {
    let (state, backup, error, recovery) = match &report.outcome {
        WindowsKeysResetOutcome::Missing => ("missing", None, None, false),
        WindowsKeysResetOutcome::NoKeys => ("no-keys", None, None, false),
        WindowsKeysResetOutcome::Reset { backup_path } => ("reset", Some(backup_path), None, false),
        WindowsKeysResetOutcome::Failed {
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
