//! Policy and preparation adapter for the names owner's launch transitions.

use std::time::Instant;

use orchbee_agent_observation_protocol::{
    AgentLaunchOutcome, AgentLaunchOutcomeEnvelope, AgentLaunchRejectionReason, AgentLaunchRequest,
    AgentLaunchResult, AgentUnavailableReason,
};

use super::{AgentRuntimeSnapshot, AgentService, PreparedUnavailable};

#[cfg(test)]
mod tests;

impl AgentService {
    pub(crate) fn execute_launch_prepared(
        &self,
        request: AgentLaunchRequest,
        prepare: impl FnOnce() -> Result<AgentRuntimeSnapshot, PreparedUnavailable>,
    ) -> AgentLaunchResult {
        let inspect = matches!(
            request,
            AgentLaunchRequest::Inspect { .. } | AgentLaunchRequest::Status { .. }
        );
        let allowed = if inspect {
            self.policy.read
        } else {
            self.policy.manage
        };
        let unavailable = || AgentLaunchOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        };
        let outcome = if !allowed {
            AgentLaunchOutcome::Rejected {
                reason: if inspect {
                    AgentLaunchRejectionReason::ReadDenied
                } else {
                    AgentLaunchRejectionReason::ManageDenied
                },
            }
        } else if std::panic::catch_unwind(std::panic::AssertUnwindSafe(prepare))
            .unwrap_or(Err(PreparedUnavailable))
            .is_err()
        {
            unavailable()
        } else {
            self.with_state(|state| {
                if state.runtime.is_disposed() {
                    return unavailable();
                }
                let now = state.launch_now_ms(Instant::now());
                state.names.execute_launch(
                    &request,
                    state.runtime.snapshot(),
                    state.native.bindings(),
                    now,
                )
            })
            .unwrap_or_else(|_| unavailable())
        };
        let result = AgentLaunchOutcomeEnvelope { outcome };
        let request_id = request.request_id().clone();
        match request {
            AgentLaunchRequest::Status { .. } => AgentLaunchResult::Status { request_id, result },
            AgentLaunchRequest::Reserve { .. } => AgentLaunchResult::Reserve { request_id, result },
            AgentLaunchRequest::Inspect { .. } => AgentLaunchResult::Inspect { request_id, result },
            AgentLaunchRequest::Complete { .. } => {
                AgentLaunchResult::Complete { request_id, result }
            }
            AgentLaunchRequest::Cancel { .. } => AgentLaunchResult::Cancel { request_id, result },
        }
    }
}
