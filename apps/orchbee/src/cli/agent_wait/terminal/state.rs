//! Pure physical wait/activity policy; Client acknowledgements and canonical outcomes are not inferred.
use super::{
    AgentLaunchBinding, AgentSafeInteger, AgentWaitCondition, Failure, FailureKind,
    TerminalAgentRead, WorkbenchRevision, error,
};
use orchbee_workbench_protocol::{
    PanelAgentState as Status, TerminalAgentChange, TerminalAgentObservation,
};

pub(super) struct State {
    binding: AgentLaunchBinding,
    agent: String,
    pub(super) sequence: WorkbenchRevision,
    revision: WorkbenchRevision,
    fence: Option<AgentSafeInteger>,
    until: Vec<AgentWaitCondition>,
    prompt: bool,
    pub(super) active: bool,
}
pub(super) struct Match {
    pub(super) conditions: Vec<AgentWaitCondition>,
    pub(super) change: Option<TerminalAgentChange>,
}
pub(in crate::cli) fn admit(until: &[AgentWaitCondition]) -> Result<(), Failure> {
    if until.iter().any(|c| {
        !matches!(
            c,
            AgentWaitCondition::Idle
                | AgentWaitCondition::Working
                | AgentWaitCondition::Blocked
                | AgentWaitCondition::NeedsAttention
                | AgentWaitCondition::Unknown
                | AgentWaitCondition::Done
                | AgentWaitCondition::Succeeded
                | AgentWaitCondition::Failed
        )
    }) {
        return Err(error(FailureKind::Rejected, "canonical-condition-required"));
    }
    Ok(())
}
fn conditions(until: &[AgentWaitCondition], status: Status) -> Vec<AgentWaitCondition> {
    until
        .iter()
        .copied()
        .filter(|c| {
            matches!(
                (c, status),
                (AgentWaitCondition::Idle, Status::Idle)
                    | (AgentWaitCondition::Done, Status::Done | Status::Error)
                    | (AgentWaitCondition::Succeeded, Status::Done)
                    | (AgentWaitCondition::Failed, Status::Error)
                    | (AgentWaitCondition::Working, Status::Working)
                    | (
                        AgentWaitCondition::Blocked | AgentWaitCondition::NeedsAttention,
                        Status::Blocked
                    )
                    | (AgentWaitCondition::Unknown, Status::Unknown)
            )
        })
        .collect()
}
impl State {
    pub(super) fn new(
        initial: &TerminalAgentRead,
        until: &[AgentWaitCondition],
        fence: Option<AgentSafeInteger>,
        prompt: bool,
    ) -> Result<Self, Failure> {
        let view = &initial.observation.view;
        let agent = view
            .agent
            .clone()
            .ok_or_else(|| error(FailureKind::NotFound, "agent-not-running"))?;
        if fence.is_some_and(|f| f.value() > initial.report_revision.value()) {
            return Err(error(FailureKind::Rejected, "revision-ahead"));
        }
        Ok(Self {
            binding: initial.observation.binding.clone(),
            agent,
            sequence: initial.observation.sequence,
            revision: initial.report_revision,
            fence,
            until: until.to_vec(),
            prompt,
            active: !prompt || view.state == Status::Working,
        })
    }
    pub(super) fn initial_match(&self, read: &TerminalAgentRead) -> Option<Match> {
        (!self.prompt).then(|| self.current_match(read)).flatten()
    }
    fn current_match(&self, read: &TerminalAgentRead) -> Option<Match> {
        if !self.active
            || self
                .fence
                .is_some_and(|f| read.report_revision.value() <= f.value())
        {
            return None;
        }
        let matched = conditions(&self.until, read.observation.view.state);
        (!matched.is_empty()).then_some(Match {
            conditions: matched,
            change: None,
        })
    }
    pub(super) fn observe(&mut self, read: &TerminalAgentRead) -> Result<Option<Match>, Failure> {
        let TerminalAgentObservation {
            binding,
            sequence,
            changes,
            view,
        } = read.observation.as_ref();
        if binding != &self.binding
            || view.agent.as_ref().is_none_or(|a| a != &self.agent)
            || changes
                .iter()
                .any(|c| c.agent.as_ref().is_some_and(|a| a != &self.agent))
        {
            return Err(error(FailureKind::NotFound, "agent-identity-changed"));
        }
        if read.report_revision.value() < self.revision.value()
            || (sequence != &self.sequence && read.report_revision == self.revision)
            || if sequence == &self.sequence {
                !changes.is_empty()
            } else {
                changes.first().is_none_or(|c| {
                    self.sequence.value().checked_add(1) != Some(c.sequence.value())
                }) || changes.last().is_none_or(|c| &c.sequence != sequence)
                    || changes.windows(2).any(|c| {
                        c[0].sequence.value().checked_add(1) != Some(c[1].sequence.value())
                    })
            }
        {
            return Err(error(FailureKind::ProtocolInvalid, "cursor-invalid"));
        }
        let mut historical = None;
        for change in changes {
            let gate_was_closed = !self.active;
            self.active |= matches!(change.state, Status::Working | Status::Blocked);
            if !self.prompt || gate_was_closed && self.active {
                let matched = conditions(&self.until, change.state);
                if !matched.is_empty() {
                    historical = Some(Match {
                        conditions: matched,
                        change: Some(change.clone()),
                    });
                }
            }
        }
        self.sequence = *sequence;
        self.revision = read.report_revision;
        Ok(historical.or_else(|| self.current_match(read)))
    }
}

#[cfg(test)]
mod tests;
