//! Compose command help from admitted grammar and localized semantic copy.

mod examples;
mod pages;
use pages::directory;
pub(super) use pages::root;
#[cfg(test)]
mod tests;

use crate::{
    help::{Document, Row, Section, render},
    invocation::{CommandPath, HelpTopic},
    localization::{HelpText, Locale},
};

pub(crate) fn help(topic: HelpTopic, locale: Locale) -> String {
    let document = match topic {
        HelpTopic::Family(_) => directory(topic, locale),
        HelpTopic::Command(path) => command(path, locale),
    };
    render(&document)
}

fn command(path: CommandPath, locale: Locale) -> Document {
    let details = locale.cli_command_help(path);
    let mut sections = vec![Section::Syntax {
        heading: locale.cli_help_text(HelpText::Usage).into(),
        lines: details.usage.lines().map(str::to_owned).collect(),
    }];
    if path == CommandPath::Status {
        let children = directory(HelpTopic::Command(path), locale);
        sections.extend(
            children
                .sections
                .into_iter()
                .filter(|section| matches!(section, Section::Rows { .. })),
        );
    }
    let arguments = locale.cli_help_arguments(path);
    if !arguments.is_empty() {
        sections.push(Section::Rows {
            heading: locale.cli_help_text(HelpText::Arguments).into(),
            rows: arguments
                .into_iter()
                .map(|(label, description)| Row {
                    label: label.into(),
                    description: description.into(),
                })
                .collect(),
        });
    }
    sections.push(Section::Rows {
        heading: locale.cli_help_text(HelpText::Options).into(),
        rows: super::completion::help_flags(path)
            .into_iter()
            .map(|flag| {
                let (value, description) = locale.cli_help_option(path, flag);
                Row {
                    label: if flag == "--help" {
                        "-h, --help".into()
                    } else if value.is_empty() {
                        flag.to_owned()
                    } else {
                        format!("{flag} {value}")
                    },
                    description: description.into(),
                }
            })
            .collect(),
    });
    sections.push(Section::Examples {
        heading: locale.cli_help_text(HelpText::Examples).into(),
        examples: vec![Row {
            label: example(path),
            description: String::new(),
        }],
    });
    sections.push(Section::Paragraphs {
        heading: locale.cli_help_text(HelpText::Notes).into(),
        paragraphs: details.notes.split("\n\n").map(str::to_owned).collect(),
    });
    Document {
        title: format!("orchbee {}", path.as_str()),
        summary: locale.cli_help_summary(HelpTopic::Command(path)).into(),
        sections,
        footer: vec![locale.cli_help_text(HelpText::Reference).into()],
    }
}

fn example(path: CommandPath) -> String {
    let mut words = vec![format!("orchbee {}", path.as_str())];
    for argument in examples::arguments(path) {
        if argument
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "-_=.:/".contains(ch))
        {
            words.push((*argument).to_owned());
        } else {
            words.push(format!("'{argument}'"));
        }
    }
    let continuation = if cfg!(windows) { '`' } else { '\\' };
    let mut output = String::new();
    let mut line = String::new();
    // Reserve four indentation cells and two cells for the shell continuation.
    // Quoted operands stay atomic, including whitespace and JSON string values.
    for word in words {
        if !line.is_empty()
            && ratatui::text::Line::from(format!("{line} {word}")).width() > crate::help::WIDTH - 6
        {
            output.push_str(&format!("{line} {continuation}\n"));
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    output.push_str(&line);
    output
}
