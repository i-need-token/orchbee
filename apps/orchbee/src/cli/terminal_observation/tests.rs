use super::*;
use crate::cli::{
    query::terminal_metadata::{Metadata, Read},
    results::{PanelProcessInfoDoc, TerminalRow},
};
use orchbee_terminal_protocol::{
    TerminalId, TerminalProcess, TerminalProcessInfo, decode_terminal_text_frame,
};

fn observed() -> Read {
    let frame = decode_terminal_text_frame(include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))
    .unwrap();
    let info = TerminalProcessInfo {
        attachment_id: frame.route.attachment_id,
        terminal_id: TerminalId::from_bytes([1; 16]).unwrap(),
        root_pid: 10,
        processes: vec![TerminalProcess {
            pid: 10,
            parent_pid: None,
            creation_time: 1,
            image: Some("C:\\Windows\\cmd.exe".into()),
            command_line: Some("cmd.exe".into()),
            current_directory: Some("C:\\root".into()),
        }],
    };
    let terminal = TerminalRow {
        terminal_id: "1".repeat(32),
        worker_id: "2".repeat(32),
        presentation_id: "3".repeat(32),
        panel_id: "4".repeat(32),
        tab_id: "5".repeat(32),
        terminal_context_id: "6".repeat(32),
    };
    Read {
        frame: Metadata {
            route: frame.route,
            columns: frame.geometry.columns,
            rows: frame.geometry.rows,
            alternate_screen: false,
            scrollback_rows: frame.scrollback_rows,
            title: frame.title,
            current_directory: Some("file:///C:/frame%20%E9%A1%B9%E7%9B%AE".into()),
        },
        processes: Some((json!({}), PanelProcessInfoDoc { terminal, info })),
    }
}

#[test]
fn metadata_directory_precedes_root_without_hiding_separate_process_directory() {
    let mut read = observed();
    let result = Observation::Observed(Box::new(read)).fields();
    assert_eq!(result["cwd"], "C:\\frame 项目");
    assert_eq!(result["cwdSource"], "terminal-frame");
    assert_eq!(result["foregroundCwd"], "C:\\root");
    assert_eq!(result["terminalTitle"], "Orchbee 🐝");
    read = observed();
    read.frame.current_directory = None;
    let result = Observation::Observed(Box::new(read)).fields();
    assert_eq!(result["cwd"], "C:\\root");
    assert_eq!(result["cwdSource"], "root-process");
    read = observed();
    read.processes = None;
    let result = Observation::Observed(Box::new(read)).fields();
    assert_eq!(result["cwd"], "C:\\frame 项目");
    assert_eq!(result["foregroundCwd"], Value::Null);
    assert_eq!(
        result["terminalObservation"]["processes"]["state"],
        "unavailable"
    );
    for unavailable in [Observation::Unbound, Observation::Unavailable] {
        let fields = unavailable.fields();
        assert!(fields["cwd"].is_null());
        assert!(fields["terminalTitle"].is_null());
    }
}

#[test]
fn title_presentation_removes_only_known_delimited_prefixes() {
    for (input, expected) in [
        ("Administrator: ⠋ 中文 task", Some("中文 task")),
        ("✳ task", Some("task")),
        ("⠋ ⠙ task", Some("⠙ task")),
        ("⠋task", Some("⠋task")),
        ("★ task", Some("★ task")),
        ("◐", None),
        ("   ", None),
        ("  task  ", Some("task")),
    ] {
        assert_eq!(stripped(input), expected);
    }
}

#[test]
fn osc_directory_decodes_once_without_admitting_uri_syntax_as_a_windows_path() {
    for (raw, expected) in [
        ("file:///C:/one%20two", "C:\\one two"),
        ("file://localhost/C:/a+b", "C:\\a+b"),
        ("file://server/share/dir", "\\\\server\\share\\dir"),
        ("file:///C:/a%2520b", "C:\\a%20b"),
        ("C:\\项目", "C:\\项目"),
    ] {
        assert_eq!(reported_directory(raw).as_deref(), Some(expected));
    }
    for raw in [
        "",
        "relative",
        "C:relative",
        "file:///tmp/work",
        "file:///C:/bad%",
        "file:///C:/%ff",
        "file:///C:/%00",
        "file:///C:/a?x",
        "file:///C:/a#b",
        "file://user@host/share",
        "file://server/share/%0a",
        "\\\\.\\pipe\\not-a-directory",
    ] {
        assert!(reported_directory(raw).is_none(), "{raw}");
    }
}

#[test]
fn runtime_text_escapes_untrusted_metadata_in_both_locales() {
    let mut read = observed();
    read.frame.title = Some("\x1b]0;evil\x07\n中文".into());
    read.frame.current_directory = Some("C:\\dir\r\nnext".into());
    let fields = Observation::Observed(Box::new(read)).fields();
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let bytes = crate::cli::output::render_text(|out| render(out, &fields, locale)).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains('\x1b'));
        assert!(!text.contains('\x07'));
        assert!(!text.contains('\r'));
    }
}
