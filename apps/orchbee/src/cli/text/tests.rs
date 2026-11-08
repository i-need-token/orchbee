use super::*;
use crate::cli_text::CliText;
use crate::invocation::{Request, help_paths, parse_request};
use std::ffi::OsString;

#[test]
fn every_help_path_parses_and_renders_in_both_languages() {
    let declared_flag = regex::Regex::new(r"--[a-z][a-z-]*").unwrap();
    for path in help_paths() {
        let mut invocations = vec![
            std::iter::once("help")
                .chain(path.words().iter().copied())
                .collect::<Vec<_>>(),
        ];
        for flag in ["-h", "--help"] {
            for index in 0..=path.words().len() {
                let mut words = path.words().to_vec();
                words.insert(index, flag);
                invocations.push(words);
            }
            invocations.push(
                std::iter::once("help")
                    .chain(path.words().iter().copied())
                    .chain([flag])
                    .collect(),
            );
        }
        for words in invocations {
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
            assert!(
                !text.contains("apps/"),
                "installed help must resolve offline: {text}"
            );
            assert!(text.ends_with('\n'));
            assert_environment_paths(&text);
            assert!(text.contains("-h, --help"), "{text}");
            assert!(
                text.lines()
                    .all(|line| ratatui::text::Line::from(line).width() <= 78),
                "{:?}: {text}",
                path.words()
            );
            if let HelpTopic::Command(command) = path.topic() {
                let prefix = format!("orchbee {}", command.as_str());
                for syntax in CliText::new(locale).cli_command_help(command).usage.lines() {
                    for flag in declared_flag.find_iter(syntax) {
                        assert!(
                            crate::invocation::grammar::help_flags(command)
                                .contains(&flag.as_str()),
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
                for flag in super::help_options(command) {
                    assert!(text.contains(flag), "{command:?} omitted {flag}");
                }
            }
        }
    }
}

/// Help fills every environment placeholder, and a development or test build never names the
/// release configuration file, since it reads its own.
fn assert_environment_paths(text: &str) {
    assert!(
        !text.contains(crate::cli_text::CONFIGURATION_FILE_PLACEHOLDER),
        "unfilled placeholder: {text}"
    );
    if !orchbee_local_runtime::is_release() {
        for release in [
            r"%USERPROFILE%\.orchbee\config.toml",
            "~/.orchbee/config.toml",
        ] {
            assert!(!text.contains(release), "{text}");
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
                    if let Section::Rows { heading, rows } = section
                        && heading
                            == CliText::new(locale)
                                .cli_help_text(crate::cli_text::HelpText::Commands)
                    {
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
            assert_eq!(
                labels.len(),
                help_paths()
                    .iter()
                    .filter(|child| {
                        child.words().len() == path.words().len() + 1
                            && child.words().starts_with(path.words())
                    })
                    .count(),
                "directory labels contain exactly their direct children"
            );
        }
    }
}

#[test]
fn prompt_help_keeps_transport_details_in_the_offline_guide() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = help(HelpTopic::Command(CommandPath::AgentPrompt), locale);
        assert!(text.contains("orchbee --skill"));
        assert!(!text.contains("600000"));
        assert!(!text.contains("Right"));
        assert!(!text.contains("prompt-specific"));
        assert!(!text.contains("专属回合"));
        assert!(text.contains("--wait"));
        assert!(text.contains("--format json"));
    }
}

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
        if accepts_format != crate::invocation::grammar::help_flags(command).contains(&"--format") {
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
        assert!(text.contains("-v, -V, --version"));
        assert_environment_paths(&text);
        assert!(!text.contains('\x1b'));
        assert!(
            text.lines()
                .all(|line| ratatui::text::Line::from(line).width() <= 78)
        );
    }
}

#[test]
fn server_help_exposes_existing_stop_and_its_confirmation_boundary() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let page = help(HelpTopic::Family(crate::invocation::Family::Server), locale);
        assert!(page.contains("stop"), "{page}");
        assert!(page.contains("stop-status"), "{page}");
        assert!(page.contains("--confirm"), "{page}");
        let command = help(HelpTopic::Command(CommandPath::ServerStop), locale);
        assert!(
            command.contains("orchbee server stop --confirm"),
            "{command}"
        );
        assert!(
            command
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .contains("server stop-status <id>"),
            "{command}"
        );
    }
}

