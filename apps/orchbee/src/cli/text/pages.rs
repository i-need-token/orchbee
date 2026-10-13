//! Navigation pages project the admitted help paths, so adding a command cannot
//! leave it hidden behind a separately maintained directory of verbs.

use crate::help::{Document, Row, Section, render};
use crate::{
    invocation::{Family, HelpTopic, help_paths},
    localization::{HelpText, Locale},
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
            heading: locale.cli_help_text(HelpText::Usage).into(),
            lines: vec!["orchbee [<command>] [<options>]".into()],
        },
        Section::Rows {
            heading: locale.cli_help_text(HelpText::QuickStart).into(),
            rows: vec![
                row("orchbee", locale.cli_help_text(HelpText::OpenTerminal)),
                row(
                    "orchbee server start",
                    locale.cli_help_text(HelpText::SessionStart),
                ),
                row(
                    "orchbee workspace create",
                    locale.cli_help_text(HelpText::NewWorkspace),
                ),
                row(
                    "orchbee agent list",
                    locale.cli_help_text(HelpText::ListAgents),
                ),
            ],
        },
        session_concept(locale),
    ];
    for (heading, names) in ROOT_GROUPS {
        let rows = names
            .iter()
            .filter_map(|name| {
                help_paths()
                    .iter()
                    .find(|path| path.words() == [*name])
                    .map(|path| row(name, locale.cli_help_summary(path.topic())))
            })
            .collect();
        sections.push(Section::Rows {
            heading: locale.cli_help_text(*heading).into(),
            rows,
        });
    }
    let mut options = vec![
        row("-h, --help", locale.cli_help_text(HelpText::ShowHelp)),
        row("--version", locale.cli_help_text(HelpText::ShowVersion)),
        row("--skill", locale.cli_help_text(HelpText::ShowSkill)),
    ];
    if cfg!(windows) {
        options.extend([
            row("--session <name>", locale.cli_help_text(HelpText::Session)),
            row(
                "--machine <target>",
                locale.cli_help_text(HelpText::Machine),
            ),
            row(
                "--default-config",
                locale.cli_help_text(HelpText::DefaultConfig),
            ),
        ]);
    }
    sections.push(Section::Rows {
        heading: locale.cli_help_text(HelpText::GlobalOptions).into(),
        rows: options,
    });
    render(&Document {
        title: "Orchbee".into(),
        summary: locale.cli_help_text(HelpText::Introduction).into(),
        sections,
        footer: vec![
            locale.cli_help_text(HelpText::Descend).into(),
            locale.cli_help_text(HelpText::FormatScope).into(),
        ],
    })
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
            heading: locale.cli_help_text(HelpText::Usage).into(),
            lines: vec![format!("orchbee {path} <command> [<options>]")],
        },
        Section::Rows {
            heading: locale.cli_help_text(HelpText::Commands).into(),
            rows: children
                .iter()
                .map(|child| {
                    let description = match child.topic() {
                        HelpTopic::Command(command)
                            if command.as_str() != child.words().join(" ") =>
                        {
                            locale.cli_help_alias(command.as_str())
                        }
                        topic => locale.cli_help_summary(topic).into(),
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
                heading: locale.cli_help_text(HelpText::Usage).into(),
                lines: vec!["orchbee completion <bash|zsh|fish|powershell|elvish>".into()],
            },
            Section::Paragraphs {
                heading: locale.cli_help_text(HelpText::Notes).into(),
                paragraphs: vec![locale.cli_help_text(HelpText::Completion).into()],
            },
            Section::Examples {
                heading: locale.cli_help_text(HelpText::Examples).into(),
                examples: vec![row(
                    "orchbee completion powershell",
                    locale.cli_help_text(HelpText::CompletionExample),
                )],
            },
        ];
    }
    let explains_session = topic == HelpTopic::Family(Family::Server);
    #[cfg(windows)]
    let explains_session = explains_session || topic == HelpTopic::Family(Family::Session);
    if explains_session {
        sections.insert(1, session_concept(locale));
        sections.push(Section::Examples {
            heading: locale.cli_help_text(HelpText::Examples).into(),
            examples: session_examples(locale),
        });
    }
    let mut footer = Vec::new();
    if cfg!(windows) && explains_session {
        footer.push(locale.cli_help_text(HelpText::SessionRules).into());
    }
    #[cfg(windows)]
    if topic == HelpTopic::Family(Family::Machine) {
        footer.push(locale.cli_help_text(HelpText::MachineRules).into());
    }
    if topic != HelpTopic::Family(Family::Completion) {
        footer.push(locale.cli_help_next(path));
    }
    footer.push(locale.cli_help_text(HelpText::Reference).into());
    Document {
        title: format!("orchbee {path}"),
        summary: locale.cli_help_summary(topic).into(),
        sections,
        footer,
    }
}

fn session_concept(locale: Locale) -> Section {
    Section::Paragraphs {
        heading: locale.cli_help_text(HelpText::SessionConcept).into(),
        paragraphs: vec![
            locale.cli_help_text(HelpText::SessionDefinition).into(),
            locale.cli_help_text(HelpText::SessionSelection).into(),
        ],
    }
}

fn session_examples(locale: Locale) -> Vec<Row> {
    let mut examples = vec![row("orchbee", locale.cli_help_text(HelpText::OpenTerminal))];
    #[cfg(windows)]
    examples.extend([
        row(
            "orchbee server start\norchbee workspace create",
            locale.cli_help_text(HelpText::SessionStartExample),
        ),
        row(
            "orchbee session attach work",
            locale.cli_help_summary(HelpTopic::Command(
                crate::invocation::CommandPath::SessionAttach,
            )),
        ),
    ]);
    #[cfg(not(windows))]
    examples.push(row(
        "orchbee server start",
        locale.cli_help_text(HelpText::SessionStart),
    ));
    examples
}

fn row(label: &str, description: &str) -> Row {
    Row {
        label: label.into(),
        description: description.into(),
    }
}

#[cfg(test)]
mod tests;
