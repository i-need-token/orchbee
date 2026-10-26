//! Compose Client-local attention with canonical Agent facts without changing either owner.
use super::{Output, failure_output};
use crate::{
    invocation::{AgentTarget, OutputFormat, QueryCommand},
    localization::Locale,
};
use orchbee_workbench_protocol::WorkbenchClientId;

pub(in crate::cli) mod workflow;

pub(super) fn run(
    command: &QueryCommand,
    target: &AgentTarget,
    client: Option<&WorkbenchClientId>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::AgentProjection) {
        return failure_output(failure, Some(format), locale);
    }
    workflow::run(command, target, client, format, locale)
}
