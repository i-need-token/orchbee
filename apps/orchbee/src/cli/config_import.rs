//! Explicit offline import. Every possible publication receipt is encoded before CREATE_NEW.
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{
    invocation::{OutputFormat, config_import::Command},
    localization::Locale,
};
use orchbee_local_site_runtime::{
    PreparedWindowsHerdrImport, WindowsHerdrImportWriteFailure,
    normalize_windows_herdr_file_operand, prepare_windows_herdr_import, read_windows_herdr_source,
    write_windows_herdr_import,
};
use serde_json::json;
use std::{ffi::OsString, path::Path};

pub(crate) struct Receipt<'a> {
    pub(crate) source: Option<&'a str>,
    pub(crate) destination: Option<&'a str>,
    pub(crate) plan: Option<&'a PreparedWindowsHerdrImport>,
    pub(crate) preview: bool,
    pub(crate) created: bool,
    pub(crate) error: Option<(&'static str, &'static str)>,
}

pub(crate) fn run(command: &Command, format: OutputFormat, locale: Locale) -> Output {
    // Capture one caller context, never consult Site/config-selection variables.
    let cwd = std::cell::OnceCell::new();
    let directory = || {
        cwd.get_or_init(|| std::env::current_dir().ok())
            .clone()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
    };
    let names = ["USERPROFILE", "HOMEDRIVE", "HOMEPATH", "HOME"];
    let home: [std::cell::OnceCell<Option<OsString>>; 4] =
        std::array::from_fn(|_| std::cell::OnceCell::new());
    let env = |name: &str| {
        names
            .iter()
            .position(|key| *key == name)
            .and_then(|index| home[index].get_or_init(|| std::env::var_os(name)).clone())
    };
    let mut receipt = Receipt {
        source: None,
        destination: None,
        plan: None,
        preview: false,
        created: false,
        error: None,
    };
    let source = match normalize_windows_herdr_file_operand(&command.input_file, &env, directory) {
        Ok(path) => path,
        Err(error) => {
            receipt.error = Some(("source", error.reason));
            return emit(&receipt, format, locale);
        }
    };
    receipt.source = Some(&source);
    let destination =
        match normalize_windows_herdr_file_operand(&command.output_file, &env, directory) {
            Ok(path) => path,
            Err(error) => {
                receipt.error = Some(("destination", error.reason));
                return emit(&receipt, format, locale);
            }
        };
    receipt.destination = Some(&destination);
    let text = match read_windows_herdr_source(Path::new(&source)) {
        Ok(text) => text,
        Err(error) => {
            receipt.error = Some((error.field, error.reason));
            return emit(&receipt, format, locale);
        }
    };
    let plan =
        match prepare_windows_herdr_import(&text, Path::new(&source), Path::new(&destination), env)
        {
            Ok(plan) => plan,
            Err(error) => {
                receipt.error = Some((error.field, error.reason));
                return emit(&receipt, format, locale);
            }
        };
    receipt.plan = Some(&plan);
    if command.dry_run {
        receipt.preview = true;
        return emit(&receipt, format, locale);
    }
    receipt.created = true;
    let Ok(success) = encode(&receipt, format, locale) else {
        return encoding_failure(format, locale);
    };
    let mut failures = Vec::new();
    for failure in WRITE_FAILURES {
        receipt.created = failure.created();
        receipt.error = Some((failure.stage(), failure.reason()));
        let Ok(encoded) = encode(&receipt, format, locale) else {
            return encoding_failure(format, locale);
        };
        failures.push((*failure, encoded));
    }
    match write_windows_herdr_import(Path::new(&destination), &plan) {
        Ok(()) => success,
        Err(failure) => {
            failures
                .into_iter()
                .find(|(case, _)| *case == failure)
                .expect("publication adapter returns only its closed failure vocabulary")
                .1
        }
    }
}

const WRITE_FAILURES: &[WindowsHerdrImportWriteFailure] = &[
    WindowsHerdrImportWriteFailure::BeforeCreate {
        stage: "destination",
        reason: "invalid-path",
    },
    WindowsHerdrImportWriteFailure::BeforeCreate {
        stage: "destination",
        reason: "unsafe-parent",
    },
    WindowsHerdrImportWriteFailure::BeforeCreate {
        stage: "create",
        reason: "already-exists",
    },
    WindowsHerdrImportWriteFailure::BeforeCreate {
        stage: "create",
        reason: "create-failed",
    },
    WindowsHerdrImportWriteFailure::AfterCreate {
        stage: "protect",
        reason: "protect-failed",
    },
    WindowsHerdrImportWriteFailure::AfterCreate {
        stage: "write",
        reason: "write-failed",
    },
    WindowsHerdrImportWriteFailure::AfterCreate {
        stage: "synchronize",
        reason: "sync-failed",
    },
];

