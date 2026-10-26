//! Backend interpretation of opaque cursor and omitted wait options, before effects.
use crate::invocation::{
    AgentWaitCondition, AgentWaitTimeout, ExpectedValue, OutputFormat, ParseFailure, UsageContext,
    UsageReason,
};

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

pub(super) fn wait_conditions(requested: Option<&[AgentWaitCondition]>) -> &[AgentWaitCondition] {
    use AgentWaitCondition::*;
    #[cfg(windows)]
    const DEFAULT: &[AgentWaitCondition] = &[Idle, Done, Blocked];
    #[cfg(not(windows))]
    const DEFAULT: &[AgentWaitCondition] = &[
        NeedsAttention,
        Failed,
        Succeeded,
        Canceled,
        Interrupted,
        Idle,
        Ended,
    ];
    requested.unwrap_or(DEFAULT)
}

pub(super) fn wait_timeout(requested: Option<AgentWaitTimeout>) -> Option<u32> {
    match requested {
        Some(AgentWaitTimeout::Milliseconds(value)) => Some(value),
        Some(AgentWaitTimeout::Forever) => None,
        None => {
            #[cfg(windows)]
            {
                None
            }
            #[cfg(not(windows))]
            {
                Some(600_000)
            }
        }
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
