//! Explicit group confirmation; retained reads and resumes use `project status|resume`.
use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request, Resource, Target,
    UsageContext, UsageReason, check_scope, missing_required, target, usage_with,
    wait_timeout_option,
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
    let context = UsageContext::Path("project close-all");
    check_scope(options, context, envelope)?;
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
            target: target(value.to_owned(), Resource::Project, envelope)?,
            operation_id,
            detach: options.detach,
            timeout_ms: timeout(options, context, envelope)?,
        },
        options,
    ))
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
