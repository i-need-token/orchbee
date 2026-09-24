//! Explicit group confirmation and identity-based retained reads; ordinary Close grammar stays separate.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, Target, UsageContext, UsageReason, check_scope, close_generation_option,
    close_request_option, missing_required, target, usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::WorkbenchRequestId;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceGroupCloseCommand {
    Start {
        target: Target,
        operation_id: Option<WorkbenchRequestId>,
        detach: bool,
        timeout_ms: u32,
    },
    Status {
        operation_id: WorkbenchRequestId,
    },
    Resume {
        operation_id: WorkbenchRequestId,
        expected_attempt: u32,
        detach: bool,
        timeout_ms: u32,
    },
}
pub(super) fn start(
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("workspace close");
    check_scope(
        options,
        Scope {
            group: true,
            confirm: true,
            request_id: true,
            detach: true,
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    if !options.confirm {
        return Err(missing_required(context, "'--confirm'", envelope));
    }
    let operation_id = options
        .request_id
        .as_ref()
        .map(|value| {
            WorkbenchRequestId::parse(value).map_err(|_| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--request-id",
                        value: value.clone(),
                        expected: ExpectedValue::WorktreeOperationId,
                    },
                    envelope,
                )
            })
        })
        .transpose()?;
    Ok(request(
        WorkspaceGroupCloseCommand::Start {
            target: target(value.to_owned(), Resource::Workspace, envelope)?,
            operation_id,
            detach: options.detach,
            timeout_ms: timeout(options, context, envelope)?,
        },
        options,
    ))
}
pub(super) fn retained(
    verb: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let resume = verb == "close-resume";
    let context = UsageContext::Path(if resume {
        "workspace close-resume"
    } else {
        "workspace close-status"
    });
    check_scope(
        options,
        Scope {
            group: true,
            request: true,
            confirm: resume,
            generation: resume,
            detach: resume,
            timeout: resume,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let operation_id = close_request_option(options, context, envelope)?;
    let command = if resume {
        if !options.confirm {
            return Err(missing_required(context, "'--confirm'", envelope));
        }
        let generation = close_generation_option(options, context, envelope)?.value();
        if generation > 64 {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--generation",
                    value: generation.to_string(),
                    expected: ExpectedValue::WorktreeAttempt,
                },
                envelope,
            ));
        }
        WorkspaceGroupCloseCommand::Resume {
            operation_id,
            expected_attempt: generation as u32,
            detach: options.detach,
            timeout_ms: timeout(options, context, envelope)?,
        }
    } else {
        WorkspaceGroupCloseCommand::Status { operation_id }
    };
    Ok(request(command, options))
}
fn request(command: WorkspaceGroupCloseCommand, options: &Options) -> Request {
    Request::WorkspaceGroupClose {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    }
}
fn timeout(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u32, ParseFailure> {
    if options.timeout.is_some() {
        wait_timeout_option(options, context, envelope)
    } else {
        Ok(120_000)
    }
}
