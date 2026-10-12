use super::*;
use crate::invocation::{Request, help_paths, parse_request};
use std::ffi::OsString;

#[test]
fn every_help_path_parses_and_renders_in_both_languages() {
    let declared_flag = regex::Regex::new(r"--[a-z][a-z-]*").unwrap();
    for path in help_paths() {
        for words in [
            std::iter::once("help")
                .chain(path.words().iter().copied())
                .collect::<Vec<_>>(),
            path.words()
                .iter()
                .copied()
                .chain(std::iter::once("--help"))
                .collect(),
        ] {
            assert_eq!(
                parse_request(words.iter().map(OsString::from)),
                Ok(Request::Help(path.topic())),
                "{words:?}"
            );
        }
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let text = help(path.topic(), locale);
            assert!(!text.contains('\x1b'));
            assert!(!text.contains('\t'));
            assert!(text.ends_with('\n'));
            assert!(
                text.lines()
                    .all(|line| ratatui::text::Line::from(line).width() <= 78),
                "{:?}: {text}",
                path.words()
            );
            if let HelpTopic::Command(command) = path.topic() {
                let prefix = format!("orchbee {}", command.as_str());
                for syntax in locale.cli_command_help(command).usage.lines() {
                    for flag in declared_flag.find_iter(syntax) {
                        assert!(
                            super::super::completion::help_flags(command).contains(&flag.as_str()),
                            "{command:?}: {} missing from the option list",
                            flag.as_str()
                        );
                    }
                    let suffix = syntax
                        .trim()
                        .strip_prefix(&prefix)
                        .unwrap_or_else(|| panic!("{command:?}: {syntax}"));
                    assert!(
                        suffix.is_empty() || suffix.starts_with(' '),
                        "{command:?}: {syntax}"
                    );
                    assert!(!syntax.contains("[same options]"));
                }
                for flag in super::super::completion::help_flags(command) {
                    assert!(text.contains(flag), "{command:?} omitted {flag}");
                }
            }
        }
    }
}

#[test]
fn directories_list_direct_children_without_dumping_their_syntax() {
    for path in help_paths()
        .iter()
        .filter(|path| matches!(path.topic(), HelpTopic::Family(_)))
    {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let page = directory(path.topic(), locale);
            let labels = page
                .sections
                .iter()
                .filter_map(|section| {
                    if let Section::Rows { rows, .. } = section {
                        Some(rows)
                    } else {
                        None
                    }
                })
                .flatten()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>();
            for child in help_paths().iter().filter(|child| {
                child.words().len() == path.words().len() + 1
                    && child.words().starts_with(path.words())
            }) {
                assert_eq!(
                    labels
                        .iter()
                        .filter(|label| **label == child.words()[path.words().len()])
                        .count(),
                    1,
                    "{:?}",
                    child.words()
                );
            }
            assert!(labels.iter().all(|label| !label.contains(' ')));
        }
    }
}

#[cfg(windows)]
#[test]
fn command_examples_are_admitted_without_executing_them() {
    let mut failures = Vec::new();
    for path in help_paths() {
        let HelpTopic::Command(command) = path.topic() else {
            continue;
        };
        let arguments = examples::arguments(command);
        assert!(arguments.iter().all(|argument| !argument.contains('\'')));
        let words = command
            .as_str()
            .split(' ')
            .chain(arguments.iter().copied())
            .collect::<Vec<_>>();
        if let Err(failure) = parse_request(words.iter().map(OsString::from)) {
            failures.push(format!("{words:?}: {failure:?}"));
        }
        let with_format = words.iter().copied().chain(["--format", "json"]);
        let accepts_format = parse_request(with_format.map(OsString::from)).is_ok();
        if accepts_format != super::super::completion::help_flags(command).contains(&"--format") {
            failures.push(format!(
                "{command:?}: --format admission disagrees with help"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn root_help_uses_the_same_bounded_plain_text_sink() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let result = crate::cli::root_help(locale);
        assert_eq!(result.status, 0);
        assert!(result.stderr.is_empty());
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.starts_with("Orchbee\n"));
        assert!(text.contains("orchbee workspace create"));
        assert!(text.contains("--skill"));
        assert!(!text.contains('\x1b'));
        assert!(
            text.lines()
                .all(|line| ratatui::text::Line::from(line).width() <= 78)
        );
    }
}

#[test]
fn example_continuations_preserve_whole_quoted_operands() {
    let marker = if cfg!(windows) { " `\n" } else { " \\\n" };
    for path in help_paths() {
        let HelpTopic::Command(command) = path.topic() else {
            continue;
        };
        let script = example(command);
        let joined = script.replace(marker, " ");
        assert!(!joined.contains('\n'), "{command:?}: {script}");
        assert!(
            script
                .lines()
                .all(|line| ratatui::text::Line::from(line).width() <= crate::help::WIDTH - 4),
            "{command:?}: {script}"
        );
        for argument in examples::arguments(command) {
            assert!(joined.contains(argument), "{command:?}: lost {argument}");
            if !argument
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "-_=.:/".contains(ch))
            {
                assert!(
                    script.contains(&format!("'{argument}'")),
                    "{command:?}: split quoted operand"
                );
            }
        }
    }
}
