//! Explicit terminal snapshot sources and data-export grammar.

use super::{
    AgentTarget, ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, QueryCommand,
    Request, Target, UsageContext, UsageReason, query, usage_with, wait_matcher_option,
    wait_timeout_option,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputSource {
    Visible,
    Recent,
    RecentUnwrapped,
    Detection,
}

impl OutputSource {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Recent => "recent",
            Self::RecentUnwrapped => "recent-unwrapped",
            Self::Detection => "detection",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OutputSelection {
    pub(crate) source: OutputSource,
    pub(crate) lines: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OutputTarget {
    Terminal(Target),
    Agent(AgentTarget),
}

pub(super) fn request(
    target: OutputTarget,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    if options.ansi && options.plain {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--ansi",
                second: "--plain",
            },
            envelope,
        ));
    }
    if options.plain && options.format == Some(OutputFormat::Json) {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--plain",
                second: "--format",
            },
            envelope,
        ));
    }
    Ok(query(
        QueryCommand::TerminalOutput {
            target,
            selection: selection(options, context, envelope)?,
            plain: options.plain,
            ansi: options.ansi,
        },
        options,
    ))
}

pub(super) fn selection(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<OutputSelection, ParseFailure> {
    let source = match options.output_source.as_deref().unwrap_or("recent") {
        "visible" => OutputSource::Visible,
        "recent" => OutputSource::Recent,
        "recent-unwrapped" => OutputSource::RecentUnwrapped,
        "detection" => OutputSource::Detection,
        value => {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--source",
                    value: value.to_owned(),
                    expected: ExpectedValue::OutputSource,
                },
                envelope,
            ));
        }
    };
    let lines = options
        .lines
        .as_ref()
        .map(|value| {
            value
                .parse::<u32>()
                .ok()
                .filter(|_| !value.starts_with('+'))
                .map(|value| u16::try_from(value.min(1000)).expect("clamped to 1000"))
                .ok_or_else(|| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option: "--lines",
                            value: value.clone(),
                            expected: ExpectedValue::OutputLines,
                        },
                        envelope,
                    )
                })
        })
        .transpose()?;
    Ok(OutputSelection { source, lines })
}

pub(super) fn wait_request(
    target: Target,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    if options.forever && options.timeout.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--forever",
                second: "--timeout",
            },
            envelope,
        ));
    }
    Ok(query(
        QueryCommand::TerminalSourceWait {
            target,
            after: options
                .output_after
                .as_deref()
                .map(|value| {
                    super::OutputAfter::parse(value).ok_or_else(|| {
                        usage_with(
                            UsageReason::InvalidValue {
                                context,
                                option: "--after",
                                value: value.to_owned(),
                                expected: ExpectedValue::OutputCursor,
                            },
                            envelope,
                        )
                    })
                })
                .transpose()?,
            selection: selection(options, context, envelope)?,
            matcher: wait_matcher_option(options, context, envelope)?,
            timeout_ms: if options.forever {
                None
            } else {
                Some(wait_timeout_option(options, context, envelope)?)
            },
        },
        options,
    ))
}

#[cfg(test)]
mod tests;
