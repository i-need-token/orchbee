//! Windows execution defaults apply only when the shared grammar retained an omitted option.
use crate::invocation::AgentWaitCondition::{self, Blocked, Done, Idle};

pub(super) const DEFAULT_CONDITIONS: &[AgentWaitCondition] = &[Idle, Done, Blocked];
pub(super) const DEFAULT_TIMEOUT: Option<u32> = None;
