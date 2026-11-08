//! `diagnostics create` grammar: the bundle's destination is required and validated lexically,
//! with no filesystem effect.
use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request, UsageContext,
    UsageReason, check_scope, required, schema_export, usage_with,
};

pub(super) fn request(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("diagnostics create");
    check_scope(options, context, envelope)?;
    let path = required(&options.output, context, "'--output'", envelope)?;
    if !schema_export::valid_path(&path) {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--output",
                value: path,
                expected: ExpectedValue::OutputFilePath,
            },
            envelope,
        ));
    }
    Ok(Request::DiagnosticsCreate {
        path,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}
