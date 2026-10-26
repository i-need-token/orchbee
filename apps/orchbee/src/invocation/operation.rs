//! Shared request identity grammar; resource creation and operation observation stay separate.
use super::{
    ClientCommand, ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request,
    UsageContext, UsageReason, WorkspaceGroupCloseCommand, check_scope, close_generation_option,
    missing_required, unexpected_argument, usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::WorkbenchRequestId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseFamily {
    Workspace,
    Tab,
    Panel,
}

impl CloseFamily {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Tab => "tab",
            Self::Panel => "panel",
        }
    }
}

pub(super) fn parse(
    family: &str,
    verb: &str,
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path(match (family, verb) {
        ("workspace", "status") => "workspace status",
        ("workspace", _) => "workspace resume",
        ("tab", "status") => "tab status",
        ("tab", _) => "tab resume",
        ("panel", "status") => "panel status",
        ("panel", _) => "panel resume",
        ("project", "status") => "project status",
        ("project", _) => "project resume",
        ("client", _) => "client status",
        _ => "server status",
    });
    let resume = verb == "resume";
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
                expected: ExpectedValue::CloseRequestId,
            },
            envelope,
        )
    })?;
    let generation = if resume {
        if !options.confirm {
            return Err(missing_required(context, "'--confirm'", envelope));
        }
        Some(close_generation_option(options, context, envelope)?)
    } else {
        None
    };
    let format = options.format.unwrap_or(OutputFormat::Text);
    Ok(match family {
        "client" => Request::Client {
            command: ClientCommand::ControlStatus(operation_id),
            format,
        },
        "server" => Request::ServerOperationStatus {
            operation_id,
            format,
        },
        "project" => {
            let command = if let Some(generation) = generation {
                let expected_attempt = u32::try_from(generation.value())
                    .ok()
                    .filter(|n| *n <= 64)
                    .ok_or_else(|| {
                        usage_with(
                            UsageReason::InvalidValue {
                                context,
                                option: "--generation",
                                value: generation.value().to_string(),
                                expected: ExpectedValue::CloseGeneration,
                            },
                            envelope,
                        )
                    })?;
                WorkspaceGroupCloseCommand::Resume {
                    operation_id,
                    expected_attempt,
                    detach: options.detach,
                    timeout_ms: if options.timeout.is_some() {
                        wait_timeout_option(options, context, envelope)?
                    } else {
                        120_000
                    },
                }
            } else {
                WorkspaceGroupCloseCommand::Status { operation_id }
            };
            Request::WorkspaceGroupClose { command, format }
        }
        _ => Request::RetainedClose {
            family: match family {
                "tab" => CloseFamily::Tab,
                "panel" => CloseFamily::Panel,
                _ => CloseFamily::Workspace,
            },
            operation_id,
            generation,
            format,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invocation::parse_request;
    use std::ffi::OsString;

    fn request(words: &[&str]) -> Result<Request, ParseFailure> {
        parse_request(words.iter().map(OsString::from))
    }

    #[test]
    fn every_status_takes_the_receipt_identity_without_resource_selectors() {
        for family in [
            "workspace",
            "tab",
            "panel",
            "project",
            "client",
            "server",
            "worktree",
        ] {
            let words = [family, "status", "00000000000000000000000000000001"];
            assert!(request(&words).is_ok(), "{words:?}");
            assert!(request(&[family, "status"]).is_err());
            assert!(request(&[family, "status", "@workspace:0x1"]).is_err());
            assert!(request(&[family, "status", words[2], "--workspace", words[2]]).is_err());
        }
    }

    #[test]
    fn resumes_require_confirmation_and_an_explicit_nonzero_fence() {
        for family in ["workspace", "tab", "panel", "project", "worktree"] {
            let id = "00000000000000000000000000000001";
            assert!(request(&[family, "resume", id, "--generation", "1", "--confirm"]).is_ok());
            assert!(request(&[family, "resume", id, "--confirm"]).is_err());
            assert!(request(&[family, "resume", id, "--generation", "1"]).is_err());
            assert!(request(&[family, "resume", id, "--generation", "0", "--confirm"]).is_err());
        }
    }
}
