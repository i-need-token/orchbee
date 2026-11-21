//! Screens captured from Claude Code 2.1.330 in an 80-column terminal on Windows, with the
//! machine's own paths and status line replaced. Each maps to what Orchbee must read from it.
use orchbee_agent_observation_protocol::AgentReportProvider;

use super::super::layout::{Dialog, Layouts};
use super::super::{Kind, Wait, judge};
use super::WINDOWS;

macro_rules! screen {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!("screens/claude-", $name, ".txt")),
        )
    };
}

fn layouts() -> Layouts {
    Layouts::bundled().expect("bundled layouts compile")
}

fn read(screen: &str) -> Option<Dialog> {
    layouts().read(AgentReportProvider::Claude, screen)
}

fn rows(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

#[test]
fn every_captured_screen_reads_as_what_it_shows() {
    type Expected = Option<(
        &'static str,
        Kind,
        &'static [&'static str],
        Result<(), Wait>,
    )>;
    let table: &[((&str, &str), Expected)] = &[
        (screen!("01-trust"), None),
        (
            screen!("10-bash-mkdir"),
            Some(("bash", Kind::Command, &["mkdir aab-dir"], Ok(()))),
        ),
        // The highlight is on "always allow", which Orchbee never presses.
        (screen!("11-bash-mkdir-highlight2"), None),
        (screen!("13-after-yes"), None),
        (
            screen!("20-write-new-file"),
            Some(("create", Kind::FileCreate, &["aab-note.txt"], Ok(()))),
        ),
        (
            screen!("21-edit-file"),
            Some(("edit", Kind::FileEdit, &["aab-note.txt"], Ok(()))),
        ),
        (screen!("22-after-escape"), None),
        // Claude Code drew no danger marker on either delete; the command check holds them.
        (
            screen!("30-rm-rf"),
            Some((
                "bash",
                Kind::Command,
                &["rm -rf ./aab-dir"],
                Err(Wait::Destructive),
            )),
        ),
        (
            screen!("31-rm-dangerous-var"),
            Some((
                "bash",
                Kind::Command,
                &[r#"AAB_D=./aab-dir && rm -rf "$AAB_D/""#],
                Err(Wait::Destructive),
            )),
        ),
        (
            screen!("40-long-command"),
            Some((
                "bash",
                Kind::Command,
                &[
                    "mkdir -p",
                    "aab-long/one/two/three/four/five/six/seven/eight/nine/ten/eleven/twelve &&",
                    "touch aab-long/one/two/three/four/five/six/seven/eight/nine/ten/eleven/twelv",
                    "e/a-file-with-a-really-long-name-to-force-wrapping.txt",
                ],
                Ok(()),
            )),
        ),
        (
            screen!("50-write-outside"),
            Some((
                "create",
                Kind::FileCreate,
                &[r"..\aab-outside.txt"],
                Err(Wait::LeavesWorkspace),
            )),
        ),
        (
            screen!("51-write-dotenv"),
            Some((
                "create",
                Kind::FileCreate,
                &[".env"],
                Err(Wait::ProtectedFile),
            )),
        ),
        (
            screen!("52-write-subdir"),
            Some((
                "create",
                Kind::FileCreate,
                &[r"sub\dir\aab-deep.txt"],
                Ok(()),
            )),
        ),
    ];
    let layouts = layouts();
    for ((name, screen), expected) in table {
        let dialog = layouts.read(AgentReportProvider::Claude, screen);
        let found = dialog.as_ref().map(|dialog| {
            (
                dialog.layout.as_str(),
                dialog.kind,
                dialog.text.clone(),
                judge(dialog, WINDOWS),
            )
        });
        let expected =
            expected.map(|(layout, kind, text, verdict)| (layout, kind, rows(text), verdict));
        assert_eq!(found, expected, "{name}");
    }
}

#[test]
fn a_dialog_that_differs_from_its_layout_is_not_read() {
    let (_, screen) = screen!("10-bash-mkdir");
    assert!(read(screen).is_some());
    for (change, from, to) in [
        (
            "a row after the footer",
            " Esc to cancel · Tab to amend",
            " Esc to cancel · Tab to amend\n extra",
        ),
        (
            "another footer",
            " Esc to cancel · Tab to amend",
            " Esc to cancel",
        ),
        ("no footer", " Esc to cancel · Tab to amend", ""),
        (
            "another question",
            " Do you want to proceed?",
            " Do you want to proceed anyway?",
        ),
        ("the highlight on No", " ❯ 1. Yes\n", "   1. Yes\n"),
        (
            "the accept row reworded",
            " ❯ 1. Yes\n",
            " ❯ 1. Yes, and don't ask again\n",
        ),
        (
            "another title",
            " Bash command",
            " Bash command (unsandboxed)",
        ),
        (
            "a symbol under the title",
            " Create directory aab-dir",
            " ⚠ Create directory aab-dir",
        ),
        (
            "a pager hint in the command",
            "╌\n mkdir aab-dir\n",
            "╌\n mkdir aab-dir\n… +12 lines\n",
        ),
        (
            "an unindented command row",
            "╌\n mkdir aab-dir\n",
            "╌\n mkdir aab-dir\nrm -rf x\n",
        ),
    ] {
        assert!(screen.contains(from), "{change}");
        assert_eq!(read(&screen.replacen(from, to, 1)), None, "{change}");
    }
    // Another Agent's layouts never read Claude Code's screen.
    assert_eq!(layouts().read(AgentReportProvider::Codex, screen), None);
    assert!(!layouts().supports(AgentReportProvider::Codex));
    assert!(layouts().supports(AgentReportProvider::Claude));
}

#[test]
fn text_inside_the_dialog_cannot_pass_for_the_dialog() {
    let (_, screen) = screen!("10-bash-mkdir");
    // A warning in words under the title is the Agent's own danger marker.
    let warned = screen.replacen(
        " Create directory aab-dir",
        " Warning: this deletes files",
        1,
    );
    let dialog = read(&warned).expect("the layout still matches");
    assert_eq!(judge(&dialog, WINDOWS), Err(Wait::MarkedDangerous));
    // A command row that imitates an option row is command text, and not text the check reads.
    let imitated = screen.replacen(
        "╌\n mkdir aab-dir\n",
        "╌\n │ mkdir aab-dir\n │ ❯ 1. Yes\n",
        1,
    );
    let dialog = read(&imitated).expect("the layout still matches");
    assert_eq!(dialog.text, rows(&["mkdir aab-dir", "❯ 1. Yes"]));
    assert_eq!(judge(&dialog, WINDOWS), Err(Wait::NotUnderstood));
    // A truncated path is not completely on screen.
    let (_, create) = screen!("52-write-subdir");
    let cut = create.replacen(r" sub\dir\aab-deep.txt", r" sub\…\aab-deep.txt", 1);
    let dialog = read(&cut).expect("the layout still matches");
    assert_eq!(judge(&dialog, WINDOWS), Err(Wait::NotVisible));
}
