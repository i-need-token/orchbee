//! The executable's platform-independent syntax authority. Descriptors are pure data;
//! parsing, help, shell adapters and the schema dictionary project these same records.

mod commands;
mod families;
mod options;
mod rules;
pub(crate) mod schemas;
pub(crate) mod values;

pub(crate) use commands::{COMMANDS, CommandPath};
pub(crate) use families::{FAMILIES, Family};
pub(crate) use options::OPTIONS;

pub(crate) const CARDINAL_DIRECTIONS: &[&str] = &["left", "right", "up", "down"];
pub(crate) const PREFIX_FLAGS: &[&str] = &["--machine"];

pub(crate) struct RootModeFlag {
    pub(crate) spelling: &'static str,
    pub(crate) requires_no_seen_flags: bool,
}

pub(crate) const ROOT_MODES: &[RootModeFlag] = &[
    RootModeFlag {
        spelling: "--help",
        requires_no_seen_flags: false,
    },
    RootModeFlag {
        spelling: "--version",
        requires_no_seen_flags: false,
    },
    RootModeFlag {
        spelling: "-v",
        requires_no_seen_flags: false,
    },
    RootModeFlag {
        spelling: "-V",
        requires_no_seen_flags: false,
    },
    RootModeFlag {
        spelling: "--skill",
        requires_no_seen_flags: false,
    },
    RootModeFlag {
        spelling: "--default-config",
        requires_no_seen_flags: true,
    },
];

use super::{FailureEnvelope, Options, ParseFailure};
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Availability {
    Portable,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetRule {
    Operand,
    Panel,
    TerminalOrPanel,
    Agent,
    Session,
}

pub(crate) struct CommandSpec {
    pub(crate) id: CommandPath,
    pub(crate) words: &'static [&'static str],
    pub(crate) key: &'static str,
    pub(crate) maximum_operands: usize,
    pub(crate) target: TargetRule,
    pub(crate) availability: Availability,
    pub(crate) format: bool,
    pub(crate) machine: bool,
    pub(crate) cwd_values: bool,
}

pub(crate) struct FamilySpec {
    pub(crate) id: Family,
    pub(crate) words: &'static [&'static str],
    pub(crate) key: &'static str,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum OptionRole {
    Business,
    Global,
}

pub(crate) struct OptionSpec {
    pub(crate) spelling: &'static str,
    pub(crate) commands: &'static [CommandPath],
    pub(crate) role: OptionRole,
    /// `None` is a switch; a value name selects a shell's value projection.
    pub(crate) value: Option<&'static str>,
    pub(crate) repeatable: bool,
    pub(crate) joined: bool,
    /// Optional new-option copy is localized by Locale at the presentation boundary.
    pub(crate) help: Option<(&'static str, &'static str)>,
    pub(crate) values: &'static [&'static str],
    pub(crate) closed_values: bool,
    pub(crate) string_rule: Option<&'static StringRule>,
    pub(super) apply: fn(&mut Options, String, FailureEnvelope) -> Result<(), ParseFailure>,
}

pub(crate) fn option(spelling: &str) -> Option<&'static OptionSpec> {
    OPTIONS.iter().find(|option| option.spelling == spelling)
}

pub(crate) fn command(path: &str) -> Option<&'static CommandSpec> {
    COMMANDS
        .iter()
        .find(|command| command.key == path || command.id.as_str() == path)
}

pub(crate) fn specification(id: CommandPath) -> &'static CommandSpec {
    COMMANDS
        .iter()
        .find(|command| command.id == id)
        .expect("command enum and descriptors share one declaration")
}

/// Stable projections are computed once; no copied option-membership table exists.
static FLAGS: LazyLock<Vec<(CommandPath, Vec<&'static str>)>> = LazyLock::new(|| {
    COMMANDS
        .iter()
        .map(|command| {
            (
                command.id,
                OPTIONS
                    .iter()
                    .filter(|option| option.commands.contains(&command.id))
                    .map(|option| option.spelling)
                    .collect(),
            )
        })
        .collect()
});

