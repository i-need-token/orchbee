//! `operation status|resume <request-id>`: the request ID alone names the operation; no family
//! word selects the store that keeps it.
use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request, UsageContext,
    UsageReason, check_scope, close_generation_option, missing_required, unexpected_argument,
    usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::{WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration};

/// `operation resume`'s fence and wait. The wait bounds only a Project or Worktree resume; a close
/// resume returns once it is submitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Resume {
    pub(crate) generation: WorkbenchTabCloseAttemptGeneration,
    pub(crate) detach: bool,
    pub(crate) timeout_ms: u32,
}

pub(super) fn parse(
    verb: &str,
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let resume = verb == "resume";
    let context = UsageContext::Path(if resume {
        "operation resume"
    } else {
        "operation status"
    });
    check_scope(options, context, envelope)?;
    let [value] = operands else {
        return Err(if operands.is_empty() {
            missing_required(context, "<request-id>", envelope)
        } else {
            unexpected_argument(context, operands[1], envelope)
        });
    };
    let operation_id = WorkbenchRequestId::parse(value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "<request-id>",
                value: (*value).to_owned(),
                expected: ExpectedValue::OperationId,
            },
            envelope,
        )
    })?;
    let resume = if resume {
        if !options.confirm {
            return Err(missing_required(context, "'--confirm'", envelope));
        }
        Some(Resume {
            generation: close_generation_option(options, context, envelope)?,
            detach: options.detach,
            timeout_ms: if options.timeout.is_some() {
                wait_timeout_option(options, context, envelope)?
            } else {
                120_000
            },
        })
    } else {
        None
    };
    Ok(Request::Operation {
        operation_id,
        resume,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests {
    use crate::invocation::parse_request;
    use std::ffi::OsString;

    fn parses(words: &[&str]) -> bool {
        parse_request(words.iter().map(OsString::from)).is_ok()
    }

    #[test]
    fn status_takes_only_the_request_identity() {
        let id = "00000000000000000000000000000001";
        assert!(parses(&["operation", "status", id]));
        assert!(!parses(&["operation", "status"]));
        assert!(!parses(&["operation", "status", "@workspace:0x1"]));
        assert!(!parses(&["operation", "status", id, "--workspace", id]));
    }

    #[test]
    fn resume_requires_confirmation_and_an_explicit_nonzero_fence() {
        let id = "00000000000000000000000000000001";
        assert!(parses(&[
            "operation",
            "resume",
            id,
            "--generation",
            "1",
            "--confirm"
        ]));
        assert!(!parses(&["operation", "resume", id, "--confirm"]));
        assert!(!parses(&["operation", "resume", id, "--generation", "1"]));
        assert!(!parses(&[
            "operation",
            "resume",
            id,
            "--generation",
            "0",
            "--confirm"
        ]));
    }
}
