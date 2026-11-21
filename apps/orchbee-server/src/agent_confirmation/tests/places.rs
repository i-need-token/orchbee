use super::super::place::Scope;
use super::super::{Wait, place};
use super::{POSIX, WINDOWS};

const INSIDE: Result<(), Wait> = Ok(());
const LEAVES: Result<(), Wait> = Err(Wait::LeavesWorkspace);
const PROTECTED: Result<(), Wait> = Err(Wait::ProtectedFile);

#[test]
fn a_path_is_placed_by_its_spelling_in_both_forms() {
    let table: &[(Scope<'_>, &str, Result<(), Wait>)] = &[
        (POSIX, "src/a.txt", INSIDE),
        (POSIX, "./src/a.txt", INSIDE),
        (POSIX, "/work/app/src/a.txt", INSIDE),
        (POSIX, ".gitignore", INSIDE),
        (POSIX, ".github/workflows/ci.yml", INSIDE),
        (POSIX, "/work/application/a.txt", LEAVES),
        (POSIX, "/Work/app/a.txt", LEAVES),
        (POSIX, "/etc/hosts", LEAVES),
        (POSIX, "../a.txt", LEAVES),
        (POSIX, "src/../a.txt", LEAVES),
        (POSIX, "~/a.txt", LEAVES),
        (POSIX, "$HOME/a.txt", LEAVES),
        (POSIX, "//server/share/a.txt", LEAVES),
        (POSIX, ".env", PROTECTED),
        (POSIX, "sub/.env.local", PROTECTED),
        (POSIX, ".git/config", PROTECTED),
        (POSIX, "CLAUDE.md", PROTECTED),
        (POSIX, "docs/claude.md", PROTECTED),
        (POSIX, ".claude/settings.json", PROTECTED),
        (POSIX, ".zshrc", PROTECTED),
        (WINDOWS, r"sub\dir\a.txt", INSIDE),
        (WINDOWS, r"C:\work\app\a.txt", INSIDE),
        (WINDOWS, r"c:\WORK\App\a.txt", INSIDE),
        (WINDOWS, "C:/work/app/a.txt", INSIDE),
        (WINDOWS, "/c/work/app/a.txt", INSIDE),
        (WINDOWS, r"..\a.txt", LEAVES),
        (WINDOWS, r"D:\work\app\a.txt", LEAVES),
        (WINDOWS, r"C:\work\app2\a.txt", LEAVES),
        (WINDOWS, r"\\server\share\a.txt", LEAVES),
        (WINDOWS, r"\work\app\a.txt", LEAVES),
        (WINDOWS, "C:a.txt", LEAVES),
        (WINDOWS, "a.txt:stream", LEAVES),
        (WINDOWS, r"PROGRA~1\a.txt", LEAVES),
        (WINDOWS, r"%USERPROFILE%\a.txt", LEAVES),
        (WINDOWS, ".ENV", PROTECTED),
        (WINDOWS, r".git.\config", PROTECTED),
        (WINDOWS, r".git \config", PROTECTED),
        (WINDOWS, r"sub\AGENTS.md", PROTECTED),
        (WINDOWS, "Microsoft.PowerShell_profile.ps1", PROTECTED),
    ];
    let wrong: Vec<String> = table
        .iter()
        .filter(|(scope, path, expected)| place::check(path, *scope) != *expected)
        .map(|(scope, path, _)| format!("{path:?} answered {:?}", place::check(path, *scope)))
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn nothing_is_inside_without_a_workspace_the_agent_works_in() {
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
            root: Some("work/app"),
            directory: Some("work/app"),
        },
    ] {
        assert_eq!(place::check("a.txt", scope), LEAVES, "{scope:?}");
    }
}

#[test]
fn a_path_that_is_not_plain_text_waits() {
    for path in ["a\u{202e}b.txt", "a\u{2502}b", "a\tb", ""] {
        assert_eq!(
            place::check(path, POSIX),
            Err(Wait::NotUnderstood),
            "{path:?}"
        );
    }
}
