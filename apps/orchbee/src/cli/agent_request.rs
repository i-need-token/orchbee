//! Backend interpretation of opaque cursor and omitted wait options, before effects.
use crate::invocation::{
    AgentWaitCondition::{
        self, Blocked, Canceled, Done, Ended, Failed, Idle, Interrupted, NeedsAttention, Succeeded,
    },
    AgentWaitTimeout, ExpectedValue, OutputFormat, ParseFailure, UsageContext, UsageReason,
};

/// Hosts that admit the unified Agent projection wait on its conditions, which physical terminal
/// Agents also answer, with no total timeout.
const PROJECTION_CONDITIONS: &[AgentWaitCondition] = &[Idle, Done, Blocked];
const PROJECTION_TIMEOUT: Option<u32> = None;
/// Canonical-only waits keep their sampled-condition defaults and ten-minute bound.
const CANONICAL_CONDITIONS: &[AgentWaitCondition] = &[
    NeedsAttention,
    Failed,
    Succeeded,
    Canceled,
    Interrupted,
    Idle,
    Ended,
];
const CANONICAL_TIMEOUT: Option<u32> = Some(600_000);

fn projected() -> bool {
    super::execution::admit(super::execution::Capability::AgentProjection).is_ok()
}

pub(super) fn list_cursor<Cursor, Error>(
    value: Option<&str>,
    format: OutputFormat,
    parse: impl Fn(&str) -> Result<Cursor, Error>,
) -> Result<Option<Cursor>, ParseFailure> {
    value
        .map(|value| {
            parse(value).map_err(|_| {
                crate::invocation::usage_with(
                    UsageReason::InvalidValue {
                        context: UsageContext::Path("agent list"),
                        option: "--cursor",
                        value: value.to_owned(),
                        expected: ExpectedValue::AgentCursor,
                    },
                    crate::invocation::selection_envelope(Some(format)),
                )
            })
        })
        .transpose()
}

/// The requested conditions, or this host's defaults when the grammar retained an omitted option.
pub(super) fn wait_conditions(requested: Option<&[AgentWaitCondition]>) -> &[AgentWaitCondition] {
    requested.unwrap_or(if projected() {
        PROJECTION_CONDITIONS
    } else {
        CANONICAL_CONDITIONS
    })
}

pub(super) fn wait_timeout(requested: Option<AgentWaitTimeout>) -> Option<u32> {
    match requested {
        Some(AgentWaitTimeout::Milliseconds(value)) => Some(value),
        Some(AgentWaitTimeout::Forever) => None,
        None if projected() => PROJECTION_TIMEOUT,
        None => CANONICAL_TIMEOUT,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn malformed_page_cursors_fail_before_backend_effects() {
        for value in [
            "nope",
            "v1.short.tag",
            "v2.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ",
        ] {
            let failure = super::list_cursor(
                Some(value),
                super::OutputFormat::Json,
                orchbee_agent_observation_protocol::AgentListCursor::parse,
            )
            .expect_err("malformed cursor");
            assert!(matches!(
                failure.usage,
                Some(super::UsageReason::InvalidValue {
                    option: "--cursor",
                    ..
                })
            ));
            assert_eq!(failure.envelope, crate::invocation::FailureEnvelope::Json);
        }
    }
}
