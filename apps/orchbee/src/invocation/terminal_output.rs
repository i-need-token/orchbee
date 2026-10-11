//! Explicit terminal snapshot sources and data-export grammar.

use super::*;

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

pub(super) fn implicit_read(options: &Options) -> bool {
    cfg!(windows) && !options.history && options.rows.is_none()
}

pub(super) fn request(
    target: OutputTarget,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let selector = if options.output_source.is_some() {
        "--source"
    } else if options.ansi {
        "--ansi"
    } else {
        "--plain"
    };
    for (present, other) in [
        (options.history, "--history"),
        (options.rows.is_some(), "--rows"),
        (
            options.unwrap && (options.output_source.is_some() || options.plain || options.ansi),
            "--unwrap",
        ),
    ] {
        if present {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: selector,
                    second: other,
                },
                envelope,
            ));
        }
    }
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
    let default = if cfg!(windows) && options.unwrap {
        "recent-unwrapped"
    } else {
        "recent"
    };
    let source = match options.output_source.as_deref().unwrap_or(default) {
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
    if options.rows.is_some()
        && (!cfg!(windows) || options.output_source.is_some() || options.lines.is_some())
    {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--rows",
                second: if options.output_source.is_some() {
                    "--source"
                } else if options.lines.is_some() {
                    "--lines"
                } else {
                    "--forever"
                },
            },
            envelope,
        ));
    }
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
            selection: if cfg!(windows) && options.rows.is_some() {
                OutputSelection {
                    source: OutputSource::Recent,
                    lines: Some(history_rows_option(options, context, envelope)?),
                }
            } else {
                selection(options, context, envelope)?
            },
            matcher: wait_matcher_option(options, context, envelope)?,
            timeout_ms: if options.forever || (cfg!(windows) && options.timeout.is_none()) {
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