impl CommandSpec {
    pub(crate) fn flags(&self) -> &'static [&'static str] {
        &FLAGS
            .iter()
            .find(|(id, _)| *id == self.id)
            .expect("one projection per command")
            .1
    }

    pub(crate) fn conflicts(&self, flag: &str) -> &'static [&'static str] {
        rules::conflicts(self, flag)
    }

    pub(crate) fn operands(&self) -> &'static rules::OperandRule {
        rules::operands(self)
    }

    pub(crate) fn requirements(&self) -> impl Iterator<Item = &'static rules::Requirement> {
        rules::requirements(self)
    }

    pub(crate) fn values(&self, spelling: &str) -> (&'static [&'static str], bool) {
        if spelling == "--cwd" {
            return (
                if self.cwd_values {
                    values::CWD_VALUES
                } else {
                    &[]
                },
                false,
            );
        }
        if spelling == "--format" {
            return (values::FORMAT_VALUES, true);
        }
        if let Some(value) = VALUE_OVERRIDES
            .iter()
            .find(|value| value.commands.contains(&self.id) && value.flag == spelling)
        {
            return (value.values, value.closed);
        }
        option(spelling).map_or((&[], false), |option| (option.values, option.closed_values))
    }

    pub(crate) fn repeats(&self, spelling: &str) -> bool {
        if let Some(rule) = REPETITION.iter().find(|rule| rule.flag == spelling) {
            return rule.commands.contains(&self.id);
        }
        option(spelling).is_some_and(|option| option.repeatable)
    }
}

struct ValueOverride {
    commands: &'static [CommandPath],
    flag: &'static str,
    values: &'static [&'static str],
    closed: bool,
}

const VALUE_OVERRIDES: &[ValueOverride] = &[
    ValueOverride {
        commands: &[CommandPath::EventsSubscribe, CommandPath::EventsWait],
        flag: "--until",
        values: values::PANEL_AGENT_STATES,
        closed: true,
    },
    ValueOverride {
        commands: &[CommandPath::AgentStart],
        flag: "--provider",
        values: crate::agent_provider::CANONICAL_IDS,
        closed: true,
    },
    ValueOverride {
        commands: &[
            CommandPath::WorkspaceReportMetadata,
            CommandPath::PanelReportMetadata,
            CommandPath::PanelReportAgent,
            CommandPath::PanelReleaseAgent,
            CommandPath::PanelClearAgentAuthority,
            CommandPath::AgentViewClear,
        ],
        flag: "--source",
        values: &[],
        closed: false,
    },
];

struct Repetition {
    flag: &'static str,
    commands: &'static [CommandPath],
}
const REPETITION: &[Repetition] = &[Repetition {
    flag: "--provider",
    commands: &[CommandPath::AgentDoctor],
}];

static CHILDREN: LazyLock<Vec<(&'static str, Vec<&'static str>)>> = LazyLock::new(|| {
    FAMILIES
        .iter()
        .map(|family| (family.key, family.words))
        .chain(COMMANDS.iter().map(|command| (command.key, command.words)))
        .map(|(key, words)| {
            let mut children = FAMILIES
                .iter()
                .map(|family| family.words)
                .chain(COMMANDS.iter().map(|command| command.words))
                .filter(|candidate| {
                    candidate.len() == words.len() + 1 && candidate.starts_with(words)
                })
                .map(|candidate| candidate[words.len()])
                .collect::<Vec<_>>();
            children.sort_unstable();
            children.dedup();
            (key, children)
        })
        .collect()
});

pub(crate) fn children(key: &str) -> &'static [&'static str] {
    CHILDREN
        .iter()
        .find(|(path, _)| *path == key)
        .map_or(&[], |(_, words)| words.as_slice())
}

/// Derived help options, including orthogonal root options.
pub(crate) fn help_flags(path: CommandPath) -> Vec<&'static str> {
    let command = specification(path);
    let mut flags = command.flags().to_vec();
    flags.extend(
        OPTIONS
            .iter()
            .filter(|option| option.role == OptionRole::Global)
            .map(|option| option.spelling),
    );
    if command.format {
        flags.push("--format");
    }
    flags.push("--help");
    flags
}

pub(crate) fn usage_option_candidates(
    context: crate::invocation::UsageContext,
) -> Vec<&'static str> {
    use crate::invocation::{HelpTopic, UsageContext};

    let mut flags = Vec::new();
    match context {
        UsageContext::Root => {
            for path in crate::invocation::help_paths() {
                if let HelpTopic::Command(command) = path.topic() {
                    flags.extend(help_flags(command));
                }
            }
            flags.extend(ROOT_MODES.iter().map(|flag| flag.spelling));
            flags.extend(PREFIX_FLAGS);
        }
        UsageContext::Path(path) => {
            let words = path.split_whitespace().collect::<Vec<_>>();
            let Some(entry) = crate::invocation::help_paths()
                .iter()
                .find(|entry| entry.words() == words.as_slice())
            else {
                return flags;
            };
            match entry.topic() {
                HelpTopic::Command(command) => flags.extend(help_flags(command)),
                HelpTopic::Family(_) => {
                    for child in crate::invocation::help_paths() {
                        if child.words().starts_with(&words)
                            && let HelpTopic::Command(command) = child.topic()
                        {
                            flags.extend(help_flags(command));
                        }
                    }
                }
            }
        }
        UsageContext::Literal(_) => return flags,
    }
    flags.sort_unstable();
    flags.dedup();
    flags
}

