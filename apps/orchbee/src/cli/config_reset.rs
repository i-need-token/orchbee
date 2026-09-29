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
mod tests {
    use super::*;
    use orchbee_local_site_runtime::WindowsSettingsError;

    #[test]
    fn reset_receipts_keep_backup_and_recovery_evidence_in_both_locales() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("schemas/orchbee.config.reset-keys.json")).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for case in [
                "missing", "no-keys", "reset", "failed", "recovery", "cleanup",
            ] {
                let outcome = match case {
                    "missing" => WindowsKeysResetOutcome::Missing,
                    "no-keys" => WindowsKeysResetOutcome::NoKeys,
                    "reset" => WindowsKeysResetOutcome::Reset {
                        backup_path: "C:/settings/config.toml.bak-keys-1".into(),
                    },
                    _ => WindowsKeysResetOutcome::Failed {
                        error: WindowsSettingsError::new("file", "write-failed"),
                        backup_path: (case != "failed")
                            .then(|| "C:/settings/config.toml.bak-keys-1".into()),
                        recovery_required: case == "recovery",
                    },
                };
                let answer = emit(
                    WindowsKeysResetReport {
                        path: Some("C:/settings/config.toml".into()),
                        outcome,
                    },
                    OutputFormat::Json,
                    locale,
                );
                let document: serde_json::Value = serde_json::from_slice(&answer.stdout).unwrap();
                assert!(validator.is_valid(&document), "{document}");
                assert_eq!(
                    answer.status,
                    u8::from(matches!(case, "failed" | "recovery" | "cleanup"))
                );
                assert_eq!(document["result"]["recoveryRequired"], case == "recovery");
                if case == "recovery" {
                    assert!(answer.stderr.contains("config.toml.bak-keys-1"));
                }
                let mut invalid = document.clone();
                invalid["result"]["unknown"] = true.into();
                assert!(!validator.is_valid(&invalid));
                if case == "reset" || case == "recovery" {
                    let mut invalid = document;
                    invalid["result"]["backupPath"] = serde_json::Value::Null;
                    assert!(!validator.is_valid(&invalid));
                }
            }
        }
    }
}
