//! Agent options: list limits, provider and adapter sources, session identities, wait conditions
//! and fences, doctor filters, Agent names, and Agent selectors.

use super::{
    AgentSelector, AgentTarget, AgentWaitCondition, AgentWaitTimeout, ExpectedValue,
    FailureEnvelope, Options, ParseFailure, Resource, UsageContext, UsageReason, grammar,
    missing_required, target, usage_with, user_path,
};
use orchbee_agent_observation_protocol::{
    AgentName, AgentSafeInteger, AgentSessionId, DEFAULT_LIST_LIMIT, MAXIMUM_LIST_LIMIT,
    MAXIMUM_SAFE_INTEGER,
};
use std::path::PathBuf;

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

/// `agent integration --provider <id>`: the closed provider vocabulary —
/// an unknown spelling is a usage failure, never a fuzzy match. `list`
/// admits it as an optional filter.
pub(super) fn provider_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<crate::agent_integrations::IntegrationId>, ParseFailure> {
    // Single-use on this family: the shared repeatable arm records every
    // `--provider` into `providers`; a second occurrence is a duplicate here.
    if options.providers.len() > 1 {
        return Err(usage_with(
            UsageReason::DuplicateOption {
                option: "--provider",
            },
            envelope,
        ));
    }
    match &options.provider {
        Some(value) => crate::agent_integrations::IntegrationId::parse_provider(value)
            .map(Some)
            .ok_or_else(|| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--provider",
                        value: value.clone(),
                        expected: ExpectedValue::Provider,
                    },
                    envelope,
                )
            }),
        None => Ok(None),
    }
}

/// The required `--provider` on explain/enable/disable.
pub(super) fn required_provider(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<crate::agent_integrations::IntegrationId, ParseFailure> {
    provider_option(options, context, envelope)?
        .ok_or_else(|| missing_required(context, "'--provider'", envelope))
}

/// `agent integration --adapter-source <abs>`: a literal absolute path —
/// never expanded, created, or probed at parse time.
pub(super) fn adapter_source_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<PathBuf>, ParseFailure> {
    match &options.adapter_source {
        Some(value)
            if value.is_empty() || !user_path::absolute(value) || value.as_bytes().contains(&0) =>
        {
            Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--adapter-source",
                    value: value.clone(),
                    expected: ExpectedValue::AdapterSource,
                },
                envelope,
            ))
        }
        Some(value) => Ok(Some(PathBuf::from(value))),
        None => Ok(None),
    }
}

/// The `agent get|explain|wait` operand: a wire-ready Agent session identity —
/// opaque `1..=128` `[A-Za-z0-9._:-]+`, never an id/label split.
fn agent_session_id(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<AgentSessionId, ParseFailure> {
    AgentSessionId::parse(value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "<session>",
                value: value.to_owned(),
                expected: ExpectedValue::AgentSessionId,
            },
            envelope,
        )
    })
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

/// `agent wait --timeout`: the total wait budget in milliseconds measured
/// from command start. Explicit values are 1..=3600000. Windows defaults to
/// no total timeout; existing non-Windows defaults remain 600000.
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
        Ok(timeout) if (1..=3_600_000).contains(&timeout) => {
            Ok(Some(AgentWaitTimeout::Milliseconds(timeout)))
        }
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--timeout",
                value,
                expected: ExpectedValue::WaitTimeout,
            },
            envelope,
        )),
    }
}

/// `agent doctor --provider <id>`: each repeatable value must be one
/// canonical provider id — an unknown id is a typed parse failure, never a
/// silently empty selection. A canonical Agent that Orchbee can only start
/// gets the start-only refusal, which names the Agents with an integration.
pub(super) fn doctor_providers_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Vec<String>, ParseFailure> {
    options
        .providers
        .iter()
        .map(|value| {
            if let Some(provider) = crate::agent_integrations::IntegrationId::parse_provider(value)
                && grammar::values::DOCTOR_PROVIDER_IDS.contains(&provider.as_str())
            {
                Ok(provider.as_str().to_owned())
            } else {
                let expected = if grammar::values::start_only_provider(value) {
                    ExpectedValue::StartOnlyProvider
                } else {
                    ExpectedValue::ProviderId
                };
                Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--provider",
                        value: value.clone(),
                        expected,
                    },
                    envelope,
                ))
            }
        })
        .collect()
}

/// `agent doctor --asset-root <path>`: each repeatable value must be a
/// nonempty absolute NUL-free path — relative spellings are usage failures
/// at this boundary, never `~`/`$`-expanded and never probed as guesses.
/// At most four supplied roots may be declared.
pub(super) fn doctor_asset_roots_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Vec<String>, ParseFailure> {
    for value in &options.asset_roots {
        // Doctor never routes to another machine, so a root must also be absolute on this host.
        if value.is_empty()
            || !user_path::absolute(value)
            || !std::path::Path::new(value).is_absolute()
            || value.as_bytes().contains(&0)
        {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--asset-root",
                    value: value.clone(),
                    expected: ExpectedValue::AssetRootPath,
                },
                envelope,
            ));
        }
    }
    if options.asset_roots.len() > 4 {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--asset-root",
                value: options.asset_roots.join(" "),
                expected: ExpectedValue::AssetRootPath,
            },
            envelope,
        ));
    }
    Ok(options.asset_roots.clone())
}

pub(super) fn agent_name(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<AgentName, ParseFailure> {
    AgentName::parse(value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "name",
                value: value.to_owned(),
                expected: ExpectedValue::AgentName,
            },
            envelope,
        )
    })
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
        + usize::from(options.agent_session.is_some())
        + usize::from(options.name.is_some())
        + usize::from(options.panel.is_some())
        + usize::from(options.terminal.is_some());
    if selected > 1 {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "<target>",
                second: "--agent-session / --name / --panel / --terminal",
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
    if let Some(value) = &options.agent_session {
        return Ok(AgentTarget::Session {
            session_id: agent_session_id(value, context, envelope)?,
        });
    }
    if let Some(value) = &options.name {
        return Ok(AgentTarget::Name {
            name: agent_name(value, context, envelope)?,
        });
    }
    Err(usage_with(
        UsageReason::MissingRequired {
            context,
            requirement: "<target> or --agent-session <id> or --name <alias> or --panel <id|selector> or --terminal <id|selector>",
        },
        envelope,
    ))
}
