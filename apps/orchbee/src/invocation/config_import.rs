//! Explicit offline import grammar; validation performs no file, environment or runtime reads.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Scope, UsageContext, UsageReason, check_scope, missing_required, usage_with,
};

/// Explicit source/destination operands after pure ordinary-file syntax admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Command {
    pub(crate) input_file: String,
    pub(crate) output_file: String,
    pub(crate) dry_run: bool,
}

pub(super) fn request(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("config import-herdr");
    check_scope(
        options,
        Scope {
            config_import: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let operand = |name, value: &Option<String>| {
        let raw = value
            .as_ref()
            .ok_or_else(|| missing_required(context, name, envelope))?;
        if !orchbee_local_site_runtime::valid_windows_herdr_file_operand(raw) {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: name,
                    value: raw.clone(),
                    expected: ExpectedValue::OutputFilePath,
                },
                envelope,
            ));
        }
        Ok(raw.clone())
    };
    Ok(Request::ConfigImport {
        command: Command {
            input_file: operand("--input-file", &options.import_input_file)?,
            output_file: operand("--output-file", &options.import_output_file)?,
            dry_run: options.dry_run,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests;
