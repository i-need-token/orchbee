//! Recognized interactive Agent identities and their canonical command names.

pub const CANONICAL_IDS: &[&str] = &[
    "pi",
    "claude-code",
    "codex",
    "gemini",
    "cursor",
    "devin",
    "antigravity",
    "cline",
    "omp",
    "mastracode",
    "opencode",
    "github-copilot",
    "kimi",
    "kiro",
    "droid",
    "amp",
    "grok",
    "hermes",
    "kilo",
    "qodercli",
    "qwen-code",
    "letta",
    "maki",
    "muse",
    "atomcode",
    "codebuddy",
    "command-code",
    "deveco",
    "mimo",
    "reasonix",
    "snow",
    "minimax-code",
    "mothx",
    "kirocrew",
    "qwenpaw",
    "jcode",
    "openclaw",
    "penguin",
    "prime-agent",
];

pub use orchbee_agent_observation_protocol::normalize_agent_report_label as reported_label;
use orchbee_agent_observation_protocol::{AgentAliasClass, AgentReportProvider};

/// Exact canonical provider ID; aliases, report labels, and process names stay absent.
pub fn provider(canonical: &str) -> Option<&'static str> {
    AgentReportProvider::from_native_provider_id(canonical)
        .map(AgentReportProvider::native_provider_id)
}

/// Only canonical identities select a command; aliases remain read-only recognition.
pub fn executable(canonical: &str) -> Option<&'static str> {
    let id = provider(canonical).filter(|id| *id == canonical)?;
    Some(match id {
        "claude-code" => "claude",
        "cursor" => "agent",
        "qodercli" => "qoder",
        "antigravity" => "agy",
        "github-copilot" => "copilot",
        "kiro" => "kiro-cli",
        "qwen-code" => "qwen",
        "minimax-code" => "mcode",
        _ => id,
    })
}

/// The fixed arguments that open a provider's interactive session when its bare command does
/// not; caller arguments follow them.
pub fn launch_arguments(canonical: &str) -> &'static [&'static str] {
    match provider(canonical) {
        Some("kirocrew" | "openclaw" | "penguin") => &["chat"],
        Some("qwenpaw") => &["tui"],
        _ => &[],
    }
}

/// The option through which a provider opens a new conversation under an id its caller chose,
/// for the providers that document one. An Agent start passes it a fresh id after the fixed
/// arguments unless the caller's own arguments already choose a conversation; the Session reads
/// that id back from the Agent process's command line, so it is stored nowhere else.
pub(crate) fn launch_session_option(canonical: &str) -> Option<&'static str> {
    session_options(canonical).map(|_| "--session-id")
}

/// Whether the caller's arguments already choose a conversation for a provider with a launch
/// session option: an id, a resume, a continue of the latest one, a fork, or none at all. Options
/// match before `=`, and nothing after `--` is an option.
pub(crate) fn chooses_session(canonical: &str, args: &[String]) -> bool {
    let Some(options) = session_options(canonical) else {
        return false;
    };
    args.iter()
        .take_while(|arg| *arg != "--")
        .filter(|arg| arg.starts_with('-'))
        .map(|arg| arg.split_once('=').map_or(arg.as_str(), |(name, _)| name))
        .any(|name| options.contains(&name))
}

/// Every option through which a provider with a launch session option selects its conversation.
fn session_options(canonical: &str) -> Option<&'static [&'static str]> {
    Some(match provider(canonical)? {
        "grok" => &[
            "-s",
            "--session-id",
            "-r",
            "--resume",
            "-c",
            "--continue",
            "--load",
            "--fork-session",
        ],
        "gemini" => &["--session-id", "-r", "--resume"],
        "qwen-code" => &["--session-id", "-r", "--resume", "-c", "--continue"],
        "github-copilot" => &["--session-id", "-r", "--resume", "--continue"],
        "pi" => &[
            "--session-id",
            "--session",
            "-r",
            "--resume",
            "-c",
            "--continue",
            "--no-session",
        ],
        _ => return None,
    })
}

/// A command that the registry classes as generic, such as `agent` or `snow`, is not enough
/// evidence to advertise a provider as installed.
pub(crate) fn detection_executable(canonical: &str) -> Option<&'static str> {
    let command = executable(canonical)?;
    let generic = AgentReportProvider::ALL.iter().any(|provider| {
        provider
            .registry_entry()
            .aliases
            .iter()
            .any(|alias| alias.name == command && alias.class == AgentAliasClass::Generic)
    });
    (!generic).then_some(command)
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
