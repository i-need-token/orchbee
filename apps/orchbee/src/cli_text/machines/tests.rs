use super::{MachineQuestion, PROMPT_LINE_CHARACTERS, PROMPT_LINES};
use crate::{cli_text::CliText, localization::Locale};

const TARGET: &str = "user@host.invalid";

/// Whether a terminal could act on `character` or reorder text around it.
fn unsafe_on_a_terminal(character: char) -> bool {
    (character.is_control() && character != '\n')
        || matches!(
            character,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
        )
}

#[test]
fn every_question_shows_ssh_s_own_text_escaped_and_bounded_before_orchbee_s_line() {
    let kinds = [
        MachineQuestion::Password,
        MachineQuestion::KeyPassphrase,
        MachineQuestion::HostKey {
            algorithm: "ed25519",
            fingerprint: "SHA256:abcDEF0123456789",
        },
        MachineQuestion::Other,
    ];
    let hostile = "\u{1b}]0;title\u{7}\u{1b}[2J\u{9b}31m\u{202e}\u{2066}Enter passphrase\r\nfor key 'x'\u{2028}\u{feff}: ";
    let long = "y".repeat(4000);
    let many = "line\n".repeat(40);
    let plain = "user@host.invalid's password: ";
    for (locale, said) in [(Locale::EnUs, "ssh: "), (Locale::ZhCn, "ssh：")] {
        let text = CliText::new(locale);
        for kind in &kinds {
            for prompt in [hostile, long.as_str(), many.as_str(), plain] {
                let shown = text.cli_machine_question(TARGET, kind, prompt, false);
                assert!(
                    !shown.chars().any(unsafe_on_a_terminal),
                    "{locale:?} {prompt:?}: {shown:?}"
                );
                let lines: Vec<&str> = shown.split('\n').collect();
                let (asked, ssh) = lines.split_last().unwrap();
                assert!(
                    !ssh.is_empty() && ssh.len() <= PROMPT_LINES + 1,
                    "{shown:?}"
                );
                for line in ssh {
                    let body = line
                        .strip_prefix(said)
                        .expect("each line of ssh's text is marked");
                    // Each escape takes at most eight characters, and a cut line ends with `…`.
                    assert!(
                        body.chars().count() <= PROMPT_LINE_CHARACTERS * 8 + 1,
                        "{line:?}"
                    );
                }
                assert!(asked.contains(TARGET), "{asked:?}");
            }
            let shown = text.cli_machine_question(TARGET, kind, plain, false);
            assert!(shown.starts_with(&format!("{said}user@host.invalid's password:\n")));
            let shown = text.cli_machine_question(TARGET, kind, hostile, false);
            assert!(shown.contains("\\u{001b}]0;title\\u{0007}"), "{shown:?}");
        }
    }
}
