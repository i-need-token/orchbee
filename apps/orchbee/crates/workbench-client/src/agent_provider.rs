//! Recognized interactive Agent identities and their canonical command names. The startable
//! catalog itself lives in `orchbee-agent-observation-protocol`, which the Server reads too.

pub use orchbee_agent_observation_protocol::STARTABLE_AGENT_PROVIDER_IDS as CANONICAL_IDS;
pub use orchbee_agent_observation_protocol::normalize_agent_report_label as reported_label;
use orchbee_agent_observation_protocol::{AgentReportProvider, StartableAgent};

/// Exact canonical provider ID; aliases, report labels, and process names stay absent.
pub fn provider(canonical: &str) -> Option<&'static str> {
    AgentReportProvider::from_native_provider_id(canonical)
        .map(AgentReportProvider::native_provider_id)
}

/// Only canonical identities select a command; aliases remain read-only recognition.
pub fn executable(canonical: &str) -> Option<&'static str> {
    StartableAgent::parse(canonical).map(StartableAgent::executable)
}

/// The fixed arguments that open a provider's interactive session when its bare command does
/// not, such as OpenClaw's `chat`; caller arguments follow them.
pub fn launch_arguments(canonical: &str) -> &'static [&'static str] {
    StartableAgent::parse(canonical).map_or(&[], StartableAgent::launch_arguments)
}

/// The option through which a provider opens a new conversation under an id its caller chose,
/// for the providers that document one. An Agent start passes it a fresh id after the fixed
/// arguments unless the caller's own arguments already choose a conversation; the Session reads
/// that id back from the Agent process's command line, so it is stored nowhere else.
pub(crate) fn launch_session_option(canonical: &str) -> Option<&'static str> {
    StartableAgent::parse(canonical).and_then(StartableAgent::session_option)
}

/// Whether the caller's arguments already choose a conversation for a provider with a launch
/// session option: an id, a resume, a continue of the latest one, a fork, or none at all. Options
/// match before `=`, and nothing after `--` is an option.
pub(crate) fn chooses_session(canonical: &str, args: &[String]) -> bool {
    StartableAgent::parse(canonical).is_some_and(|agent| agent.chooses_session(args))
}

/// One startable provider as a person chooses it: the canonical id a start and the CLI take, the
/// product name every list shows, and the command a start sends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderChoice {
    pub id: &'static str,
    pub product: &'static str,
    pub command: &'static str,
    /// Whether finding `command` on PATH says the provider is installed
    /// ([`StartableAgent::checkable`]): a generic command such as Cursor's `agent` or `snow` can
    /// be started but not checked.
    pub checkable: bool,
}

/// Every startable provider in the catalog's stable order.
pub fn choices() -> impl Iterator<Item = ProviderChoice> {
    CANONICAL_IDS.iter().filter_map(|id| choice(id))
}

/// The catalog entry for a canonical id; aliases and report labels stay absent.
pub fn choice(canonical: &str) -> Option<ProviderChoice> {
    let provider = AgentReportProvider::from_native_provider_id(canonical)?;
    let agent = StartableAgent::parse(canonical)?;
    Some(ProviderChoice {
        id: provider.native_provider_id(),
        product: provider.product_name(),
        command: agent.executable(),
        checkable: agent.checkable(),
    })
}

/// The canonical id a report label or alias names, when it is a startable provider.
pub(crate) fn canonical_id(label: &str) -> Option<&'static str> {
    let id = AgentReportProvider::lookup(label)?.native_provider_id();
    CANONICAL_IDS.contains(&id).then_some(id)
}

/// The line a start types for a canonical id, as a list shows it: the command and its fixed
/// arguments.
pub(crate) fn launch_line(canonical: &str) -> Option<String> {
    let choice = choice(canonical)?;
    Some(
        std::iter::once(choice.command)
            .chain(launch_arguments(canonical).iter().copied())
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// The product name of a canonical id or report label, else the text itself, for what a person
/// reads.
pub fn product_name(label: &str) -> &str {
    AgentReportProvider::lookup(label).map_or(label, |provider| provider.product_name())
}

/// A command that the registry classes as generic is not enough evidence to advertise a provider
/// as installed.
pub(crate) fn detection_executable(canonical: &str) -> Option<&'static str> {
    choice(canonical)
        .filter(|choice| choice.checkable)
        .map(|choice| choice.command)
}

/// The `agent integration --provider` identity that manages a canonical
/// interactive identity, when a managed integration exists for it.
pub fn integration_id(canonical: &str) -> Option<&'static str> {
    let id = match canonical {
        "antigravity" => "antigravity-cli",
        other => other,
    };
    IntegrationId::parse(id).map(|id| id.as_str())
}

pub use orchbee_agent_integration::IntegrationId;

#[cfg(test)]
mod tests;
