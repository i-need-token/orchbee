//! User-provided arguments for a fresh Agent launch. The input groups literal argv words across
//! platforms and has no expansions. Each shell owns quoting on delivery.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A startable Agent's canonical provider id from the closed catalog. The Server resolves it to
/// the command the new shell runs first; the caller never supplies command text.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AgentStartSelector(orchbee_agent_observation_protocol::StartableAgent);
impl AgentStartSelector {
    #[must_use]
    pub fn parse(provider_id: &str) -> Option<Self> {
        orchbee_agent_observation_protocol::StartableAgent::parse(provider_id).map(Self)
    }
    #[must_use]
    pub const fn agent(self) -> orchbee_agent_observation_protocol::StartableAgent {
        self.0
    }

    /// The known providers, in catalog order.
    pub fn all() -> impl Iterator<Item = Self> {
        orchbee_agent_observation_protocol::STARTABLE_AGENT_PROVIDER_IDS
            .iter()
            .filter_map(|id| Self::parse(id))
    }
}
impl TryFrom<String> for AgentStartSelector {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).ok_or("unknown startable Agent provider")
    }
}
impl From<AgentStartSelector> for String {
    fn from(value: AgentStartSelector) -> Self {
        value.0.id().to_owned()
    }
}

pub const MAXIMUM_AGENT_ARGUMENT_INPUT_BYTES: usize = 2048;
const MAXIMUM_CUSTOM_ARGUMENTS: usize = 24;

/// Optional launch preferences per startable Agent. Providers absent from the map use defaults.
pub type UserAgentLaunchSettings = BTreeMap<AgentStartSelector, UserAgentLaunch>;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct UserAgentLaunch {
    pub yolo: bool,
    pub arguments: String,
}

impl AgentStartSelector {
    /// The provider's spelling in a dotted settings key.
    pub fn settings_id(self) -> String {
        self.agent().id().replace('-', "_")
    }

    pub fn arguments_key(self) -> String {
        format!("agents.launch.{}.arguments", self.settings_id())
    }

    pub fn yolo_key(self) -> String {
        format!("agents.launch.{}.yolo", self.settings_id())
    }

    /// This provider's native opt-in for unattended approvals, if Orchbee offers it.
    pub fn yolo_flag(self) -> Option<&'static str> {
        match self.agent().id() {
            "antigravity" | "claude-code" => Some("--dangerously-skip-permissions"),
            "codex" => Some("--dangerously-bypass-approvals-and-sandbox"),
            "cursor" | "github-copilot" => Some("--yolo"),
            "gemini" | "qwen-code" => Some("--approval-mode=yolo"),
            "grok" => Some("--always-approve"),
            _ => None,
        }
    }

    /// An explicit permission or sandbox option supersedes the launch preference.
    pub fn overrides_yolo(self, arguments: &[String]) -> bool {
        let (long, short): (&[&str], &[&str]) = match self.agent().id() {
            "antigravity" => (
                &["--dangerously-skip-permissions", "--sandbox", "--mode"],
                &[],
            ),
            "claude-code" => (
                &["--dangerously-skip-permissions", "--permission-mode"],
                &[],
            ),
            "codex" => (
                &[
                    "--dangerously-bypass-approvals-and-sandbox",
                    "--sandbox",
                    "--ask-for-approval",
                    "--full-auto",
                    "--yolo",
                ],
                &["-s", "-a"],
            ),
            "cursor" => (&["--yolo", "--force", "--sandbox"], &[]),
            "gemini" => (&["--approval-mode", "--yolo"], &["-y"]),
            "github-copilot" => (
                &[
                    "--yolo",
                    "--allow-all",
                    "--allow-all-tools",
                    "--allow-tool",
                    "--deny-tool",
                    "--assisted-approval",
                ],
                &[],
            ),
            "grok" => (
                &[
                    "--always-approve",
                    "--yolo",
                    "--permission-mode",
                    "--dangerously-skip-permissions",
                    "--allow",
                    "--deny",
                    "--sandbox",
                ],
                &[],
            ),
            "qwen-code" => (&["--approval-mode", "--yolo", "--sandbox"], &["-s"]),
            _ => return false,
        };
        arguments
            .iter()
            .take_while(|arg| arg.as_str() != "--")
            .any(|arg| {
                let name = arg.split_once('=').map_or(arg.as_str(), |(name, _)| name);
                long.contains(&name) || short.iter().any(|option| arg.starts_with(option))
            })
    }
}

impl UserAgentLaunch {
    /// The extra arguments a fresh start appends to the provider's fixed arguments.
    pub fn extra_arguments(
        &self,
        provider: AgentStartSelector,
    ) -> Result<Vec<String>, &'static str> {
        let mut arguments = parse_agent_arguments(&self.arguments)?;
        if self.yolo
            && let Some(flag) = provider.yolo_flag()
            && !provider.overrides_yolo(&arguments)
        {
            arguments.insert(0, flag.to_owned());
        }
        Ok(arguments)
    }
}

/// Single or double quotes group words, including an empty word. In a quoted word a backslash
/// escapes its matching quote; other backslashes are literal so Windows paths retain them.
pub fn parse_agent_arguments(input: &str) -> Result<Vec<String>, &'static str> {
    if input.len() > MAXIMUM_AGENT_ARGUMENT_INPUT_BYTES {
        return Err("value-too-long");
    }
    let mut arguments = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    let mut input = input.chars().peekable();
    while let Some(character) = input.next() {
        if character.is_control() && (character != '\t' || quote.is_some()) {
            return Err("invalid-value");
        }
        match quote {
            Some(end) if character == end => quote = None,
            Some(end) if character == '\\' && input.peek() == Some(&end) => {
                word.push(end);
                input.next();
            }
            Some(_) => word.push(character),
            None if matches!(character, ' ' | '\t') => {
                if started {
                    arguments.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            None => {
                started = true;
                if matches!(character, '\'' | '"') {
                    quote = Some(character);
                } else {
                    word.push(character);
                }
            }
        }
    }
    if quote.is_some() {
        return Err("unclosed-quote");
    }
    if started {
        arguments.push(word);
    }
    if arguments.len() > MAXIMUM_CUSTOM_ARGUMENTS {
        return Err("too-many-arguments");
    }
    Ok(arguments)
}
