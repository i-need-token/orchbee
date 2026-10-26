//! Prompt composition retains delivery evidence independently of observation.
//! The shared terminal send owner alone constructs input and delivery receipts.

use super::{Output, failure_output};
use crate::invocation::AgentTarget;
use crate::invocation::{AgentPromptWait, OutputFormat};
use crate::localization::Locale;

mod workflow;

struct ResolvedWait {
    client: Option<orchbee_workbench_protocol::WorkbenchClientId>,
    until: Vec<crate::invocation::AgentWaitCondition>,
    timeout_ms: Option<u32>,
}

pub(super) fn run(
    target: &AgentTarget,
    text: &str,
    wait: Option<&AgentPromptWait>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::AgentPrompt) {
        return failure_output(failure, Some(format), locale);
    }
    let wait = wait.map(|request| ResolvedWait {
        client: request.client.clone(),
        until: super::agent_request::wait_conditions(request.until.as_deref()).to_vec(),
        timeout_ms: super::agent_request::wait_timeout(request.timeout_ms),
    });
    workflow::run(target, text, wait.as_ref(), format, locale)
}
