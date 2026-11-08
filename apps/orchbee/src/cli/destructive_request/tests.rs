//! The release confirmation rule cannot run in a development or test build, so its decisions are
//! judged here rather than through a staged release binary.
use super::{Console, Decision, Refusal, decide};

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
