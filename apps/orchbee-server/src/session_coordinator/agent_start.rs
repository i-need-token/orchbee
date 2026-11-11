//! Resolves a Workbench Agent start request into the first command of a new shell.
use orchbee_capability_node_protocol::{AgentStartCommand, ProtocolError};
use orchbee_workbench_protocol::AgentStartSelector;

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
