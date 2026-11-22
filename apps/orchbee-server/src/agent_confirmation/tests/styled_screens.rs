//! Screens captured with their styles from OpenCode 2.2.5 on Windows, 77 to 143 columns wide,
//! with the machine's own paths and model names replaced. A capture keeps the escape sequences
//! the terminal drew; its text is what a text read of the same frame gives. Each maps to what
//! Orchbee must read from it.
use std::collections::HashMap;

use orchbee_agent_observation_protocol::AgentReportProvider;

use super::super::layout::{Dialog, Layouts, StyledRow};
use super::super::{Kind, Wait, judge};
use super::WINDOWS;

macro_rules! capture {
    ($name:literal) => {
        (
            concat!("opencode-", $name),
            include_str!(concat!("screens/opencode-", $name, ".ansi")),
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

/// The text of a capture, as a text read gives it, and its rows with one style per distinct run
/// of escape sequences.
fn styled(capture: &str) -> (String, Vec<StyledRow>) {
    let mut styles: HashMap<String, u128> = HashMap::new();
    let mut rows = Vec::new();
    for line in capture.lines() {
        let mut row = StyledRow::default();
        let mut style = String::new();
        let mut rest = line;
        while let Some(index) = rest.find("\x1b[") {
            draw(&mut row, &rest[..index], &style, &mut styles);
            let sequence = &rest[index + 2..];
            let end = sequence.find('m').expect("an escape sequence ends");
            match &sequence[..end] {
                "" | "0" => style.clear(),
                parameters => {
                    style.push_str(parameters);
                    style.push(';');
                }
            }
            rest = &sequence[end + 1..];
        }
        draw(&mut row, rest, &style, &mut styles);
        rows.push(row);
    }
    let text = rows
        .iter()
        .map(|row| row.text.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    (text.trim_end().to_owned(), rows)
}

fn draw(row: &mut StyledRow, text: &str, style: &str, styles: &mut HashMap<String, u128>) {
    let next = u128::try_from(styles.len()).expect("few styles");
    let id = *styles.entry(style.to_owned()).or_insert(next);
    for (offset, _) in text.char_indices() {
        row.cells.push((row.text.len() + offset, id));
    }
    row.text.push_str(text);
}

/// The styled rows of `screen`, a changed copy of a captured screen: a row the capture holds
/// keeps its styles, and a new row is drawn in one new style.
fn restyle(captured: &[StyledRow], screen: &str) -> Vec<StyledRow> {
    screen
        .lines()
        .map(|line| {
            captured
                .iter()
                .find(|row| row.text.trim_end() == line)
                .cloned()
                .unwrap_or_else(|| StyledRow {
                    text: line.to_owned(),
                    cells: line
                        .char_indices()
                        .map(|(offset, _)| (offset, u128::MAX))
                        .collect(),
                })
        })
        .collect()
}

fn layouts() -> Layouts {
    Layouts::bundled().expect("bundled layouts compile")
}

fn read(screen: &str, styles: &[StyledRow]) -> Option<Dialog> {
    layouts().read(AgentReportProvider::OpenCode, screen, Some(styles))
}

#[test]
fn every_captured_opencode_screen_reads_as_what_it_shows() {
    let table: &[((&str, &str), Expected)] = &[
        (
            capture!("10-shell-80-columns"),
            Some(("shell-wide", Kind::Command, &["mkdir aab-dir"], Ok(()))),
        ),
        (
            capture!("11-shell"),
            Some(("shell", Kind::Command, &["mkdir aab-o1"], Ok(()))),
        ),
        // A mouse moved over "Reject" with no button pressed: the text is the same, the colours
        // are not.
        (capture!("12-hover-on-reject"), None),
        (capture!("13-hover-away"), None),
        (capture!("14-always-selected"), None),
        (
            capture!("15-back-on-once"),
            Some(("shell", Kind::Command, &["mkdir aab-o1"], Ok(()))),
        ),
        (capture!("16-after-allow"), None),
        (
            capture!("17-two-line-command"),
            Some((
                "shell",
                Kind::Command,
                &["mkdir aab-o2", "mkdir aab-o3"],
                Ok(()),
            )),
        ),
        (
            capture!("18-shell-wide"),
            Some(("shell-wide", Kind::Command, &["mkdir aab-o4"], Ok(()))),
        ),
        (
            capture!("20-write-new-file"),
            Some(("edit", Kind::FileEdit, &["aab-note.txt"], Ok(()))),
        ),
        (
            capture!("21-edit-file"),
            Some(("edit", Kind::FileEdit, &["aab-note.txt"], Ok(()))),
        ),
        (
            capture!("22-write-subfolder-wide"),
            Some((
                "edit-wide",
                Kind::FileEdit,
                &["aab-sub\\aab-deep.txt"],
                Ok(()),
            )),
        ),
        // From about 120 columns a sidebar writes the folder at the end of the last row.
        (
            capture!("23-edit-with-sidebar"),
            Some(("edit-wide", Kind::FileEdit, &["aab-note.txt"], Ok(()))),
        ),
        // A command is not judged.
        (
            capture!("30-rm-rf"),
            Some(("shell", Kind::Command, &["rm -rf ./aab-o1"], Ok(()))),
        ),
        (
            capture!("31-write-env"),
            Some(("edit", Kind::FileEdit, &[".env"], Err(Wait::ProtectedFile))),
        ),
        (
            capture!("32-write-env-with-sidebar"),
            Some((
                "edit-wide",
                Kind::FileEdit,
                &[".env"],
                Err(Wait::ProtectedFile),
            )),
        ),
        // A path outside the Agent's folder is written from the home folder and takes two rows.
        (
            capture!("33-write-outside"),
            Some((
                "edit",
                Kind::FileEdit,
                &[
                    "  ┃    → Edit ~/AppData/Local/Temp/claude/0123456789abcdef/01234567-89ab-",
                    "  ┃      cdef-0123-456789abcdef/scratchpad/aab-outside.txt",
                ],
                Err(Wait::NotVisible),
            )),
        ),
        // The box is full and cuts the command below what it shows; the dialog is still whole.
        (
            capture!("40-long-command-clipped"),
            Some((
                "shell",
                Kind::Command,
                &[
                    "mkdir aab-alpha-bravo-charlie-delta-echo-foxtrot-golf-hotel-india-",
                    "juliet-kilo-lima-mike-november-oscar-papa-quebec-romeo-sierra-tango-",
                    "uniform-victor-whiskey-xray-yankee-zulu-alpha-bravo-charlie-delta-",
                    "echo-foxtrot-golf-hotel-india-juliet-kilo-lima-mike-november-oscar-",
                    "papa-quebec-romeo-sierra-tango-uniform-victor-whiskey-xray-yankee-",
                    "zulu-alpha-bravo-charlie-delta-echo-foxtrot-golf-hotel-india-juliet-",
                ],
                Ok(()),
            )),
        ),
        (
            capture!("41-long-command-clipped-wide"),
            Some((
                "shell-wide",
                Kind::Command,
                &[
                    "mkdir aab-alpha-bravo-charlie-delta-echo-foxtrot-golf-hotel-india-juliet-kilo-lima-mike-november-",
                    "oscar-papa-quebec-romeo-sierra-tango-uniform-victor-whiskey-xray-yankee-zulu-alpha-bravo-charlie-",
                    "delta-echo-foxtrot-golf-hotel-india-juliet-kilo-lima-mike-november-oscar-papa-quebec-romeo-sierra-",
                    "tango-uniform-victor-whiskey-xray-yankee-zulu-alpha-bravo-charlie-delta-echo-foxtrot-golf-hotel-",
                    "india-juliet-kilo-lima-mike-november-oscar-papa-quebec-romeo-sierra-tango-uniform-victor-whiskey-",
                    "xray-yankee-zulu-alpha-bravo-charlie-delta-echo-foxtrot-golf-hotel-india-juliet-kilo-lima-mike-",
                    "november-oscar-papa-quebec-romeo-sierra-tango-uniform-victor-whiskey-xray-yankee-zulu-alpha-bravo-",
                    "charlie-delta-echo-foxtrot-golf-hotel-india-juliet-kilo-lima-mike-november-oscar-papa-quebec-romeo-",
                ],
                Ok(()),
            )),
        ),
        (
            capture!("42-short-panel"),
            Some((
                "shell",
                Kind::Command,
                &[
                    "mkdir aab-alpha-bravo-charlie-delta-echo-foxtrot-golf-hotel-",
                    "india-juliet-kilo-lima-mike-november-oscar-papa-quebec-romeo-",
                    "sierra-tango-uniform-victor-whiskey-xray-yankee-zulu-end",
                ],
                Ok(()),
            )),
        ),
        // The Panel is shorter than the box, whose last rows are cut.
        (capture!("43-short-panel-cut"), None),
    ];
    let layouts = layouts();
    for ((name, capture), expected) in table {
        let (screen, styles) = styled(capture);
        let found = layouts
            .read(AgentReportProvider::OpenCode, &screen, Some(&styles))
            .map(|dialog| {
                (
                    dialog.layout.clone(),
                    dialog.kind,
                    dialog.text.clone(),
                    judge(&dialog, WINDOWS),
                )
            });
        let expected = expected.map(|(layout, kind, text, verdict)| {
            (
                layout.to_owned(),
                kind,
                text.iter().map(|row| (*row).to_owned()).collect::<Vec<_>>(),
                verdict,
            )
        });
        assert_eq!(found, expected, "{name}");
    }
}

#[test]
fn an_opencode_dialog_is_read_only_with_allow_once_drawn_apart() {
    let (_, capture) = capture!("11-shell");
    let (screen, styles) = styled(capture);
    assert!(read(&screen, &styles).is_some());
    // Without styles nothing tells which option Enter chooses.
    assert_eq!(
        layouts().read(AgentReportProvider::OpenCode, &screen, None),
        None
    );
    let options = "  ┃   Allow once   Always allow   Reject";
    let row = styles
        .iter()
        .position(|row| row.text.trim_end() == options)
        .expect("the options row");
    let alike = |styles: &[StyledRow]| {
        let mut styles = styles.to_vec();
        for cell in &mut styles[row].cells {
            cell.1 = 0;
        }
        styles
    };
    // All three drawn alike.
    assert_eq!(read(&screen, &alike(&styles)), None);
    // Styles read from another frame, whose options row says something else.
    let mut other = styles.clone();
    other[row].text = other[row].text.replacen("Reject", "Rejekt", 1);
    assert_eq!(read(&screen, &other), None);
    // Styles that stop above the dialog.
    assert_eq!(read(&screen, &styles[..row]), None);
}

#[test]
fn an_opencode_dialog_that_differs_from_its_layout_is_not_read() {
    let (_, capture) = capture!("11-shell");
    let (screen, styles) = styled(capture);
    let read = |screen: &str| self::read(screen, &restyle(&styles, screen));
    assert!(read(&screen).is_some());
    let command = "  ┃  $ mkdir aab-o1\n";
    let rows = |count: usize| {
        let mut rows = String::from(command);
        for index in 1..count {
            rows.push_str(&format!("  ┃  mkdir aab-o{index}\n"));
        }
        rows
    };
    // Six rows are the most a command shows where the keys have a row of their own.
    assert!(read(&screen.replacen(command, &rows(6), 1)).is_some());
    for (change, from, to) in [
        ("a seventh command row", command.to_owned(), rows(7)),
        (
            "a second title inside the command",
            command.to_owned(),
            "  ┃  $ rm -rf ~\n  ┃  △ Permission required\n  ┃\n".to_owned() + command,
        ),
        (
            "a blank line inside the command",
            command.to_owned(),
            command.to_owned() + "  ┃\n  ┃  rm -rf ~\n",
        ),
        (
            "a next line that starts with the prompt sign",
            command.to_owned(),
            command.to_owned() + "  ┃  $ rm -rf ~\n",
        ),
        (
            "a next line that starts with a space",
            command.to_owned(),
            command.to_owned() + "  ┃   rm -rf ~\n",
        ),
        (
            "another title",
            "△ Permission required".to_owned(),
            "△ Permission needed".to_owned(),
        ),
        (
            "two options",
            "   Always allow   Reject".to_owned(),
            "   Reject".to_owned(),
        ),
        (
            "other keys",
            "enter confirm".to_owned(),
            "enter confirm  esc cancel".to_owned(),
        ),
        (
            "a row after the box",
            "enter confirm\n  ┃".to_owned(),
            "enter confirm\n  ┃\n  ┃  extra".to_owned(),
        ),
    ] {
        let changed = screen.replacen(&from, &to, 1);
        assert_ne!(changed, screen, "{change}");
        assert_eq!(read(&changed), None, "{change}");
    }
    // A path the box writes on one row is judged as written.
    let (_, write) = capture!("20-write-new-file");
    let (write, write_styles) = styled(write);
    let path = |path: &str| {
        let changed = write.replacen("→ Edit aab-note.txt", &format!("→ Edit {path}"), 1);
        let dialog =
            self::read(&changed, &restyle(&write_styles, &changed)).expect("an edit dialog");
        judge(&dialog, WINDOWS)
    };
    assert_eq!(path("~/notes.txt"), Err(Wait::LeavesWorkspace));
    assert_eq!(path("C:/Users/demo/notes.txt"), Err(Wait::LeavesWorkspace));
    assert_eq!(path("aab-sub/AGENTS.md"), Err(Wait::ProtectedFile));
    assert_eq!(path("aab-long-name….txt"), Err(Wait::NotVisible));
    // Another Agent's layouts never read OpenCode's screen.
    for provider in [
        AgentReportProvider::Claude,
        AgentReportProvider::Codex,
        AgentReportProvider::Gemini,
    ] {
        assert_eq!(layouts().read(provider, &screen, Some(&styles)), None);
    }
}
