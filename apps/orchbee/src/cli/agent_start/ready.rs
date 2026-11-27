//! `agent start --wait-ready`: after a start passes its early check, the started Panel is watched
//! inside the start's own deadline and signal span until its Agent takes a prompt.
use super::{Failure, FailureKind, failure};
use crate::cli::agent_wait::{agent_wait_armed, wait_backoff};
use crate::cli::command_interrupt::ArmedSignals;
use crate::cli::query::AgentQueryError;
use crate::cli::results::Projection;
use crate::invocation::{AgentTarget, AgentWaitCondition, Target};
use orchbee_workbench_protocol::WorkbenchPanelId;
use serde_json::Value;
use std::time::{Duration, Instant};

/// The pause before looking again at a Panel whose Agent is not recognized yet.
const RECOGNITION_RETRY: Duration = Duration::from_millis(250);

/// Wait until the Panel's Agent is idle, or done with a first turn its arguments asked for. A
/// blocked Agent is refused as `agent-blocked`; an Agent not ready by the deadline is
/// `start-deadline`. A wake surfaces as `internal-failure`, which the span's owner arbitrates.
pub(super) fn wait(
    panel: &WorkbenchPanelId,
    deadline: Instant,
    armed: &mut ArmedSignals,
) -> Result<(), Failure> {
    let target = AgentTarget::Panel {
        target: Target::Id(panel.as_str().to_owned()),
    };
    let until = [
        AgentWaitCondition::Idle,
        AgentWaitCondition::Done,
        AgentWaitCondition::Blocked,
    ];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Some(timeout_ms) = u32::try_from(remaining.as_millis())
            .ok()
            .filter(|ms| *ms > 0)
        else {
            return Err(failure(FailureKind::Timeout, "start-deadline"));
        };
        let matched = match agent_wait_armed(&target, &until, timeout_ms, armed) {
            Ok((_, Projection::AgentWait(doc))) => doc.matched,
            Ok((_, Projection::AgentTerminalWait(doc))) => doc.conditions,
            Ok(_) | Err(AgentQueryError::Cancelled(_)) => {
                return Err(Failure::new(FailureKind::InternalFailure));
            }
            Err(AgentQueryError::Admission(read) | AgentQueryError::Read(read)) => {
                if read.kind == FailureKind::Timeout {
                    return Err(failure(FailureKind::Timeout, "start-deadline"));
                }
                if !unrecognized(&read) {
                    return Err(read);
                }
                if wait_backoff(armed.poll_fd(), RECOGNITION_RETRY.min(remaining)) {
                    return Err(Failure::new(FailureKind::InternalFailure));
                }
                continue;
            }
        };
        return if matched.contains(&AgentWaitCondition::Blocked) {
            Err(failure(FailureKind::Rejected, "agent-blocked"))
        } else {
            Ok(())
        };
    }
}

/// The Panel shows no recognized Agent yet: its row is not published, or its terminal reports no
/// running Agent. Both clear once the screen rules recognize the Agent.
fn unrecognized(read: &Failure) -> bool {
    read.kind == FailureKind::NotFound
        && read.details.as_ref().is_some_and(|details| {
            matches!(
                (
                    details.get("stage").and_then(Value::as_str),
                    details.get("reason").and_then(Value::as_str),
                ),
                (Some("agent-projection"), Some("target-not-found"))
                    | (Some("agent-terminal"), Some("agent-not-running"))
            )
        })
}
