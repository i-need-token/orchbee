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
        // Only the command list; this page renders its own option table.
        let options = locale.cli_help_text(HelpText::Options);
        let children = directory(HelpTopic::Command(path), locale);
        sections.extend(children.sections.into_iter().filter(
            |section| matches!(section, Section::Rows { heading, .. } if heading != options),
        ));
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
        rows: help_options(path)
            .into_iter()
            .map(|flag| {
                let (value, description) = if let Some(spec) =
                    crate::invocation::grammar::option(flag)
                    && let Some(copy) = spec.help
                {
                    (
                        spec.value.map_or("", |_| "<value>"),
                        locale.cli_grammar_option(copy),
                    )
                } else {
                    locale.cli_help_option(path, flag)
                };
                let (choices, closed) =
                    crate::invocation::grammar::specification(path).values(flag);
                let value = if closed && !choices.is_empty() {
                    format!("<{}>", choices.join("|"))
                } else {
                    usage_value(details.usage, flag).unwrap_or_else(|| value.to_owned())
                };
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
    let mut examples = vec![Row {
        label: example(path),
        description: String::new(),
    }];
    if crate::invocation::grammar::help_flags(path).contains(&"--format") {
        examples.push(Row {
            label: example_with_options(path, &["--format", "json"]),
            description: String::new(),
        });
    }
    sections.push(Section::Examples {
        heading: locale.cli_help_text(HelpText::Examples).into(),
        examples,
    });
    sections.push(Section::Paragraphs {
        heading: locale.cli_help_text(HelpText::Notes).into(),
        paragraphs: std::iter::once(
            locale
                .cli_grammar_availability(
                    crate::invocation::grammar::specification(path).availability,
                )
                .to_owned(),
        )
        .chain(details.notes.split("\n\n").map(str::to_owned))
        .collect(),
    });
    Document {
        title: format!("orchbee {}", path.as_str()),
        summary: locale.cli_help_summary(HelpTopic::Command(path)).into(),
        sections,
        footer: vec![locale.cli_help_text(HelpText::Reference).into()],
    }
}

/// The option table projects the admitted grammar, including orthogonal global options.
pub(super) fn help_options(path: CommandPath) -> Vec<&'static str> {
    crate::invocation::grammar::help_flags(path)
}

/// The value placeholder the usage line spells after `flag`, so the option
/// table and the usage never disagree. Bare closed sets such as
/// `text|json` are shown as `<text|json>`; a switch followed by an
/// alternative separator (`--detach | --timeout <ms>`) takes no value.
pub(super) fn usage_value(usage: &str, flag: &str) -> Option<String> {
    let mut offset = 0;
    while let Some(found) = usage[offset..].find(flag) {
        let start = offset + found;
        let end = start + flag.len();
        offset = end;
        let joined = |text: &str| text.ends_with(|c: char| c.is_ascii_alphanumeric() || c == '-');
        if joined(&usage[..start])
            || usage[end..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
        {
            continue;
        }
        let rest = usage[end..].strip_prefix(' ')?;
        if rest.starts_with('<') {
            return rest.find('>').map(|close| rest[..=close].to_owned());
        }
        let token: String = rest
            .chars()
            .take_while(|c| !matches!(c, ' ' | ']' | ')' | '\n'))
            .collect();
        return (!token.is_empty() && !token.starts_with(['-', '[', '(', '.', '|']))
            .then(|| format!("<{token}>"));
    }
    None
}

fn example(path: CommandPath) -> String {
    example_with_options(path, &[])
}

fn example_with_options(path: CommandPath, options: &[&str]) -> String {
    let mut words = vec![format!("orchbee {}", path.as_str())];
    for argument in examples::arguments(path).iter().chain(options) {
        if argument
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "-_=.:/".contains(ch))
        {
            words.push((*argument).to_owned());
        } else {
            words.push(format!("'{argument}'"));
        }
    }
    let continuation = '`';
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
