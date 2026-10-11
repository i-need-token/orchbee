//! Exact report targets are validated before discovery; membership stays snapshot-owned.
use super::{
    ExpectedValue, FailureEnvelope, Options, ParseFailure, UsageContext, UsageReason, usage_with,
};
use orchbee_workbench_protocol::TerminalPresentationId;

pub(super) fn parse(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<TerminalPresentationId>, ParseFailure> {
    options
        .report_presentation
        .as_deref()
        .map(|value| {
            TerminalPresentationId::parse(value).map_err(|_| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--presentation",
                        value: value.to_owned(),
                        expected: ExpectedValue::PresentationId,
                    },
                    envelope,
                )
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests;
