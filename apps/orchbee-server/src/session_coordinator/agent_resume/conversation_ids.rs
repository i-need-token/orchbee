//! Where a resumable provider's command line carries a conversation id, as one closed table read
//! in both directions: the argument tokens a resume types after the program, and the options and
//! subcommands through which a running Agent's own command line binds the id it opened. Every typed
//! form is one of the read bindings, so a resumed run reads back the id it was given. It decides no
//! capture: the capture rule admits a read id only for the published run it was sampled from.

use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};

#[cfg(test)]
mod tests;

/// The argument tokens that resume `session`, the only builder of them. The id is bound to a
/// value-taking option or a resume subcommand, so no grammar reads it as a prompt. An option whose
/// value is optional takes `--name=<id>`; a required-value option or a subcommand takes a separate
/// token. Every token uses only `[A-Za-z0-9_=-]`; Letta's stored `default:<agent>` key is split
/// into its own options first.
pub(crate) fn resume_arguments(session: &RetainedAgentSession) -> Vec<String> {
    use AgentResumeProvider as P;
    let id = session.session_id();
    let separate = |option: &str| vec![option.to_owned(), id.to_owned()];
    match session.provider() {
        P::Codex => separate("resume"),
        P::ClaudeCode
        | P::Grok
        | P::Cursor
        | P::QoderCli
        | P::Droid
        | P::Gemini
        | P::GitHubCopilot
        | P::QwenCode => {
            vec![format!("--resume={id}")]
        }
        P::OpenCode | P::Kilo | P::Kimi => vec![format!("--session={id}")],
        P::Antigravity => separate("--conversation"),
        P::Letta => match id.strip_prefix("default:") {
            Some(agent) => ["--conversation", "default", "--agent", agent]
                .map(str::to_owned)
                .to_vec(),
            None => separate("--conversation"),
        },
        P::Devin | P::Hermes | P::Omp => separate("--resume"),
        P::Pi => separate("--session"),
        P::Cline => separate("--id"),
    }
}

/// How one provider's own command line binds a conversation id.
struct Binding {
    /// Options whose value is the id: a long one as `--name <id>` or `--name=<id>`, a short one as
    /// `-n <id>` only.
    options: &'static [&'static str],
    /// A first positional word after which the next positional is the id.
    subcommand: Option<&'static str>,
    /// Flags that leave the process in another conversation than any bound id: continuing the
    /// latest one, or forking a new one from it.
    voiding: &'static [&'static str],
}

const fn options(options: &'static [&'static str], voiding: &'static [&'static str]) -> Binding {
    Binding {
        options,
        subcommand: None,
        voiding,
    }
}

/// The closed table. It lists the forms each provider documents, the resume forms above, and the
/// launch-time `--session-id` an Agent start gives Grok, Gemini, Qwen Code, Copilot and Pi.
const fn binding(provider: AgentResumeProvider) -> Binding {
    use AgentResumeProvider as P;
    match provider {
        P::ClaudeCode => options(
            &["--resume", "-r", "--session-id"],
            &["-c", "--continue", "--fork-session"],
        ),
        P::Codex => Binding {
            options: &[],
            subcommand: Some("resume"),
            voiding: &["--last"],
        },
        P::Grok => options(
            &["--resume", "-r", "--session-id", "-s"],
            &["-c", "--continue", "--fork-session"],
        ),
        P::OpenCode | P::Kilo => options(&["--session", "-s"], &["-c", "--continue"]),
        P::Cursor | P::Droid => options(&["--resume"], &[]),
        P::QoderCli => options(&["--resume", "-r"], &["-c", "--continue"]),
        P::Antigravity => options(&["--conversation"], &[]),
        P::Kimi => options(&["--session", "-S"], &[]),
        P::Letta => options(&["--conversation", "--conv", "-C"], &[]),
        P::Devin | P::Hermes => options(&["--resume", "-r"], &[]),
        P::Pi => options(
            &["--session", "--session-id"],
            &["-c", "--continue", "-r", "--resume"],
        ),
        P::Omp => options(&["--resume"], &["-c", "--continue"]),
        P::Gemini => options(&["--resume", "-r", "--session-id"], &[]),
        P::GitHubCopilot => options(&["--resume", "-r", "--session-id"], &["--continue"]),
        P::QwenCode => options(&["--resume", "-r", "--session-id"], &["-c", "--continue"]),
        P::Cline => options(&["--id"], &[]),
    }
}

/// Letta's agent option, which completes a bound `default` conversation into its stored
/// `default:<agent>` key.
const LETTA_AGENT: [&str; 2] = ["--agent", "-a"];

/// The session `arguments`, the arguments after the program of an interactive run of `provider`,
/// bind on that command line: exactly one id bound through the table, and valid in the provider's
/// grammar. Other options skip the value their provider's mode grammar gives them, and nothing
/// after `--` is read. Zero or several bound ids, a voiding flag, a short option glued with `=`,
/// or an invalid id give none, never an error.
pub(crate) fn command_line_session(
    provider: AgentResumeProvider,
    arguments: &[String],
) -> Option<RetainedAgentSession> {
    let grammar = AgentReportProvider::from_native_provider_id(provider.provider_id())?
        .registry_entry()
        .grammar;
    let binding = binding(provider);
    let mut bound = Vec::new();
    let mut agents = Vec::new();
    let mut positionals = 0_usize;
    let mut subcommand = false;
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        index += 1;
        if argument == "--" {
            break;
        }
        if argument.len() > 1 && argument.starts_with('-') {
            let (name, inline) = match argument.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (argument.as_str(), None),
            };
            if binding.voiding.contains(&name) {
                return None;
            }
            let agent = provider == AgentResumeProvider::Letta && LETTA_AGENT.contains(&name);
            if binding.options.contains(&name) || agent {
                let value = match inline {
                    Some(value) if name.starts_with("--") => Some(value.to_owned()),
                    Some(_) => return None,
                    None => match arguments.get(index) {
                        Some(next) if !next.starts_with('-') => {
                            index += 1;
                            Some(next.clone())
                        }
                        _ => None,
                    },
                };
                match value {
                    Some(value) if agent => agents.push(value),
                    Some(value) => bound.push(value),
                    None => {}
                }
            } else if inline.is_none() && grammar.interactive_options.contains(&name) {
                index += 1;
            }
            continue;
        }
        positionals += 1;
        match binding.subcommand {
            Some(word) if positionals == 1 => subcommand = argument == word,
            Some(_) if subcommand && positionals == 2 => bound.push(argument.clone()),
            _ => {}
        }
    }
    let [id] = bound.as_slice() else {
        return None;
    };
    let id = match (provider, agents.as_slice()) {
        (AgentResumeProvider::Letta, [agent]) if id == "default" => format!("default:{agent}"),
        _ => id.clone(),
    };
    RetainedAgentSession::new(provider, &id)
}
