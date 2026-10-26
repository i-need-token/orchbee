//! Frozen plugin commands and generation-fenced, bounded execution receipts.
use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerId};

pub const MAXIMUM_PLUGIN_OUTPUT_BYTES: usize = 64 * 1024;
pub const MAXIMUM_PLUGIN_LOG_PAGE: usize = 4;
pub const MAXIMUM_PLUGIN_INTENT_BYTES: usize = 160 * 1024;
const MAX_INTEGER: u64 = 9_007_199_254_740_991;

/// A frozen user-configured Windows command, with no plugin identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredCommandIntent {
    pub command_id: String,
    pub directory: String,
    pub command: String,
    pub environment: Vec<(String, String)>,
}
impl ConfiguredCommandIntent {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        identifier(&self.command_id, 120)?;
        text(&self.directory, 4096)?;
        text(&self.command, 4096)?;
        if self.command.trim().is_empty()
            || !crate::terminal_launch::is_absolute_path(self.directory.as_bytes())
        {
            return Err(ProtocolError::InvalidPayload);
        }
        environment(&self.environment)
    }
}

fn environment(entries: &[(String, String)]) -> Result<(), ProtocolError> {
    if entries.len() > 32 {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut names = std::collections::BTreeSet::new();
    let mut bytes = 0;
    for (name, value) in entries {
        bytes += name.len() + value.len();
        if name.len() > 120
            || !name.starts_with("ORCHBEE_")
            || !name
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            || !names.insert(name)
            || value.len() > 16 * 1024
            || value.contains('\0')
        {
            return Err(ProtocolError::InvalidPayload);
        }
    }
    if bytes > 128 * 1024 {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginCommandIntent {
    pub plugin_id: String,
    pub command_id: String,
    pub directory: String,
    pub argv: Vec<String>,
    pub environment: Vec<(String, String)>,
}
impl PluginCommandIntent {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        identifier(&self.plugin_id, 120)?;
        identifier(&self.command_id, 241)?;
        text(&self.directory, 4096)?;
        if !crate::terminal_launch::is_absolute_path(self.directory.as_bytes()) {
            return Err(ProtocolError::InvalidPayload);
        }
        argv(&self.argv)?;
        if self.environment.len() > 32 {
            return Err(ProtocolError::InvalidPayload);
        }
        let mut names = std::collections::BTreeSet::new();
        for (name, value) in &self.environment {
            if name.len() > 120
                || !name.starts_with("ORCHBEE_")
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
                || !names.insert(name)
                || value.len() > 16 * 1024
                || value.contains('\0')
            {
                return Err(ProtocolError::InvalidPayload);
            }
        }
        let bytes = self.argv.iter().map(String::len).sum::<usize>()
            + self
                .environment
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
            + self.directory.len()
            + self.plugin_id.len()
            + self.command_id.len();
        if bytes > MAXIMUM_PLUGIN_INTENT_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
}
pub(super) fn identifier(value: &str, maximum: usize) -> Result<(), ProtocolError> {
    if value.is_empty()
        || value.len() > maximum
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(&c))
    {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(())
}
fn text(value: &str, cap: usize) -> Result<(), ProtocolError> {
    if value.is_empty() || value.len() > cap || value.contains('\0') {
        Err(ProtocolError::InvalidPayload)
    } else {
        Ok(())
    }
}
fn argv(args: &[String]) -> Result<(), ProtocolError> {
    if args.is_empty()
        || args.len() > 128
        || args.iter().map(String::len).sum::<usize>() > 128 * 1024
    {
        return Err(ProtocolError::InvalidPayload);
    }
    for arg in args {
        text(arg, 4096)?;
    }
    Ok(())
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginJobAction {
    Start(PluginCommandIntent),
    StartConfigured(ConfiguredCommandIntent),
    List {
        plugin_id: Option<String>,
        before: Option<u64>,
        limit: u8,
    },
    Get {
        job_id: OperationId,
    },
    Cancel {
        job_id: OperationId,
    },
}
impl PluginJobAction {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Start(intent) => intent.validate(),
            Self::StartConfigured(intent) => intent.validate(),
            Self::List {
                plugin_id,
                before,
                limit,
            } => {
                if *limit == 0
                    || usize::from(*limit) > MAXIMUM_PLUGIN_LOG_PAGE
                    || before.is_some_and(|v| v == 0 || v > MAX_INTEGER)
                {
                    return Err(ProtocolError::InvalidPayload);
                }
                if let Some(id) = plugin_id {
                    identifier(id, 120)?;
                }
                Ok(())
            }
            Self::Get { .. } | Self::Cancel { .. } => Ok(()),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginJobRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub action: PluginJobAction,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginCommandFailure {
    ExitFailure,
    SpawnUnavailable,
    SpawnFailed,
    Io,
    Cancelled,
    PhysicalUnknown,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginCommandLog {
    pub job_id: OperationId,
    pub sequence: u64,
    pub plugin_id: String,
    pub command_id: String,
    pub command: Vec<String>,
    pub started_ms: u64,
    pub finished_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub failure: Option<PluginCommandFailure>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}
impl PluginCommandLog {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        identifier(&self.plugin_id, 120)?;
        identifier(&self.command_id, 241)?;
        argv(&self.command)?;
        if self.sequence == 0
            || self.sequence > MAX_INTEGER
            || self.started_ms > MAX_INTEGER
            || self
                .finished_ms
                .is_some_and(|v| v < self.started_ms || v > MAX_INTEGER)
            || self.stdout.len() > MAXIMUM_PLUGIN_OUTPUT_BYTES
            || self.stderr.len() > MAXIMUM_PLUGIN_OUTPUT_BYTES
            || (self.stdout_truncated && self.stdout.len() != MAXIMUM_PLUGIN_OUTPUT_BYTES)
            || (self.stderr_truncated && self.stderr.len() != MAXIMUM_PLUGIN_OUTPUT_BYTES)
        {
            return Err(ProtocolError::InvalidPayload);
        }
        match self.finished_ms {
            None if self.exit_code.is_none()
                && self.failure.is_none()
                && self.stdout.is_empty()
                && self.stderr.is_empty()
                && !self.stdout_truncated
                && !self.stderr_truncated =>
            {
                Ok(())
            }
            Some(_) => match self.failure {
                None if self.exit_code == Some(0) => Ok(()),
                Some(PluginCommandFailure::ExitFailure) if self.exit_code != Some(0) => Ok(()),
                Some(reason)
                    if reason != PluginCommandFailure::ExitFailure && self.exit_code.is_none() =>
                {
                    Ok(())
                }
                // A finished log must carry success, an exit failure, or a failure without exit.
                _ => Err(ProtocolError::InvalidPayload),
            },
            // An unfinished log cannot carry completion data or captured output.
            _ => Err(ProtocolError::InvalidPayload),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginJobRefusal {
    RejectedFencing,
    InvalidIntent,
    Busy,
    Conflict,
    Unavailable,
    PhysicalUnknown,
    NotFound,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginJobOutcome {
    ConfiguredStarted {
        job_id: OperationId,
        existing: bool,
    },
    Started {
        log: Box<PluginCommandLog>,
        existing: bool,
    },
    Logs {
        logs: Vec<PluginCommandLog>,
        next_before: Option<u64>,
    },
    Cancellation {
        already_complete: bool,
    },
    Refused(PluginJobRefusal),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginJobResult {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub outcome: PluginJobOutcome,
}
impl PluginJobResult {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match &self.outcome {
            PluginJobOutcome::Started { log, .. } => {
                log.validate()?;
                if log.job_id != self.operation_id {
                    return Err(ProtocolError::InvalidPayload);
                }
            }
            PluginJobOutcome::ConfiguredStarted { job_id, .. } => {
                if job_id != &self.operation_id {
                    return Err(ProtocolError::InvalidPayload);
                }
            }
            PluginJobOutcome::Logs { logs, next_before } => {
                if logs.len() > MAXIMUM_PLUGIN_LOG_PAGE
                    || logs.windows(2).any(|p| p[0].sequence <= p[1].sequence)
                    || next_before.is_some_and(|v| logs.last().is_none_or(|log| log.sequence != v))
                {
                    return Err(ProtocolError::InvalidPayload);
                }
                for log in logs {
                    log.validate()?;
                }
            }
            PluginJobOutcome::Cancellation { .. } | PluginJobOutcome::Refused(_) => {}
        }
        Ok(())
    }
    pub fn answers(&self, request: &PluginJobRequest) -> bool {
        let refused = matches!(&self.outcome, PluginJobOutcome::Refused(_));
        self.operation_id == request.operation_id
            && self.worker_id == request.worker_id
            && self.worker_generation == request.worker_generation
            && self.validate().is_ok()
            && match &request.action {
                PluginJobAction::StartConfigured(_) => {
                    matches!(&self.outcome, PluginJobOutcome::ConfiguredStarted { job_id, .. }
                        if *job_id == request.operation_id)
                        || refused
                }
                PluginJobAction::Start(intent) => {
                    matches!(&self.outcome, PluginJobOutcome::Started { log, .. }
                        if log.plugin_id == intent.plugin_id
                            && log.command_id == intent.command_id
                            && log.command == intent.argv)
                        || refused
                }
                PluginJobAction::List {
                    plugin_id,
                    before,
                    limit,
                } => {
                    matches!(&self.outcome, PluginJobOutcome::Logs { logs, .. }
                    if logs.len() <= usize::from(*limit)
                        && logs.iter().all(|log| {
                        plugin_id.as_ref().is_none_or(|id| *id == log.plugin_id)
                            && before.is_none_or(|v| log.sequence < v)
                    })) || refused
                }
                PluginJobAction::Get { job_id } => {
                    matches!(&self.outcome, PluginJobOutcome::Logs { logs, next_before }
                        if next_before.is_none()
                            && logs.len() <= 1
                            && logs.iter().all(|log| log.job_id == *job_id))
                        || refused
                }
                PluginJobAction::Cancel { .. } => {
                    matches!(&self.outcome, PluginJobOutcome::Cancellation { .. }) || refused
                }
            }
    }
}

#[cfg(test)]
mod tests;
