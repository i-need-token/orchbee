//! Navigation pages project the admitted help paths, so adding a command cannot
//! leave it hidden behind a separately maintained directory of verbs.

use crate::cli_text::{CliText, HelpText};
use crate::help::{Document, Row, Section, render};
use crate::{
    invocation::{Family, HelpPath, HelpTopic, help_paths},
    localization::Locale,
};

const ROOT_GROUPS: &[(HelpText, &[&str])] = &[
    (
        HelpText::Terminals,
        &[
            "tui",
            "workspace",
            "tab",
            "panel",
            "terminal",
            "layout",
            "client",
        ],
    ),
    (
        HelpText::Agents,
        &[
            "agent",
            "project",
            "worktree",
            "launch-profile",
            "plugin",
            "command",
            "events",
            "popup",
            "notification",
        ],
    ),
    (
        HelpText::Sessions,
        &["status", "server", "session", "machine"],
    ),
    (
        HelpText::Setup,
        &["config", "channel", "update", "api", "completion"],
    ),
];

pub(crate) fn root(locale: Locale) -> String {
    let mut sections = vec![
        Section::Syntax {
            heading: CliText::new(locale).cli_help_text(HelpText::Usage).into(),
            lines: vec!["orchbee [<command>] [<options>]".into()],
        },
        Section::Rows {
            heading: CliText::new(locale)
                .cli_help_text(HelpText::QuickStart)
                .into(),
            rows: vec![
                row(
                    "orchbee",
                    CliText::new(locale).cli_help_text(HelpText::OpenTerminal),
                ),
                row(
                    "orchbee server start",
                    CliText::new(locale).cli_help_text(HelpText::SessionStart),
                ),
                row(
                    "orchbee workspace create",
                    CliText::new(locale).cli_help_text(HelpText::NewWorkspace),
                ),
                row(
                    "orchbee agent list",
                    CliText::new(locale).cli_help_text(HelpText::ListAgents),
                ),
            ],
        },
    ];
    for (heading, names) in ROOT_GROUPS {
        let rows = names
            .iter()
            .filter_map(|name| {
                help_paths()
                    .iter()
                    .find(|path| path.words() == [*name])
                    .map(|path| row(name, CliText::new(locale).cli_help_summary(path.topic())))
            })
            .collect();
        sections.push(Section::Rows {
            heading: CliText::new(locale).cli_help_text(*heading).into(),
            rows,
        });
    }
    let mut options = vec![
        row(
            "-h, --help",
            CliText::new(locale).cli_help_text(HelpText::ShowHelp),
        ),
        row(
            "-v, -V, --version",
            CliText::new(locale).cli_help_text(HelpText::ShowVersion),
        ),
        row(
            "--skill",
            CliText::new(locale).cli_help_text(HelpText::ShowSkill),
        ),
    ];
    {
        options.extend([
            row(
                "--session <name>",
                CliText::new(locale).cli_help_text(HelpText::Session),
            ),
            row(
                "--machine <target>",
                CliText::new(locale).cli_help_text(HelpText::Machine),
            ),
            row(
                "--default-config",
                CliText::new(locale).cli_help_text(HelpText::DefaultConfig),
            ),
        ]);
    }
    sections.push(Section::Rows {
        heading: CliText::new(locale)
            .cli_help_text(HelpText::GlobalOptions)
            .into(),
        rows: options,
    });
    render(&Document {
        title: "Orchbee".into(),
        summary: CliText::new(locale)
            .cli_help_text(HelpText::Introduction)
            .into(),
        sections,
        footer: vec![
            CliText::new(locale).cli_help_text(HelpText::Descend).into(),
            CliText::new(locale)
                .cli_help_text(HelpText::FormatScope)
                .into(),
            crate::cli_text::fill_environment_placeholders(
                CliText::new(locale).cli_help_text(HelpText::ConfigLocation),
            ),
        ],
    })
}

/// One line per direct child, taken from that command's own usage line so
/// the quick reference follows the grammar it summarizes. The leading
/// `orchbee` and the shared `--format` choice are omitted.
fn synopsis(path: &str, children: &[&HelpPath], locale: Locale) -> Vec<String> {
    children
        .iter()
        .filter_map(|child| match child.topic() {
            HelpTopic::Command(command) if command.as_str() == child.words().join(" ") => {
                let usage = CliText::new(locale).cli_command_help(command).usage;
                let line = usage.lines().next().unwrap_or(usage);
                let line = line.strip_prefix("orchbee ").unwrap_or(line);
                Some(
                    line.replace(" [--format text|json]", "")
                        .trim_end()
                        .to_owned(),
                )
            }
            HelpTopic::Family(_) => Some(format!(
                "{path} {} <command>",
                child.words()[child.words().len() - 1]
            )),
            HelpTopic::Command(_) => None,
        })
        .collect()
}

