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
        // OpenCode reads its permissions from either spelling, and Claude Code and Codex load
        // these instructions in later sessions.
        (POSIX, "opencode.jsonc", PROTECTED),
        (POSIX, "sub/CLAUDE.local.md", PROTECTED),
        (POSIX, "AGENTS.override.md", PROTECTED),
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
    let home = Some("/home/demo");
    for scope in [
        Scope {
            root: None,
            directory: Some("/work/app"),
            home,
        },
        Scope {
            root: Some("/work/app"),
            directory: None,
            home,
        },
        Scope {
            root: Some("/work/app"),
            directory: Some("/work"),
            home,
        },
        Scope {
            root: Some("work/app"),
            directory: Some("work/app"),
            home,
        },
        Scope {
            root: Some("/work/app"),
            directory: Some("/work/app"),
            home: None,
        },
    ] {
        assert_eq!(place::check("a.txt", scope), LEAVES, "{scope:?}");
    }
}

// A Workspace at the home folder made every file of the account "inside": login keys, cloud
// credentials, and programs that start at logon.
#[test]
fn nothing_is_inside_a_workspace_that_holds_the_home_folder() {
    let at = |root: &'static str, home: &'static str| Scope {
        root: Some(root),
        directory: Some(root),
        home: Some(home),
    };
    for (scope, path) in [
        (at("/home/demo", "/home/demo"), ".ssh/authorized_keys"),
        (
            at("/home/demo", "/home/demo"),
            "/home/demo/.aws/credentials",
        ),
        (at("/home", "/home/demo"), "demo/.gitconfig"),
        (
            at(r"C:\Users\demo", r"C:\Users\demo"),
            r"AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup\x.cmd",
        ),
        (at(r"c:\users\DEMO", r"C:\Users\demo"), "a.txt"),
        (at(r"C:\Users", r"C:\Users\demo"), r"demo\.npmrc"),
    ] {
        assert_eq!(
            place::check(path, scope),
            Err(Wait::HomeWorkspace),
            "{path:?} in {scope:?}"
        );
    }
    // A Workspace inside the home folder keeps its own files.
    assert_eq!(
        place::check("src/a.txt", at("/home/demo/work", "/home/demo")),
        INSIDE
    );
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