impl CommandSpec {
    /// Grammar constraints run after typed semantic validation to preserve precise domain errors.
    pub(super) fn validate(
        &self,
        words: &[&str],
        options: &Options,
        envelope: FailureEnvelope,
    ) -> Result<(), ParseFailure> {
        use super::{ExpectedValue, UsageContext, UsageReason, usage_with};
        let context = UsageContext::Path(self.id.as_str());
        for flag in &options.seen {
            if !self.repeats(flag) && options.seen.iter().filter(|seen| *seen == flag).count() > 1 {
                return Err(usage_with(
                    UsageReason::DuplicateOption { option: flag },
                    envelope,
                ));
            }
            if let Some(second) = self
                .conflicts(flag)
                .iter()
                .find(|peer| options.seen.contains(peer))
            {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: flag,
                        second,
                    },
                    envelope,
                ));
            }
        }
        for (flag, value) in &options.values {
            let (choices, closed) = self.values(flag);
            let valid_string = self
                .string_rule(flag)
                .is_none_or(|rule| rule.accepts(value));
            if !valid_string || (closed && !choices.contains(&value.as_str())) {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: flag,
                        value: value.clone(),
                        expected: ExpectedValue::DeclaredValue,
                    },
                    envelope,
                ));
            }
        }
        let limit = self
            .operands()
            .reductions
            .iter()
            .filter(|group| group.iter().any(|flag| options.seen.contains(flag)))
            .fold(self.maximum_operands, |limit, _| limit.saturating_sub(1));
        if let Some(extra) = words.get(self.words.len() + limit) {
            return Err(super::unexpected_argument(context, extra, envelope));
        }
        Ok(())
    }
}

/// Character and length constraints, projected without regex anchor or dot semantics.
pub(crate) struct StringRule {
    pub(crate) allowed: Option<&'static str>,
    pub(crate) minimum: usize,
    pub(crate) maximum: Option<usize>,
    pub(crate) excluded: &'static [&'static str],
    pub(crate) trim: bool,
}
impl StringRule {
    fn accepts(&self, value: &str) -> bool {
        let value = if self.trim { value.trim() } else { value };
        let length = value.chars().count();
        length >= self.minimum
            && self.maximum.is_none_or(|maximum| length <= maximum)
            && !self.excluded.contains(&value)
            && self.allowed.is_none_or(|characters| {
                !regex::Regex::new(&format!("[^{characters}]"))
                    .expect("literal ASCII character set")
                    .is_match(value)
            })
    }
}
/// Shell trimming uses the same Unicode White_Space definition as Rust's str::trim.
pub(crate) fn trim_characters() -> &'static str {
    static CHARACTERS: LazyLock<String> = LazyLock::new(|| {
        (0..=char::MAX as u32)
            .filter_map(char::from_u32)
            .filter(|character| character.is_whitespace())
            .collect()
    });
    &CHARACTERS
}
pub(crate) const SESSION_NAME_RULE: StringRule = StringRule {
    allowed: Some("A-Za-z0-9._-"),
    minimum: 1,
    maximum: Some(64),
    excluded: &[".", ".."],
    trim: false,
};
const METADATA_SOURCE_RULE: StringRule = StringRule {
    allowed: Some("A-Za-z0-9:._-"),
    minimum: 1,
    maximum: Some(80),
    excluded: &[],
    trim: true,
};
const AGENT_SOURCE_RULE: StringRule = StringRule {
    allowed: None,
    minimum: 1,
    maximum: None,
    excluded: &[],
    trim: true,
};
impl CommandSpec {
    pub(crate) fn string_rule(&self, flag: &str) -> Option<&'static StringRule> {
        match (self.id, flag) {
            (
                CommandPath::WorkspaceReportMetadata | CommandPath::PanelReportMetadata,
                "--source",
            ) => Some(&METADATA_SOURCE_RULE),
            (CommandPath::PanelReportAgent | CommandPath::PanelReleaseAgent, "--source") => {
                Some(&AGENT_SOURCE_RULE)
            }
            _ => option(flag).and_then(|option| option.string_rule),
        }
    }
    pub(crate) fn operand_string_rule(&self) -> Option<&'static StringRule> {
        (self.target == TargetRule::Session).then_some(&SESSION_NAME_RULE)
    }
}
