//! Screens captured from Claude Code 2.1.330 and Codex CLI 0.171.0 in an 80-column terminal on
//! Windows, with the machine's own paths and status line replaced. Each maps to what Orchbee
//! must read from it.
use orchbee_agent_observation_protocol::AgentReportProvider;

use super::super::layout::{Dialog, Layouts};
use super::super::{Kind, Wait, judge};
use super::WINDOWS;

macro_rules! screen {
    ($name:literal) => {
        screen!("claude", $name)
    };
    ($agent:literal, $name:literal) => {
        (
            concat!($agent, "-", $name),
            include_str!(concat!("screens/", $agent, "-", $name, ".txt")),
        )
    };
}

/// What one screen must read as: the layout, its kind, the text and the verdict on it.
type Expected = Option<(
    &'static str,
    Kind,
    &'static [&'static str],
    Result<(), Wait>,
)>;

fn layouts() -> Layouts {
    Layouts::bundled().expect("bundled layouts compile")
}

fn read(screen: &str) -> Option<Dialog> {
    layouts().read(AgentReportProvider::Claude, screen)
}

fn rows(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

fn assert_reads(provider: AgentReportProvider, table: &[((&str, &str), Expected)]) {
    let layouts = layouts();
    for ((name, screen), expected) in table {
        let dialog = layouts.read(provider, screen);
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
fn every_captured_screen_reads_as_what_it_shows() {
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
    assert_reads(AgentReportProvider::Claude, table);
}

#[test]
fn every_captured_codex_screen_reads_as_what_it_shows() {
    let table: &[((&str, &str), Expected)] = &[
        (
            screen!("codex", "10-command-mkdir"),
            Some(("command", Kind::Command, &["mkdir aab-dir2"], Ok(()))),
        ),
        // The highlight is on "don't ask again", which Orchbee never presses, and on "No".
        (screen!("codex", "11-highlight-always"), None),
        (screen!("codex", "12-highlight-no"), None),
        (screen!("codex", "13-after-yes"), None),
        // Codex writes files through the shell; PowerShell's quoting is more than the check reads.
        (
            screen!("codex", "20-powershell-new-item"),
            Some((
                "command",
                Kind::Command,
                &[
                    r#"New-Item -Path 'aab-note.txt' -ItemType File -Value "hello`n" -ErrorAction"#,
                    "Stop",
                ],
                Err(Wait::NotUnderstood),
            )),
        ),
        // Codex marks nothing as dangerous; the command check holds both.
        (
            screen!("codex", "30-rm-rf"),
            Some((
                "command",
                Kind::Command,
                &["rm -rf ./aab-dir"],
                Err(Wait::Destructive),
            )),
        ),
        (
            screen!("codex", "31-git-push-force"),
            Some((
                "command",
                Kind::Command,
                &["git push --force origin main"],
                Err(Wait::Destructive),
            )),
        ),
        (
            screen!("codex", "40-long-command"),
            Some((
                "command",
                Kind::Command,
                &[
                    "mkdir",
                    "aab-long-one-two-three-four-five-six-seven-eight-nine-ten-eleven-twelve-thir",
                    "teen-fourteen-fifteen-sixteen-seventeen-eighteen-nineteen-twenty-twentyone-t",
                    "wentytwo",
                ],
                Ok(()),
            )),
        ),
        // A dialog taller than the Panel fills it: Codex keeps the title, shortens the options
        // behind an arrow, and in a short Panel puts a marker in place of the command.
        (screen!("codex", "41-tall-options-cut"), None),
        (screen!("codex", "42-short-command-hidden"), None),
        (
            screen!("codex", "50-write-outside"),
            Some((
                "command",
                Kind::Command,
                &[
                    r#"New-Item -Path '..\aab-outside.txt' -ItemType File -Value "x`n""#,
                    "-ErrorAction Stop",
                ],
                Err(Wait::NotUnderstood),
            )),
        ),
        (
            screen!("codex", "51-write-dotenv"),
            Some((
                "command",
                Kind::Command,
                &[r#"New-Item -Path '.env' -ItemType File -Value "A=1`n" -ErrorAction Stop"#],
                Err(Wait::ProtectedFile),
            )),
        ),
    ];
    assert_reads(AgentReportProvider::Codex, table);
}

#[test]
fn a_codex_dialog_that_differs_from_its_layout_is_not_read() {
    let (_, screen) = screen!("codex", "10-command-mkdir");
    let read = |screen: &str| layouts().read(AgentReportProvider::Codex, screen);
    assert!(read(screen).is_some());
    let title = "  Would you like to run the following command?\n";
    let command = "  $ mkdir aab-dir2\n";
    for (change, from, to) in [
        (
            "a row after the footer",
            "  Press enter to confirm or esc to cancel\n",
            "  Press enter to confirm or esc to cancel\n  extra\n",
        ),
        (
            "another footer",
            "  Press enter to confirm or esc to cancel\n",
            "  Press enter to confirm\n",
        ),
        (
            "another environment",
            "  Environment: local\n",
            "  Environment: remote\n",
        ),
        (
            "the accept row reworded",
            "› 1. Yes, proceed (y)\n",
            "› 1. Yes, and don't ask again (y)\n",
        ),
        (
            "an option Codex does not draw today",
            "  3. No, and tell",
            "  3. Yes, for this session (s)\n  4. No, and tell",
        ),
        // Codex indents the model's text like its own title, so that text can draw a title.
        (
            "a second title drawn by the command",
            command,
            "  $ rm -rf ./aab-dir\n  Would you like to run the following command?\n",
        ),
        (
            "a second title drawn above the dialog",
            "• Running mkdir aab-dir2\n",
            "• Running\n  Would you like to run the following command?\n",
        ),
        (
            "a second title drawn by the reason",
            command,
            "  Would you like to run the following command?\n\n  $ mkdir aab-dir2\n",
        ),
        // A dialog taller than the screen: its own title is gone and the command drew this one.
        (
            "no row above the title that only Codex draws",
            "› Run exactly this shell command and nothing else: mkdir aab-dir2\n\n\n• 执行 mkdir aab-dir2。\n\n• Running mkdir aab-dir2\n",
            "  $ rm -rf ./aab-dir\n  &&\n\n\n\n\n",
        ),
        (
            "a second command row drawn by the reason",
            command,
            "  $ mkdir aab-dir3\n\n  $ mkdir aab-dir2\n",
        ),
        (
            "a wrapped row that starts like a command row",
            command,
            "  $ rm -rf ./aab-dir &&\n  $ mkdir aab-dir2\n",
        ),
        (
            "an unindented command row",
            command,
            "  $ mkdir aab-dir2\nrm -rf x\n",
        ),
        ("no reason", "  Reason: ", "  Note: "),
    ] {
        assert!(screen.contains(from), "{change}");
        assert_eq!(read(&screen.replacen(from, to, 1)), None, "{change}");
    }
    assert_eq!(screen.matches(title).count(), 1);
    // A command row that imitates an option row is command text, and not text the check reads.
    let imitated = screen.replacen(command, "  $ mkdir aab-dir2\n  › 1. Yes, proceed (y)\n", 1);
    let dialog = read(&imitated).expect("the layout still matches");
    assert_eq!(
        dialog.text,
        rows(&["mkdir aab-dir2", "› 1. Yes, proceed (y)"])
    );
    assert_eq!(judge(&dialog, WINDOWS), Err(Wait::NotUnderstood));
    // Codex's marker for rows it does not show is command text that is not all on the screen.
    let hidden = screen.replacen(
        command,
        "  $ mkdir aab-dir2\n  [… 9 lines] ctrl+a view all\n",
        1,
    );
    let dialog = read(&hidden).expect("the layout still matches");
    assert_eq!(judge(&dialog, WINDOWS), Err(Wait::NotVisible));
    // Another Agent's layouts never read Codex's screen.
    assert_eq!(layouts().read(AgentReportProvider::Claude, screen), None);
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
    assert!(!layouts().supports(AgentReportProvider::Cursor));
    assert!(layouts().supports(AgentReportProvider::Claude));
    assert!(layouts().supports(AgentReportProvider::Codex));
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
