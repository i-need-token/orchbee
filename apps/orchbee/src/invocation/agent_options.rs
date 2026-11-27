//! Agent options: list limits, wait conditions and fences, and Agent selectors.

use super::{
    AgentSelector, AgentTarget, AgentWaitCondition, AgentWaitTimeout, ExpectedValue,
    FailureEnvelope, Options, ParseFailure, Resource, UsageContext, UsageReason, target,
    usage_with,
};
use orchbee_agent_observation_protocol::{
    AgentSafeInteger, AgentSessionId, DEFAULT_LIST_LIMIT, MAXIMUM_LIST_LIMIT, MAXIMUM_SAFE_INTEGER,
};

/// `agent list --limit`: the bounded page size, 1..=100, defaulting to the
/// wire contract's 50.
pub(super) fn agent_limit_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u64, ParseFailure> {
    let Some(value) = options.limit.clone() else {
        return Ok(DEFAULT_LIST_LIMIT);
    };
    match value.parse::<u64>() {
        Ok(limit) if (1..=MAXIMUM_LIST_LIMIT).contains(&limit) => Ok(limit),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--limit",
                value,
                expected: ExpectedValue::AgentListLimit,
            },
            envelope,
        )),
    }
}

/// `agent wait --until <condition>`: the repeatable canonical condition
/// vocabulary — every value must be one of the fourteen names, a repeated
/// value is a duplicate flag. Omission stays explicit for each execution owner
/// to apply its existing default conditions.
pub(super) fn agent_until_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<Vec<AgentWaitCondition>>, ParseFailure> {
    if options.until.is_empty() {
        return Ok(None);
    }
    let mut conditions = Vec::with_capacity(options.until.len());
    for value in &options.until {
        let Some(condition) = AgentWaitCondition::parse(value) else {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--until",
                    value: value.clone(),
                    expected: ExpectedValue::AgentWaitUntil,
                },
                envelope,
            ));
        };
        if conditions.contains(&condition) {
            return Err(usage_with(
                UsageReason::DuplicateOption { option: "--until" },
                envelope,
            ));
        }
        conditions.push(condition);
    }
    Ok(Some(conditions))
}

/// `agent wait --after-revision <n>`: the optional global-revision fence —
/// a nonnegative safe integer echoed verbatim into the emitted document.
pub(super) fn agent_after_revision_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<AgentSafeInteger>, ParseFailure> {
    match &options.after_revision {
        Some(value) => match value.parse::<u64>() {
            Ok(revision) if revision <= MAXIMUM_SAFE_INTEGER => {
                Ok(Some(AgentSafeInteger::new(revision)))
            }
            _ => Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--after-revision",
                    value: value.clone(),
                    expected: ExpectedValue::AgentAfterRevision,
                },
                envelope,
            )),
        },
        None => Ok(None),
    }
}

/// `agent wait --timeout`, also under `agent prompt --wait`, `terminal stream` and `terminal attach`:
/// the total budget in milliseconds measured from command start. Explicit values are 1..=86400000,
/// one day, so a long Agent turn fits one wait. With the unified Agent
/// projection, which every platform serves, the default is no total timeout; a
/// canonical-only read keeps 600000.
pub(super) fn agent_wait_timeout_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<AgentWaitTimeout>, ParseFailure> {
    if options.forever {
        if options.timeout.is_some() {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: "--forever",
                    second: "--timeout",
                },
                envelope,
            ));
        }
        return Ok(Some(AgentWaitTimeout::Forever));
    }
    let Some(value) = options.timeout.clone() else {
        return Ok(None);
    };
    match value.parse::<u32>() {
        Ok(timeout) if (1..=86_400_000).contains(&timeout) => {
            Ok(Some(AgentWaitTimeout::Milliseconds(timeout)))
        }
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--timeout",
                value,
                expected: ExpectedValue::AgentWaitTimeout,
            },
            envelope,
        )),
    }
}

pub(super) fn agent_selector(
    words: &[&str],
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<AgentTarget, ParseFailure> {
    if words.len() > 1 {
        return Err(usage_with(
            UsageReason::UnexpectedArgument {
                context,
                value: words[1].to_owned(),
            },
            envelope,
        ));
    }
    let selected = usize::from(!words.is_empty())
        + usize::from(options.panel.is_some())
        + usize::from(options.terminal.is_some());
    if selected > 1 {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "<target>",
                second: "--panel / --terminal",
            },
            envelope,
        ));
    }
    if let Some(value) = &options.terminal {
        return Ok(AgentTarget::Terminal {
            target: target(value.clone(), Resource::Terminal, envelope)?,
        });
    }
    if let Some(value) = &options.panel {
        return Ok(AgentTarget::Panel {
            target: target(value.clone(), Resource::Panel, envelope)?,
        });
    }
    if let Some(value) = words.first() {
        if let Some(value) = value.strip_prefix("@agent:") {
            let selector = if value.bytes().all(|b| b.is_ascii_digit()) {
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n > 0 && *n <= 4096 && n.to_string() == value)
                    .map(AgentSelector::Ordinal)
            } else {
                orchbee_agent_observation_protocol::AgentProviderId::parse(value)
                    .ok()
                    .map(|kind| AgentSelector::Kind(kind.to_string()))
            }
            .ok_or_else(|| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<target>",
                        value: format!("@agent:{value}"),
                        expected: ExpectedValue::AgentName,
                    },
                    envelope,
                )
            })?;
            return Ok(AgentTarget::Projection { selector });
        }
        if value.starts_with("@terminal:") {
            return Ok(AgentTarget::Terminal {
                target: target((*value).to_owned(), Resource::Terminal, envelope)?,
            });
        }
        if value.starts_with("@panel:") {
            return Ok(AgentTarget::Panel {
                target: target((*value).to_owned(), Resource::Panel, envelope)?,
            });
        }
        return Ok(AgentTarget::Auto {
            target: AgentSessionId::parse(value).map_err(|_| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<target>",
                        value: (*value).to_owned(),
                        expected: ExpectedValue::AgentTarget,
                    },
                    envelope,
                )
            })?,
        });
    }
    Err(usage_with(
        UsageReason::MissingRequired {
            context,
            requirement: "<target> or --panel <id|selector> or --terminal <id|selector>",
        },
        envelope,
    ))
}
