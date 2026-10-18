#[cfg(windows)]
pub(in crate::cli::completion) mod cases;

use super::{
    BUSINESS_FLAGS, COMMAND_FLAGS, DIRECTION_VALUES, FAMILY_VERBS, FORMAT_VALUES, HELP_CHILDREN,
    HELP_LEVEL3, HELP_TOPICS, INTEGRATION_COMMAND_FLAGS, INTEGRATION_VERBS, OPERAND_PATHS,
    ROOT_WORDS, command_flags, families_of, render,
};
use crate::invocation::CompletionShell;

#[test]
fn switches_do_not_consume_the_next_completion_word() {
    let grammar = super::grammar::build();
    let values = grammar["valueFlags"].as_array().unwrap();
    for flag in ["--child", "--yes"] {
        assert!(!values.iter().any(|value| value == flag), "{flag}");
    }
}

#[test]
fn root_mode_candidates_share_the_completion_grammar() {
    let grammar = super::grammar::build();
    let root_mode_flags = super::ROOT_MODE_FLAGS
        .iter()
        .map(|flag| flag.spelling)
        .collect::<Vec<_>>();
    let root_mode_flags_requiring_no_seen_flags = super::ROOT_MODE_FLAGS
        .iter()
        .filter(|flag| flag.requires_no_seen_flags)
        .map(|flag| flag.spelling)
        .collect::<Vec<_>>();
    assert_eq!(grammar["rootModeFlags"], serde_json::json!(root_mode_flags));
    assert_eq!(
        grammar["rootModeFlagsRequireNoSeenFlags"],
        serde_json::json!(root_mode_flags_requiring_no_seen_flags)
    );
    assert_eq!(
        grammar["prefixFlags"],
        serde_json::json!(super::PREFIX_FLAGS)
    );

    for shell in [
        CompletionShell::Bash,
        CompletionShell::Zsh,
        CompletionShell::Fish,
        CompletionShell::PowerShell,
        CompletionShell::Elvish,
    ] {
        let script = render(shell);
        for flag in &root_mode_flags {
            assert!(script.contains(flag), "{shell:?} lacks root mode {flag}");
        }
        assert_eq!(
            script.contains("--default-config"),
            cfg!(windows),
            "{shell:?} has a platform-inappropriate default-config mode"
        );
    }
}

