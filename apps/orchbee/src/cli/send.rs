//! `terminal send`: resolve one bound terminal from the snapshot, then submit
//! exactly one `terminal.send` request on the same admitted session. The
//! Server authorizes the current binding during this admission and drives the
//! temporary input-only attach → input → detach sequence; the CLI never takes
//! a TUI controller or resize lease and never retries an ambiguous
//! answer — an uncertain outcome surfaces as `outcome-unknown`, not a replay.
//! Windows Agent targets first use a transient read viewer for process preflight.

use super::Output;
use crate::invocation::{OutputFormat, Target, TerminalSendInput};
use crate::localization::Locale;

pub(crate) const TERMINAL_SEND_ID: &str = "orchbee.terminal.send";
pub(crate) use crate::agent_runtime::SendVerdict;
mod workflow;
#[cfg(all(test, any(target_os = "linux", windows)))]
pub(super) use workflow::test_receipt;
pub(crate) use workflow::{
    Receipt, receipt_output, submit_prompt_on_session, submit_prompt_on_session_cancellable,
};

enum SendTarget<'a> {
    Terminal(&'a Target),
    Agent(&'a crate::invocation::AgentTarget),
}

pub(super) fn run(
    target: &Target,
    input: &TerminalSendInput,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    workflow::run(SendTarget::Terminal(target), input, format, locale)
}

pub(super) fn run_agent(
    target: &crate::invocation::AgentTarget,
    keys: &[orchbee_client_server_protocol::SendKeyStroke],
    format: OutputFormat,
    locale: Locale,
) -> Output {
    workflow::run(
        SendTarget::Agent(target),
        &TerminalSendInput::Keys(keys.to_vec()),
        format,
        locale,
    )
}