#[test]
fn example_continuations_preserve_whole_quoted_operands() {
    let marker = " `\n";
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

/// Every command's option table and usage name the same options with the
/// same placeholders, and every positional argument appears in its usage.
#[test]
fn option_tables_and_usage_lines_agree_for_every_command() {
    use crate::invocation::{HelpTopic, help_paths};
    let usage_flags = |usage: &str| {
        usage
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .filter(|word| word.starts_with("--") && word.len() > 2)
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>()
    };
    let mut mismatches = Vec::new();
    for help in help_paths() {
        let HelpTopic::Command(path) = help.topic() else {
            continue;
        };
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let usage = CliText::new(locale).cli_command_help(path).usage;
            let table = super::help_options(path)
                .into_iter()
                .filter(|flag| !matches!(*flag, "--help" | "--session"))
                .map(str::to_owned)
                .collect::<std::collections::BTreeSet<_>>();
            let spelled = usage_flags(usage)
                .into_iter()
                .filter(|flag| flag != "--help" && flag != "--session")
                .collect::<std::collections::BTreeSet<_>>();
            if table != spelled {
                mismatches.push(format!(
                    "{} [{locale:?}] table-only {:?} usage-only {:?}",
                    path.as_str(),
                    table.difference(&spelled).collect::<Vec<_>>(),
                    spelled.difference(&table).collect::<Vec<_>>(),
                ));
            }
            for (label, _) in CliText::new(locale).cli_help_arguments(path) {
                let label = label.trim_start_matches("-- ");
                let core = label
                    .trim_start_matches('[')
                    .split(['>', ']'])
                    .next()
                    .unwrap_or(label);
                if !usage.contains(core) {
                    mismatches.push(format!("{} [{locale:?}] argument {label}", path.as_str()));
                }
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn option_values_follow_the_usage_line() {
    let usage = "orchbee x [--cwd <abs|follow|home>] [--tab-label <text>] [--tab <t>] [--format text|json] [--plain|--ansi] [--detach | --timeout <ms>]";
    assert_eq!(
        super::usage_value(usage, "--cwd").as_deref(),
        Some("<abs|follow|home>")
    );
    assert_eq!(super::usage_value(usage, "--tab").as_deref(), Some("<t>"));
    assert_eq!(
        super::usage_value(usage, "--format").as_deref(),
        Some("<text|json>")
    );
    assert_eq!(super::usage_value(usage, "--plain"), None);
    assert_eq!(super::usage_value(usage, "--detach"), None);
}

/// Family help lists every child's usage line, so a reader sees required
/// options such as `tab rename --label` without descending.
#[test]
fn family_help_carries_a_synopsis_derived_from_child_usage() {
    use crate::invocation::{Family, HelpTopic};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = super::help(HelpTopic::Family(Family::Tab), locale);
        let heading = CliText::new(locale).cli_help_text(HelpText::Synopsis);
        let synopsis = text
            .split(heading)
            .nth(1)
            .expect("family help has a synopsis section");
        let usage = CliText::new(locale)
            .cli_command_help(crate::invocation::CommandPath::TabRename)
            .usage
            .lines()
            .next()
            .unwrap()
            .strip_prefix("orchbee ")
            .unwrap()
            .replace(" [--format text|json]", "");
        let flattened = synopsis.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(flattened.contains(&usage), "{text}");
        assert!(
            text.lines()
                .all(|line| ratatui::text::Line::from(line).width() <= 80),
            "{text}"
        );
    }
}
