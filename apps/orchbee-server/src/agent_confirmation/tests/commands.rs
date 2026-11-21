use super::super::place::Scope;
use super::super::{Wait, command};
use super::{POSIX, WINDOWS};

fn rows(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

#[test]
fn the_check_answers_every_fixture_row() {
    let mut wrong = Vec::new();
    let mut checked = 0;
    for (index, row) in include_str!("commands.tsv").lines().enumerate() {
        if row.is_empty() || row.starts_with('#') {
            continue;
        }
        let mut columns = row.splitn(3, '\t');
        let (Some(answer), Some(shell), Some(command)) =
            (columns.next(), columns.next(), columns.next())
        else {
            panic!("row {} does not have three columns", index + 1);
        };
        let expected = match answer {
            "pass" => Ok(()),
            "destructive" => Err(Wait::Destructive),
            "leaves" => Err(Wait::LeavesWorkspace),
            "protected" => Err(Wait::ProtectedFile),
            "unread" => Err(Wait::NotUnderstood),
            "hidden" => Err(Wait::NotVisible),
            _ => panic!("row {} has an unknown answer", index + 1),
        };
        let scopes: &[Scope<'_>] = match shell {
            "sh" => &[POSIX],
            "pwsh" | "cmd" => &[WINDOWS],
            "any" => &[POSIX, WINDOWS],
            _ => panic!("row {} has an unknown shell", index + 1),
        };
        for scope in scopes {
            checked += 1;
            let found = command::check(&rows(&[command]), *scope);
            if found != expected {
                wrong.push(format!("row {}: {command:?} answered {found:?}", index + 1));
            }
        }
    }
    assert!(checked > 300, "the fixture lost rows: {checked}");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_reading_of_a_line_break_is_checked() {
    // A break may be a wrap at a space, a wrap inside a word, or a new line that starts a command.
    for lines in [
        &["git reset", "--hard"][..],
        &["rm -r", "f build"],
        &["echo ok", "rm -rf build"],
        &["cargo test --package app --", "-rf", "x && r", "m -r build"],
    ] {
        assert_eq!(
            command::check(&rows(lines), POSIX),
            Err(Wait::Destructive),
            "{lines:?}"
        );
    }
    assert_eq!(
        command::check(&rows(&["cargo te", "st --package", "app"]), POSIX),
        Ok(())
    );
    let long = rows(&["echo a", "b", "c", "d", "e", "f", "g"]);
    assert_eq!(command::check(&long, POSIX), Err(Wait::NotUnderstood));
}

#[test]
fn text_that_is_not_plain_waits() {
    for text in [
        "cargo test\u{202e}",
        "cargo\u{200b}test",
        "cargo test \u{2502} x",
        "cargo test\u{1b}[0m",
        "cargo\ttest",
        "",
    ] {
        assert_eq!(
            command::check(&rows(&[text]), POSIX),
            Err(Wait::NotUnderstood),
            "{text:?}"
        );
    }
    assert_eq!(command::check(&[], POSIX), Err(Wait::NotUnderstood));
    let huge = "a".repeat(4096);
    assert_eq!(
        command::check(&rows(&[&huge]), POSIX),
        Err(Wait::NotUnderstood)
    );
}

#[test]
fn a_command_waits_when_the_agent_does_not_work_inside_the_workspace() {
    for scope in [
        Scope {
            root: None,
            directory: Some("/work/app"),
        },
        Scope {
            root: Some("/work/app"),
            directory: None,
        },
        Scope {
            root: Some("/work/app"),
            directory: Some("/work"),
        },
        Scope {
            root: Some("/work/app"),
            directory: Some("/work/app/../other"),
        },
        Scope {
            root: Some("/"),
            directory: Some("/work"),
        },
        Scope {
            root: Some(r"C:\work\app"),
            directory: Some(r"D:\work\app"),
        },
    ] {
        assert_eq!(
            command::check(&rows(&["cargo test"]), scope),
            Err(Wait::LeavesWorkspace),
            "{scope:?}"
        );
    }
}
