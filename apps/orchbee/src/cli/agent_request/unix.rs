//! Unix canonical waits preserve their existing sampled-condition defaults and ten-minute bound.
use crate::invocation::AgentWaitCondition::{
    self, Canceled, Ended, Failed, Idle, Interrupted, NeedsAttention, Succeeded,
};

pub(super) const DEFAULT_CONDITIONS: &[AgentWaitCondition] = &[
    NeedsAttention,
    Failed,
    Succeeded,
    Canceled,
    Interrupted,
    Idle,
    Ended,
];
pub(super) const DEFAULT_TIMEOUT: Option<u32> = Some(600_000);