fn encoding_failure(format: OutputFormat, locale: Locale) -> Output {
    failure_output(
        Failure::new(FailureKind::InternalFailure),
        Some(format),
        locale,
    )
}
fn emit(receipt: &Receipt<'_>, format: OutputFormat, locale: Locale) -> Output {
    encode(receipt, format, locale).unwrap_or_else(|_| encoding_failure(format, locale))
}
fn encode(
    receipt: &Receipt<'_>,
    format: OutputFormat,
    locale: Locale,
) -> Result<Output, output::TooLarge> {
    let failed = receipt.error.is_some();
    let text = locale.cli_config_import(receipt);
    let stdout = match format {
        OutputFormat::Text if failed => Vec::new(),
        OutputFormat::Text => output::encode_text(&text)?,
        OutputFormat::Json => {
            let mut result = json!({
                "state": if failed { "failed" } else if receipt.preview { "preview" } else { "created" },
                "sourcePath": receipt.source, "destinationPath": receipt.destination,
                "candidateBytes": receipt.plan.map(|plan| plan.candidate_toml().len()),
                "fields": receipt.plan.map(|plan| plan.report().items().iter().map(|item| json!({
                    "field": item.field, "targetFields": item.target_fields,
                    "disposition": item.disposition.as_str(), "reason": item.reason,
                })).collect::<Vec<_>>()).unwrap_or_default(),
                "omittedCount": receipt.plan.map_or(0, |plan| plan.report().omitted_count()),
                "created": receipt.created, "recoveryRequired": failed && receipt.created,
                "error": receipt.error.map(|(stage, reason)| json!({"stage": stage, "reason": reason})),
            });
            if receipt.preview {
                result["candidateToml"] = receipt
                    .plan
                    .expect("preview requires preparation")
                    .candidate_toml()
                    .into();
            }
            output::encode_document(
                &json!({"schema": "orchbee.config.import-herdr", "version": 1,
                "source": {"kind": "local-configuration"}, "result": result}),
            )?
        }
    };
    Ok(Output {
        status: u8::from(failed),
        stdout,
        stderr: if failed { text } else { String::new() },
        finalization: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_import_receipts_obey_the_closed_schema_without_disclosing_candidate_values() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("schemas/orchbee.config.import-herdr.json")).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let plan = prepare_windows_herdr_import(
            "[ui]\nwindow_title='sensitive-value'",
            Path::new("C:/source.toml"),
            Path::new("C:/target.toml"),
            |_| None,
        )
        .unwrap();
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut receipt = Receipt {
                source: Some("C:/source.toml"),
                destination: Some("C:/target.toml"),
                plan: Some(&plan),
                preview: false,
                created: true,
                error: None,
            };
            for case in 0..WRITE_FAILURES.len() + 2 {
                receipt.preview = case == 1;
                receipt.created = case == 0;
                receipt.error = None;
                if let Some(failure) = case.checked_sub(2).and_then(|i| WRITE_FAILURES.get(i)) {
                    receipt.created = failure.created();
                    receipt.error = Some((failure.stage(), failure.reason()));
                }
                let answer = encode(&receipt, OutputFormat::Json, locale).unwrap();
                let document: serde_json::Value = serde_json::from_slice(&answer.stdout).unwrap();
                assert!(validator.is_valid(&document), "{document}");
                assert_eq!(
                    document["result"].get("candidateToml").is_some(),
                    receipt.preview
                );
                assert!(!answer.stderr.contains("sensitive-value"));
                let text = encode(&receipt, OutputFormat::Text, locale).unwrap();
                assert!(
                    !String::from_utf8(text.stdout)
                        .unwrap()
                        .contains("sensitive-value")
                );
                let mut invalid = document.clone();
                invalid["result"]["created"] = (!receipt.created).into();
                assert!(!validator.is_valid(&invalid));
            }
        }
    }
}
