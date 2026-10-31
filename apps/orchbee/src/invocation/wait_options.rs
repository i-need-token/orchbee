//! Wait options: bounded output matchers, finite wait timeouts, and close waits.

use super::{
    CloseWait, ExpectedValue, FailureEnvelope, Options, ParseFailure, UsageContext, UsageReason,
    WaitPattern, check_scope, missing_required, usage_with,
};

/// The `--regex` pattern bound: a nonempty UTF-8 pattern of at most this
/// many bytes, compiled once before any effect.
const MAXIMUM_WAIT_PATTERN_BYTES: usize = 1024;

/// The compiled-program bound the `regex` builder enforces at compile time.
const REGEX_SIZE_LIMIT: usize = 1024 * 1024;

/// The regex engine's lazy DFA cache bound during matching.
const REGEX_DFA_SIZE_LIMIT: usize = 1024 * 1024;

/// `wait-output` admits exactly one search input: `--match` keeps the
/// literal, case-sensitive substring semantics (nonempty, ≤4096 bytes —
/// nothing is interpreted); `--regex` is a nonempty ≤1024-byte pattern
/// compiled once here under the declared program and DFA budgets, so an
/// invalid or oversized program is a usage failure before any runtime
/// access. Both present is a conflict; neither is a missing requirement.
pub(super) fn wait_matcher_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WaitPattern, ParseFailure> {
    match (&options.match_text, &options.regex) {
        (Some(_), Some(_)) => Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--match",
                second: "--regex",
            },
            envelope,
        )),
        (None, None) => Err(missing_required(
            context,
            "'--match' or '--regex'",
            envelope,
        )),
        (Some(text), None) => {
            if text.is_empty() || text.len() > 4096 {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--match",
                        value: text.clone(),
                        expected: ExpectedValue::MatchText,
                    },
                    envelope,
                ));
            }
            Ok(WaitPattern::Literal(text.clone()))
        }
        (None, Some(pattern)) => {
            if pattern.is_empty() || pattern.len() > MAXIMUM_WAIT_PATTERN_BYTES {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--regex",
                        value: pattern.clone(),
                        expected: ExpectedValue::RegexPattern,
                    },
                    envelope,
                ));
            }
            let compiled = regex::RegexBuilder::new(pattern)
                .size_limit(REGEX_SIZE_LIMIT)
                .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
                .build()
                .map_err(|_| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option: "--regex",
                            value: pattern.clone(),
                            expected: ExpectedValue::RegexPattern,
                        },
                        envelope,
                    )
                })?;
            Ok(WaitPattern::Regex(compiled))
        }
    }
}

/// `--timeout` under `wait-output`: the total wait budget in milliseconds
/// measured from command start. 1..=3600000, defaulting to 30000.
pub(super) fn wait_timeout_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u32, ParseFailure> {
    let Some(value) = options.timeout.clone() else {
        return Ok(30_000);
    };
    match value.parse::<u32>() {
        Ok(timeout) if (1..=3_600_000).contains(&timeout) => Ok(timeout),
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

/// `tab close` / `workspace close` settlement. A dry run discards nothing,
/// so it needs no `--confirm`; every real close does. Waiting is the
/// default; `--detach` returns at acceptance and conflicts with `--timeout`.
pub(super) fn close_wait(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<CloseWait, ParseFailure> {
    check_scope(options, context, envelope)?;
    let conflict = |first, second| {
        usage_with(
            UsageReason::ConflictingOptions {
                context,
                first,
                second,
            },
            envelope,
        )
    };
    if options.dry_run {
        if options.detach {
            return Err(conflict("--dry-run", "--detach"));
        }
        if options.timeout.is_some() {
            return Err(conflict("--dry-run", "--timeout"));
        }
        return Ok(CloseWait::DryRun);
    }
    if !options.confirm {
        return Err(missing_required(context, "'--confirm'", envelope));
    }
    if options.detach {
        if options.timeout.is_some() {
            return Err(conflict("--detach", "--timeout"));
        }
        return Ok(CloseWait::Detach);
    }
    Ok(CloseWait::Wait {
        timeout_ms: wait_timeout_option(options, context, envelope)?,
    })
}