pub(crate) fn directory(topic: HelpTopic, locale: Locale) -> Document {
    let path = match topic {
        HelpTopic::Family(family) => family.as_str(),
        HelpTopic::Command(command) => command.as_str(),
    };
    let words = path.split(' ').collect::<Vec<_>>();
    let mut children = help_paths()
        .iter()
        .filter(|child| child.words().len() == words.len() + 1 && child.words().starts_with(&words))
        .collect::<Vec<_>>();
    children.sort_by_key(|child| child.words());
    let mut sections = vec![
        Section::Syntax {
            heading: CliText::new(locale).cli_help_text(HelpText::Usage).into(),
            lines: vec![format!("orchbee {path} <command> [<options>]")],
        },
        Section::Rows {
            heading: CliText::new(locale)
                .cli_help_text(HelpText::Commands)
                .into(),
            rows: children
                .iter()
                .map(|child| {
                    let description = match child.topic() {
                        HelpTopic::Command(command)
                            if command.as_str() != child.words().join(" ") =>
                        {
                            CliText::new(locale).cli_help_alias(command.as_str())
                        }
                        topic => CliText::new(locale).cli_help_summary(topic).into(),
                    };
                    Row {
                        label: child.words()[words.len()].into(),
                        description,
                    }
                })
                .collect(),
        },
    ];
    if topic == HelpTopic::Family(Family::Completion) {
        sections = vec![
            Section::Syntax {
                heading: CliText::new(locale).cli_help_text(HelpText::Usage).into(),
                lines: vec!["orchbee completion <bash|zsh|fish|powershell|elvish>".into()],
            },
            Section::Paragraphs {
                heading: CliText::new(locale).cli_help_text(HelpText::Notes).into(),
                paragraphs: vec![
                    CliText::new(locale)
                        .cli_help_text(HelpText::Completion)
                        .into(),
                ],
            },
            Section::Examples {
                heading: CliText::new(locale)
                    .cli_help_text(HelpText::Examples)
                    .into(),
                examples: vec![row(
                    "orchbee completion powershell",
                    CliText::new(locale).cli_help_text(HelpText::CompletionExample),
                )],
            },
        ];
    }
    sections.insert(
        if topic == HelpTopic::Family(Family::Completion) {
            1
        } else {
            2
        },
        Section::Rows {
            heading: CliText::new(locale).cli_help_text(HelpText::Options).into(),
            rows: vec![row(
                "-h, --help",
                CliText::new(locale).cli_help_text(HelpText::ShowHelp),
            )],
        },
    );
    let synopsis = synopsis(path, &children, locale);
    if topic != HelpTopic::Family(Family::Completion) && !synopsis.is_empty() {
        sections.insert(
            2,
            Section::Syntax {
                heading: CliText::new(locale)
                    .cli_help_text(HelpText::Synopsis)
                    .into(),
                lines: synopsis,
            },
        );
    }
    {
        let notes = match topic {
            HelpTopic::Family(Family::Server | Family::Session) => Some(HelpText::SessionRules),
            HelpTopic::Family(Family::Machine) => Some(HelpText::MachineRules),
            _ => None,
        };
        if let Some(notes) = notes {
            sections.push(Section::Paragraphs {
                heading: CliText::new(locale).cli_help_text(HelpText::Notes).into(),
                paragraphs: vec![CliText::new(locale).cli_help_text(notes).into()],
            });
        }
    }
    let mut footer = Vec::new();
    if topic != HelpTopic::Family(Family::Completion) {
        footer.push(CliText::new(locale).cli_help_next(path));
    }
    footer.push(
        CliText::new(locale)
            .cli_help_text(HelpText::Reference)
            .into(),
    );
    Document {
        title: format!("orchbee {path}"),
        summary: CliText::new(locale).cli_help_summary(topic).into(),
        sections,
        footer,
    }
}

fn row(label: &str, description: &str) -> Row {
    Row {
        label: label.into(),
        description: description.into(),
    }
}

#[cfg(test)]
mod tests;
