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
    pub(crate) seen_bit: &'static str,
    pub(crate) values: &'static [&'static str],
    pub(crate) closed_values: bool,
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
        if spelling == "--format" {
            return (values::FORMAT_VALUES, true);
        }
        if let Some(value) = VALUE_OVERRIDES.iter().find(|value| value.commands.contains(&self.id) && value.flag == spelling) {
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
    ValueOverride { commands: &[CommandPath::AgentStart], flag: "--provider", values: crate::agent_provider::CANONICAL_IDS, closed: true },
    ValueOverride { commands: &[CommandPath::WorkspaceReportMetadata, CommandPath::PanelReportMetadata, CommandPath::PanelReportAgent, CommandPath::PanelReleaseAgent, CommandPath::PanelClearAgentAuthority], flag: "--source", values: &[], closed: false },
];

struct Repetition { flag: &'static str, commands: &'static [CommandPath] }
const REPETITION: &[Repetition] = &[
    Repetition { flag: "--provider", commands: &[CommandPath::AgentDoctor] },
];

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
