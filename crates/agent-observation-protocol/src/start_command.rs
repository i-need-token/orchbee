//! The closed catalog of startable interactive Agents: the command an Agent start runs for each
//! canonical provider id, its fixed arguments, and the option through which a new conversation
//! gets an id the starter chose. Clients and the Server read the same entries, so a typed start
//! and a shell's start command run the same executable with the same arguments, and both decide
//! alike which providers a search-path lookup can check.

use crate::{AgentAliasClass, AgentReportProvider};

/// Every startable provider in the catalog's stable order.
pub const STARTABLE_AGENT_PROVIDER_IDS: &[&str] = &[
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
    "jcode",
    "openclaw",
    "prime-agent",
];

/// One startable provider, admitted only by its exact canonical id; aliases and report labels
/// select nothing.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StartableAgent(&'static str);

impl StartableAgent {
    #[must_use]
    pub fn parse(canonical: &str) -> Option<Self> {
        let known = AgentReportProvider::from_native_provider_id(canonical)?.native_provider_id();
        STARTABLE_AGENT_PROVIDER_IDS
            .iter()
            .find(|id| **id == known && known == canonical)
            .map(|id| Self(id))
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        self.0
    }

    /// The command name a start resolves on the shell's own search path.
    #[must_use]
    pub fn executable(self) -> &'static str {
        match self.0 {
            "claude-code" => "claude",
            "cursor" => "agent",
            "qodercli" => "qoder",
            "antigravity" => "agy",
            "github-copilot" => "copilot",
            "kiro" => "kiro-cli",
            "qwen-code" => "qwen",
            "minimax-code" => "mcode",
            id => id,
        }
    }

    /// Whether finding [`executable`](Self::executable) on a search path says the provider is
    /// installed. A command the registry classes as generic for any provider, such as Cursor's
    /// `agent` or `snow`, may belong to another program, so such a provider can be started but
    /// not checked, and its command is never looked up.
    #[must_use]
    pub fn checkable(self) -> bool {
        let command = self.executable();
        !AgentReportProvider::ALL.iter().any(|known| {
            known
                .registry_entry()
                .aliases
                .iter()
                .any(|alias| alias.name == command && alias.class == AgentAliasClass::Generic)
        })
    }

    /// The fixed arguments that open the provider's interactive session when its bare command
    /// does not, such as OpenClaw's `chat`; caller arguments follow them.
    #[must_use]
    pub fn launch_arguments(self) -> &'static [&'static str] {
        match self.0 {
            "openclaw" => &["chat"],
            _ => &[],
        }
    }

    /// The option through which the provider opens a new conversation under an id its starter
    /// chose, for the providers that document one. The Session reads that id back from the
    /// Agent process's command line, so it is stored nowhere else.
    #[must_use]
    pub fn session_option(self) -> Option<&'static str> {
        self.session_options().map(|_| "--session-id")
    }

    /// Whether caller arguments already choose a conversation for a provider with a launch
    /// session option: an id, a resume, a continue of the latest one, a fork, or none at all.
    /// Options match before `=`, and nothing after `--` is an option.
    #[must_use]
    pub fn chooses_session(self, args: &[String]) -> bool {
        let Some(options) = self.session_options() else {
            return false;
        };
        args.iter()
            .take_while(|arg| *arg != "--")
            .filter(|arg| arg.starts_with('-'))
            .map(|arg| arg.split_once('=').map_or(arg.as_str(), |(name, _)| name))
            .any(|name| options.contains(&name))
    }

    /// The arguments after the command for a start without caller arguments: the fixed
    /// arguments, then the session option and `session` when the provider takes one.
    #[must_use]
    pub fn start_arguments(self, session: Option<&str>) -> Vec<String> {
        let mut arguments: Vec<String> = self
            .launch_arguments()
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect();
        if let (Some(option), Some(session)) = (self.session_option(), session) {
            arguments.extend([option.to_owned(), session.to_owned()]);
        }
        arguments
    }

    /// Every option through which a provider with a launch session option selects its
    /// conversation.
    fn session_options(self) -> Option<&'static [&'static str]> {
        Some(match self.0 {
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
}

#[cfg(test)]
mod tests;
