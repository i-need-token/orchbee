//! Defines generation-fenced start, poll, and cancellation actions and outcomes for the one
//! Worker-owned Agent probe job, which reports whether each named command is found on the search
//! path a newly started shell would use. Requests carry bare command names only; the Server
//! chooses them and the Worker resolves them.

use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// Commands one probe may name, and results one completed probe carries.
pub const MAXIMUM_AGENT_PROBE_COMMANDS: usize = 64;
/// Bytes of one command name.
pub const MAXIMUM_AGENT_PROBE_COMMAND_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentProbeJobAction {
    Start { commands: Vec<String> },
    Poll { job_id: OperationId },
    Cancel { job_id: OperationId },
}

impl AgentProbeJobAction {
    /// One to 64 command names, each one to 64 bytes of visible ASCII without a quote, so a name
    /// never carries whitespace or quoting that could add an argument.
    pub fn start(commands: Vec<String>) -> Result<Self, ProtocolError> {
        if commands.is_empty()
            || commands.len() > MAXIMUM_AGENT_PROBE_COMMANDS
            || !commands.iter().all(|command| valid_command(command))
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self::Start { commands })
    }

    #[must_use]
    pub fn admits(&self, outcome: &AgentProbeJobOutcome) -> bool {
        match self {
            Self::Start { .. } => matches!(outcome, AgentProbeJobOutcome::Start(_)),
            Self::Poll { .. } => matches!(outcome, AgentProbeJobOutcome::Poll(_)),
            Self::Cancel { .. } => matches!(outcome, AgentProbeJobOutcome::Cancel(_)),
        }
    }
}

fn valid_command(command: &str) -> bool {
    (1..=MAXIMUM_AGENT_PROBE_COMMAND_BYTES).contains(&command.len())
        && command
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b'\'')
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentProbeJobRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub action: AgentProbeJobAction,
}

/// `Busy` is a definite pre-effect refusal: another probe still runs on this Worker.
/// `Unsupported` means this Worker's platform has no probe, so none ever runs there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProbeJobStartOutcome {
    Accepted,
    Busy,
    Unsupported,
    SpawnThreadFailed,
    Dead,
    RejectedFencing,
}

/// The answer for one command, in request order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProbeResult {
    Found,
    Missing,
    /// The query for this command did not finish with an answer: its deadline passed, the probe
    /// was cancelled, or the lookup itself failed.
    Unavailable,
}

/// `Completed` carries one result per requested command, in request order. `Abandoned` means the
/// job ended without results; `Unknown` names a job the owner no longer retains.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentProbeJobPollOutcome {
    Pending,
    Completed { results: Vec<AgentProbeResult> },
    Abandoned,
    Unknown,
    RejectedFencing,
}

impl AgentProbeJobPollOutcome {
    pub fn completed(results: Vec<AgentProbeResult>) -> Result<Self, ProtocolError> {
        if results.is_empty() || results.len() > MAXIMUM_AGENT_PROBE_COMMANDS {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self::Completed { results })
    }
}

/// `Registered` records cooperative cancellation; the job's results stay observable through
/// `Poll`, with every query the cancellation stopped answered `Unavailable`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProbeJobCancelOutcome {
    Registered,
    AlreadyTerminal,
    Unknown,
    RejectedFencing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentProbeJobOutcome {
    Start(AgentProbeJobStartOutcome),
    Poll(AgentProbeJobPollOutcome),
    Cancel(AgentProbeJobCancelOutcome),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentProbeJobResult {
    pub operation_id: OperationId,
    pub job_id: OperationId,
    pub outcome: AgentProbeJobOutcome,
}
