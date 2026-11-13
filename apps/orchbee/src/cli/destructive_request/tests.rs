//! The release confirmation rule cannot run in a development or test build, so its decisions are
//! judged here rather than through a staged release binary.
use super::{
    Console, Decision, Host, Output, OutputFormat, Refusal, RemoteTarget, Typed, decide,
    remote_stop_with,
};
use crate::localization::Locale;

const INTERACTIVE: Console = Console {
    stdin: true,
    stdout: true,
};

#[test]
fn development_and_test_builds_run_on_the_grammar_confirmation_without_a_prompt() {
    for stdin in [false, true] {
        for stdout in [false, true] {
            let console = Console { stdin, stdout };
            assert_eq!(
                decide(false, console, true, Some("work"), None),
                Decision::Run
            );
            assert_eq!(decide(false, console, true, None, None), Decision::Run);
        }
    }
}

#[test]
fn release_refuses_the_confirm_option_even_from_a_console_with_the_right_name() {
    assert_eq!(
        decide(true, INTERACTIVE, true, Some("work"), Some("work")),
        Decision::Refuse(Refusal::ConfirmOption)
    );
    assert_eq!(
        decide(
            true,
            Console {
                stdin: false,
                stdout: false
            },
            true,
            Some("work"),
            None
        ),
        Decision::Refuse(Refusal::ConfirmOption)
    );
}

#[test]
fn release_refuses_unless_both_standard_input_and_output_are_consoles() {
    for console in [
        Console {
            stdin: false,
            stdout: false,
        },
        Console {
            stdin: true,
            stdout: false,
        },
        Console {
            stdin: false,
            stdout: true,
        },
    ] {
        assert_eq!(
            decide(true, console, false, Some("work"), Some("work")),
            Decision::Refuse(Refusal::NotInteractive)
        );
    }
}

#[test]
fn release_runs_only_after_the_exact_session_name_is_typed() {
    assert_eq!(
        decide(true, INTERACTIVE, false, Some("work"), None),
        Decision::Prompt
    );
    assert_eq!(
        decide(true, INTERACTIVE, false, None, None),
        Decision::Refuse(Refusal::SessionUnknown)
    );
    for typed in ["work", "work\r\n", "  work \n"] {
        assert_eq!(
            decide(true, INTERACTIVE, false, Some("work"), Some(typed)),
            Decision::Run,
            "{typed:?}"
        );
    }
    for typed in ["", "\r\n", "Work", "wor", "work2", "default"] {
        assert_eq!(
            decide(true, INTERACTIVE, false, Some("work"), Some(typed)),
            Decision::Refuse(Refusal::NameMismatch),
            "{typed:?}"
        );
    }
}

/// A scripted platform: what the build and its consoles are, the line a person types, and what the
/// admission prompted and logged.
struct Script {
    release: bool,
    console: Console,
    typed: &'static str,
    prompts: Vec<String>,
    records: Vec<(String, Option<String>, String)>,
}

impl Script {
    fn new(release: bool, console: Console, typed: &'static str) -> Self {
        Self {
            release,
            console,
            typed,
            prompts: Vec::new(),
            records: Vec::new(),
        }
    }
}

impl Host for Script {
    fn release(&self) -> bool {
        self.release
    }
    fn console(&self) -> Console {
        self.console
    }
    fn prompt(&mut self, text: &str) -> Typed {
        self.prompts.push(text.to_owned());
        Ok(self.typed.to_owned())
    }
    fn record(&mut self, command: &str, target: Option<&str>, _console: Console, outcome: &str) {
        self.records.push((
            command.to_owned(),
            target.map(str::to_owned),
            outcome.to_owned(),
        ));
    }
}

const REMOTE: RemoteTarget<'static> = RemoteTarget {
    selector: "lab",
    id: "11111111111111111111111111111111",
    label: "Lab box",
    session: "far",
};

/// Admits the remote stop of `target` against `script` and returns the refusal, if any.
fn remote(
    script: &mut Script,
    confirm: bool,
    target: &RemoteTarget<'_>,
    locale: Locale,
) -> Option<Output> {
    remote_stop_with(script, confirm, target, OutputFormat::Json, locale)
        .err()
        .map(|output| *output)
}

/// The confirmation `reason` a refusal document names.
fn reason(output: &Output) -> String {
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["failure"]["kind"], "denied");
    assert_eq!(document["failure"]["details"]["stage"], "confirmation");
    document["failure"]["details"]["reason"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn a_remote_stop_follows_the_local_release_rule_on_the_remote_session_and_is_logged_as_remote() {
    let not_interactive = Console {
        stdin: true,
        stdout: false,
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        // Refusals that need no typed line never prompt, and name the remote command to run by hand.
        for (console, confirm, expected) in [
            (INTERACTIVE, true, "confirm-option"),
            (not_interactive, false, "not-interactive"),
        ] {
            let mut script = Script::new(true, console, "far");
            let output = remote(&mut script, confirm, &REMOTE, locale).expect("refused");
            assert_eq!(output.status, 3);
            assert_eq!(reason(&output), expected);
            assert!(script.prompts.is_empty());
            assert!(output.stderr.contains("orchbee --machine lab server stop"));
            assert!(!output.stderr.contains("--session"));
            assert_eq!(
                script.records,
                [(
                    "machine server stop".to_owned(),
                    Some("far".to_owned()),
                    format!("refused-{expected}")
                )]
            );
        }
        // The prompt names the Machine and the remote Session, whose exact name admits the stop.
        for (typed, admitted) in [("far\r\n", true), ("Far", false), ("lab", false)] {
            let mut script = Script::new(true, INTERACTIVE, typed);
            let refusal = remote(&mut script, false, &REMOTE, locale);
            assert_eq!(refusal.is_none(), admitted, "{typed:?}");
            if let Some(output) = &refusal {
                assert_eq!(reason(output), "name-mismatch");
            }
            let [prompt] = script.prompts.as_slice() else {
                panic!("one prompt: {:?}", script.prompts);
            };
            assert!(
                prompt.contains("Lab box") && prompt.contains("far"),
                "{prompt}"
            );
            let outcome = if admitted {
                "accepted"
            } else {
                "refused-name-mismatch"
            };
            assert_eq!(script.records[0].2, outcome);
        }
    }
    // A development or test build runs on the grammar's `--confirm` without asking.
    let mut script = Script::new(false, not_interactive, "");
    assert!(remote(&mut script, true, &REMOTE, Locale::EnUs).is_none());
    assert!(script.prompts.is_empty());
    assert_eq!(script.records[0].2, "accepted");
    // A selector a shell would split is replaced by the Machine's ID in the command to run by hand.
    let spaced = RemoteTarget {
        selector: "Lab box",
        ..REMOTE
    };
    let mut script = Script::new(true, INTERACTIVE, "far");
    let output = remote(&mut script, true, &spaced, Locale::EnUs).expect("refused");
    assert!(
        output
            .stderr
            .contains("orchbee --machine 11111111111111111111111111111111 server stop")
    );
}
