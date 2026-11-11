//! Resolves a Workbench Agent start request into the first command of a new shell, names the start
//! a settled creation leaves for the Server to deliver, publishes that start's failure fact, and
//! records the Session's Agent starts in its catalog's Recent list. Whether and when anything is
//! typed belongs to the start delivery.
use super::SessionCoordinator;
use orchbee_capability_node_protocol::{AgentStartCommand, CapabilityNodeMessage, ProtocolError};
use orchbee_workbench_protocol::{
    AgentStartFailureReason, AgentStartSelector, PresentationBinding, TerminalPresentationId,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// The largest Unix millisecond time the Workbench protocol carries.
const MAXIMUM_UNIX_MILLISECONDS: u64 = 9_007_199_254_740_991;

/// The Agent start of a creation whose terminal started: the Presentation and the binding the
/// creation committed, the provider it named and the command its new shell received.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStartLaunch {
    pub(crate) presentation: TerminalPresentationId,
    pub(crate) binding: PresentationBinding,
    pub(crate) provider: AgentStartSelector,
    pub(crate) command: AgentStartCommand,
}

/// The start `selector` named, as the launch `command` attached it; nothing when the creation
/// named none or its launch carries no start.
pub(super) fn launch_of(
    selector: Option<AgentStartSelector>,
    command: &CapabilityNodeMessage,
    presentation: &TerminalPresentationId,
    binding: &PresentationBinding,
) -> Option<AgentStartLaunch> {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = command else {
        return None;
    };
    Some(AgentStartLaunch {
        presentation: presentation.clone(),
        binding: binding.clone(),
        provider: selector?,
        command: launch.agent_start()?.clone(),
    })
}

impl SessionCoordinator {
    /// Publishes, in one Workbench revision on the launch's Presentation while it is still bound as
    /// the creation committed it, that its Agent start did not happen for `reason`, stamped now.
    /// Returns whether a fact was published.
    pub(crate) fn publish_agent_start_failure(
        &mut self,
        launch: &AgentStartLaunch,
        reason: AgentStartFailureReason,
    ) -> bool {
        let (Some(workbench), Some(at)) = (self.workbench.as_mut(), unix_milliseconds()) else {
            return false;
        };
        workbench.record_agent_start_failure(
            &launch.presentation,
            &launch.binding,
            launch.provider,
            reason,
            at,
        )
    }
}

/// Now in Unix milliseconds, when the clock reads within the range the Workbench protocol carries.
fn unix_milliseconds() -> Option<u64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(elapsed.as_millis())
        .ok()
        .filter(|milliseconds| (1..=MAXIMUM_UNIX_MILLISECONDS).contains(milliseconds))
}

/// The catalog command for `selector`, with a fresh conversation id for a provider that takes
/// one at launch, so a later resume can name the conversation. Entropy failure starts without an
/// id rather than with a predictable one.
pub(super) fn agent_start_command(
    selector: &AgentStartSelector,
) -> Result<AgentStartCommand, ProtocolError> {
    let agent = selector.agent();
    let session = agent.session_option().and_then(|_| conversation_id());
    AgentStartCommand::new(
        agent.executable().to_owned(),
        agent.start_arguments(session.as_deref()),
    )
}

impl SessionCoordinator {
    /// Moves `agent` to the front of Recent: a creation starting it was accepted. A Session without
    /// a catalog records nothing.
    pub(super) fn record_agent_start(&self, agent: AgentStartSelector) {
        if let Some(catalog) = &self.agent_catalog {
            catalog.started(agent);
        }
    }

    /// A run of `agent` was first published, typed by hand or not: it moves to the front of Recent
    /// and its catalog row reads found, since the run proves its command starts.
    pub(super) fn record_agent_run(&self, agent: AgentStartSelector) {
        if let Some(catalog) = &self.agent_catalog {
            catalog.started(agent);
            catalog.mark_found(agent.agent());
        }
    }
}

/// A random version-4 UUID in lowercase hyphenated form.
fn conversation_id() -> Option<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = format!("{:032x}", u128::from_be_bytes(bytes));
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}
