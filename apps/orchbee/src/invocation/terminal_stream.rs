use super::{
    AgentTarget, ExpectedValue, FailureEnvelope, Options, ParseFailure, Request, Resource, Target,
    UsageContext, UsageReason, agent_selector, agent_wait_timeout_option, check_scope,
    missing_required, target, unexpected_argument, usage_with,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalObserveCommand {
    pub(crate) target: Target,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) timeout_ms: Option<u32>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalControlCommand {
    pub(crate) stream: TerminalObserveCommand,
    pub(crate) takeover: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachTarget {
    Terminal(Target),
    Agent(AgentTarget),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalAttachCommand {
    pub(crate) target: AttachTarget,
    pub(crate) takeover: bool,
    pub(crate) timeout_ms: Option<u32>,
    pub(crate) mouse: bool,
}
pub(super) fn attach(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
    agent: bool,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path(if agent {
        "agent attach"
    } else {
        "terminal attach"
    });
    check_scope(options, context, envelope)?;
    let selected = if agent {
        AttachTarget::Agent(agent_selector(operands, options, context, envelope)?)
    } else {
        let [value] = operands else {
            return Err(if operands.is_empty() {
                missing_required(context, "<terminal>", envelope)
            } else {
                unexpected_argument(context, operands[1], envelope)
            });
        };
        AttachTarget::Terminal(target((*value).to_owned(), Resource::Terminal, envelope)?)
    };
    Ok(Request::TerminalAttach {
        command: TerminalAttachCommand {
            target: selected,
            takeover: options.takeover,
            mouse: options.mouse.unwrap_or(true),
            timeout_ms: if options.timeout.is_some() {
                explicit_timeout(options, context, envelope)?
            } else {
                None
            },
        },
    })
}
pub(super) fn control(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    Ok(Request::TerminalControl {
        command: TerminalControlCommand {
            stream: stream(operands, options, envelope, true)?,
            takeover: options.takeover,
        },
    })
}
pub(super) fn observe(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    Ok(Request::TerminalObserve {
        command: stream(operands, options, envelope, false)?,
    })
}
fn stream(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
    control: bool,
) -> Result<TerminalObserveCommand, ParseFailure> {
    let context = UsageContext::Path(if control {
        "terminal session control"
    } else {
        "terminal session observe"
    });
    check_scope(options, context, envelope)?;
    for (present, flag) in [
        (options.cell_width.is_some(), "--cell-width"),
        (options.cell_height.is_some(), "--cell-height"),
    ] {
        if present {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: flag,
                },
                envelope,
            ));
        }
    }
    let [value] = operands else {
        return Err(if operands.is_empty() {
            missing_required(context, "<terminal>", envelope)
        } else {
            unexpected_argument(context, operands[1], envelope)
        });
    };
    let dimension = |value: &Option<String>, default, option| {
        value.as_ref().map_or(Ok(default), |value| {
            value
                .parse::<u16>()
                .ok()
                .filter(|n| (1..=4096).contains(n) && value.bytes().all(|c| c.is_ascii_digit()))
                .ok_or_else(|| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option,
                            value: value.clone(),
                            expected: ExpectedValue::Dimension,
                        },
                        envelope,
                    )
                })
        })
    };
    let columns = dimension(&options.cols, 120, "--cols")?;
    let rows = dimension(&options.rows, 40, "--rows")?;
    if u32::from(columns) * u32::from(rows) > 262144 {
        return Err(usage_with(
            UsageReason::InvalidGeometry {
                context,
                cols: columns,
                rows,
                cell_width: 0,
                cell_height: 0,
            },
            envelope,
        ));
    }
    let timeout_ms = if options.timeout.is_some() {
        explicit_timeout(options, context, envelope)?
    } else {
        None
    };
    Ok(TerminalObserveCommand {
        target: target((*value).to_owned(), Resource::Terminal, envelope)?,
        columns,
        rows,
        timeout_ms,
    })
}

#[cfg(test)]
mod tests;

fn explicit_timeout(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<u32>, ParseFailure> {
    Ok(
        match agent_wait_timeout_option(options, context, envelope)? {
            Some(super::AgentWaitTimeout::Milliseconds(value)) => Some(value),
            Some(super::AgentWaitTimeout::Forever) | None => None,
        },
    )
}
