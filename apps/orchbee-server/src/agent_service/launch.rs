//! Policy and preparation adapter for the names owner's launch transitions.

use std::collections::BTreeMap;
use std::time::Instant;

use orchbee_agent_observation_protocol::{
    AgentLaunchOutcome, AgentLaunchOutcomeEnvelope, AgentLaunchRejectionReason, AgentLaunchRequest,
    AgentLaunchResult, AgentUnavailableReason,
};

use super::names::{LaunchEvidence, ObservedCommand};
use super::{AgentRuntimeSnapshot, AgentService, PreparedUnavailable};

#[cfg(test)]
mod tests;

impl AgentService {
    /// `observed` reads the latest shell-echoed command per Host Terminal outside this owner's
    /// lock; Reserve also mints the run token here, before any reservation mutation.
    pub(crate) fn execute_launch_prepared(
        &self,
        request: AgentLaunchRequest,
        prepare: impl FnOnce() -> Result<AgentRuntimeSnapshot, PreparedUnavailable>,
        observed: impl FnOnce() -> Vec<ObservedCommand>,
    ) -> AgentLaunchResult {
        self.execute_launch_guarded(request, |request| {
            prepare()?;
            let evidence = LaunchEvidence {
                run_token: matches!(request, AgentLaunchRequest::Reserve { .. })
                    .then(mint_run_token)
                    .flatten(),
                commands: observed(),
                session_tokens: BTreeMap::new(),
            };
            self.fold_launch(request, evidence)
        })
    }

    fn execute_launch_guarded(
        &self,
        request: AgentLaunchRequest,
        fold: impl FnOnce(&AgentLaunchRequest) -> Result<AgentLaunchOutcome, PreparedUnavailable>,
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
        let outcome = if !allowed {
            AgentLaunchOutcome::Rejected {
                reason: if inspect {
                    AgentLaunchRejectionReason::ReadDenied
                } else {
                    AgentLaunchRejectionReason::ManageDenied
                },
            }
        } else {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fold(&request)))
                .ok()
                .and_then(Result::ok)
                .unwrap_or(AgentLaunchOutcome::Unavailable {
                    reason: AgentUnavailableReason::RuntimeUnavailable,
                })
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

    fn fold_launch(
        &self,
        request: &AgentLaunchRequest,
        mut evidence: LaunchEvidence,
    ) -> Result<AgentLaunchOutcome, PreparedUnavailable> {
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                return Err(PreparedUnavailable);
            }
            evidence.session_tokens = state
                .ingress
                .run_tokens()
                .map(|(session, token)| (session.to_owned(), token.clone()))
                .collect();
            let now = state.launch_now_ms(Instant::now());
            Ok(state.names.execute_launch(
                request,
                state.runtime.snapshot(),
                state.native.bindings(),
                now,
                &evidence,
            ))
        })
        .map_err(|_| PreparedUnavailable)?
    }
}

/// A fresh, never-reused run token from OS entropy; without entropy Reserve is unavailable.
fn mint_run_token() -> Option<orchbee_agent_observation_protocol::AgentRunToken> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    orchbee_agent_observation_protocol::AgentRunToken::parse(&format!(
        "{:032x}",
        u128::from_be_bytes(bytes)
    ))
    .ok()
}