#[test]
fn completion_and_help_admit_the_same_command_paths() {
    let grammar = super::grammar::build();
    let completed = grammar["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| {
            command["words"]
                .as_array()
                .unwrap()
                .iter()
                .map(|word| word.as_str().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<std::collections::BTreeSet<_>>();
    let documented = crate::invocation::help_paths()
        .iter()
        .filter(|path| matches!(path.topic(), crate::invocation::HelpTopic::Command(_)))
        .map(|path| path.words().to_vec())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(completed, documented);
}

#[test]
fn scripts_cover_the_implemented_surface() {
    for shell in [
        CompletionShell::Bash,
        CompletionShell::Zsh,
        CompletionShell::Fish,
    ] {
        let script = render(shell);
        for word in ROOT_WORDS {
            assert!(script.contains(word), "{shell:?} lacks {word}");
        }
        for (family, verbs) in FAMILY_VERBS {
            assert!(
                ROOT_WORDS.contains(family),
                "root completion lacks {family}"
            );
            assert!(
                super::FORMAT_ROOT_WORDS.contains(family),
                "formatted root completion lacks {family}"
            );
            for verb in *verbs {
                assert!(script.contains(verb), "{shell:?} lacks {family} {verb}");
            }
        }
        for flag in BUSINESS_FLAGS {
            assert!(script.contains(flag), "{shell:?} lacks {flag}");
        }
        assert!(script.contains("--format"));
        assert!(script.contains("-v"), "{shell:?} lacks -v");
        assert!(script.contains("-V"), "{shell:?} lacks -V");
        assert_eq!(script.contains("--default-config"), cfg!(windows));
        assert!(script.contains("--mouse"), "{shell:?} lacks --mouse");
        for value in FORMAT_VALUES {
            assert!(script.contains(value), "{shell:?} lacks {value}");
        }
        assert!(script.contains("off"), "{shell:?} lacks --mouse values");
        assert!(script.contains("bash"));
        assert!(script.contains("zsh"));
        assert!(script.contains("fish"));
        assert!(script.contains("--"), "{shell:?} lacks terminator handling");
    }
    // Every help child path begins with an implemented topic; `agent
    // integration` is a sub-family, not a verb, so it is validated
    // against INTEGRATION_VERBS' owner instead.
    for (topic, children) in HELP_CHILDREN {
        assert!(HELP_TOPICS.contains(topic), "{topic}");
        for child in *children {
            let implemented = FAMILY_VERBS
                .iter()
                .any(|(family, verbs)| family == topic && verbs.contains(child))
                || (*topic == "agent" && *child == "integration");
            assert!(implemented, "{topic} {child}");
        }
    }
    // Third-level help paths hang off an implemented topic:child pair.
    for (path, words) in HELP_LEVEL3 {
        let (topic, child) = path.split_once(':').expect("topic:child");
        assert!(
            HELP_CHILDREN
                .iter()
                .any(|(t, c)| *t == topic && c.contains(&child)),
            "{path}"
        );
        assert!(!words.is_empty(), "{path}");
    }
}

#[test]
fn path_tables_agree_with_the_grammar() {
    #[cfg(windows)]
    assert_eq!(
        super::COPY_MOTION_VALUES,
        crate::terminal_copy::CopyMotion::ALL
            .into_iter()
            .map(crate::terminal_copy::CopyMotion::token)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        BUSINESS_FLAGS.len(),
        BUSINESS_FLAGS
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        "duplicate flags multiply every shell projection"
    );
    // Every operand path is an implemented verb.
    for path in OPERAND_PATHS {
        let (family, verb) = path.rsplit_once(':').expect("family:verb");
        assert!(
            FAMILY_VERBS
                .iter()
                .any(|(f, verbs)| *f == family && verbs.contains(&verb))
                || HELP_LEVEL3
                    .iter()
                    .any(|(f, verbs)| *f == family && verbs.contains(&verb)),
            "{path} is not an implemented command"
        );
    }
    assert!(OPERAND_PATHS.contains(&"api:schema"));
    // Every scoped command is an implemented verb (the third-level
    // `agent:integration:*` keys split off their last segment), and every
    // scoped flag is a declared business flag.
    for (command, flags) in COMMAND_FLAGS {
        let (family, verb) = command.rsplit_once(':').expect("family:verb");
        let implemented = FAMILY_VERBS
            .iter()
            .any(|(f, verbs)| *f == family && verbs.contains(&verb))
            || HELP_LEVEL3
                .iter()
                .any(|(f, verbs)| *f == family && verbs.contains(&verb));
        assert!(implemented, "{command} is not an implemented command");
        for flag in *flags {
            assert!(
                *flag == "--mouse" || BUSINESS_FLAGS.contains(flag),
                "{command} lists {flag}"
            );
        }
    }
    // Every integration verb is contained in its union; only list admits maintenance filtering.
    for verb in INTEGRATION_VERBS {
        assert_eq!(
            command_flags(&format!("agent:integration:{verb}")),
            if *verb == "list" {
                INTEGRATION_COMMAND_FLAGS
            } else {
                &["--provider", "--adapter-source"]
            },
            "agent:integration:{verb} scope"
        );
    }
    // Direction values hang off commands that actually admit --direction.
    for (command, values) in DIRECTION_VALUES {
        assert!(
            command_flags(command).contains(&"--direction"),
            "{command} lacks --direction"
        );
        assert!(!values.is_empty(), "{command} lacks direction words");
    }
    // Every flag has at least one owning family.
    for flag in BUSINESS_FLAGS {
        assert!(!families_of(flag).is_empty(), "{flag} has no owner");
    }
}

#[test]
fn launch_flags_are_scoped_and_repeatable() {
    // Explicit program selection belongs only to terminal launches and stored profiles.
    for (command, flags) in COMMAND_FLAGS {
        let admits = *command == "workspace:create"
            || *command == "tab:create"
            || *command == "panel:split"
            || *command == "panel:create-terminal"
            || *command == "launch-profile:create";
        assert_eq!(flags.contains(&"--exec"), admits, "{command} --exec");
        assert_eq!(
            flags.contains(&"--arg"),
            admits || *command == "agent:start",
            "{command} --arg"
        );
    }
    for shell in [
        CompletionShell::Bash,
        CompletionShell::Zsh,
        CompletionShell::Fish,
    ] {
        let script = render(shell);
        // `--exec` is one-shot: its seen arm re-enters `done`.
        // `--arg` is repeatable: its arm consumes a value without
        // touching `done`, and the offer is gated on `--exec`.
        let (exec_arm, arg_arm, arg_offer) = match shell {
            CompletionShell::Fish => (
                "case '--exec'\n                            if test $seen_exec -eq 1; set done 1",
                "case '--arg'\n                            set seen_arg 1; set -a seen_flags '--arg'; set value filter",
                "if test $seen_exec -eq 1 -a $seen_profile -eq 0; set -a candidates --arg; end",
            ),
            _ => (
                "--exec)\n                            if ((seen_exec)); then done=1",
                "--arg)\n                            seen_arg=1; value=filter; seen_flags+=(--arg) ;;",
                "((!seen_exec || seen_profile)) || extras=\"$extras --arg\"",
            ),
        };
        // These assertions own control flow, not generated indentation.
        let unindented = script
            .lines()
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        let normalized = |text: &str| {
            text.lines()
                .map(str::trim_start)
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(
            unindented.contains(&normalized(exec_arm)),
            "{shell:?} --exec arm"
        );
        assert!(
            unindented.contains(&normalized(arg_arm)),
            "{shell:?} --arg arm"
        );
        assert!(script.contains(arg_offer), "{shell:?} --arg offer");
    }
}

/// fish's `test` misreads a leading-dash operand inside a compound
/// `-a`/`-o` expression as an operator (witnessed on fish 4.2.1:
/// `test "$value" = format -a "$word" != text` fails once `$word` is
/// `-c`). `$word` is the only variable that can hold a typed word, so it
/// must never appear inside a compound `test`; `-a`/`-o` compounds over
/// fixed 0/1 `seen_*` operands are valid and remain.
#[test]
fn fish_compound_tests_have_no_user_word_operands() {
    let script = render(CompletionShell::Fish);
    let mut compounds = 0;
    for line in script.lines() {
        for segment in line.split(';') {
            if !segment.contains("test ") || !(segment.contains(" -a ") || segment.contains(" -o "))
            {
                continue;
            }
            compounds += 1;
            assert!(
                !segment.contains("$word"),
                "compound test carries typed-word operand: {line}"
            );
        }
    }
    assert!(compounds > 0, "native -a/-o compounds should remain");
}
