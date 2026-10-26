//! Public screen-buffer coordinates are admitted before runtime discovery.
use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request, Resource, Target,
    TerminalTextPoint, UsageContext, UsageReason, check_scope, target, usage_with,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalSelectionCommand {
    pub(crate) target: Target,
    pub(crate) anchor: TerminalTextPoint,
    pub(crate) cursor: TerminalTextPoint,
    pub(crate) revision: Option<u64>,
    pub(crate) generation: Option<u64>,
}

const CONTEXT: UsageContext = UsageContext::Path("terminal selection-read");

pub(super) fn parse(
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    check_scope(options, CONTEXT, envelope)?;
    let anchor = super::terminal_buffer::point(
        options.selection_anchor.as_deref(),
        "--anchor",
        CONTEXT,
        envelope,
    )?;
    let cursor =
        super::terminal_buffer::point(options.cursor.as_deref(), "--cursor", CONTEXT, envelope)?;
    if u64::from(anchor.row.abs_diff(cursor.row)) + 1 > 4096 {
        return Err(invalid(
            options.cursor.as_deref().expect("required cursor"),
            "--cursor",
            ExpectedValue::TerminalSelectionSpan,
            envelope,
        ));
    }
    Ok(Request::TerminalSelection {
        command: TerminalSelectionCommand {
            target: target(value.to_owned(), Resource::Terminal, envelope)?,
            anchor,
            cursor,
            revision: options
                .content_revision
                .as_deref()
                .map(|raw| {
                    super::terminal_buffer::number(
                        raw,
                        "--content-revision",
                        false,
                        CONTEXT,
                        envelope,
                    )
                })
                .transpose()?,
            generation: options
                .generation
                .as_deref()
                .map(|raw| {
                    super::terminal_buffer::number(raw, "--generation", true, CONTEXT, envelope)
                })
                .transpose()?,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

fn invalid(
    raw: &str,
    option: &'static str,
    expected: ExpectedValue,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::InvalidValue {
            context: CONTEXT,
            option,
            value: raw.into(),
            expected,
        },
        envelope,
    )
}

#[cfg(test)]
mod tests;
