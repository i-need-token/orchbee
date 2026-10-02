use super::*;

#[test]
fn checkpoint_directory_prefers_terminal_report_then_root_and_never_child_directory() {
    use orchbee_terminal_protocol::{AttachmentId, TerminalId, TerminalProcess};
    use orchbee_workbench_protocol::WorkbenchDirectoryPath;
    let mut sample = TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 10,
        processes: vec![TerminalProcess {
            pid: 20,
            parent_pid: Some(10),
            creation_time: 1,
            image: None,
            command_line: None,
            current_directory: Some("C:/child".into()),
        }],
    };
    assert_eq!(checkpoint_directory(None, &sample), None);
    sample.processes.push(TerminalProcess {
        pid: 10,
        parent_pid: None,
        creation_time: 2,
        image: None,
        command_line: None,
        current_directory: Some("C:/root".into()),
    });
    assert_eq!(
        checkpoint_directory(None, &sample).unwrap().as_str(),
        "C:/root"
    );
    let reported = WorkbenchDirectoryPath::parse("C:/reported").unwrap();
    assert_eq!(
        checkpoint_directory(Some(reported.clone()), &sample),
        Some(reported)
    );
    sample.processes[1].current_directory = Some("relative".into());
    assert_eq!(checkpoint_directory(None, &sample), None);
}

#[test]
fn physical_lines_unicode_trailing_blanks_and_whole_input_bounds() {
    assert_eq!(
        bounded_text(["  提示  ", "answer", "  ", ""].into_iter()).as_deref(),
        Some("  提示\nanswer")
    );
    assert!(bounded_text(["x".repeat(64 * 1024)].iter().map(String::as_str)).is_none());
    assert_eq!(bounded_text(["   ", ""].into_iter()).as_deref(), Some(""));
}

#[test]
fn progress_keeps_unknown_absence_distinct_from_explicit_removal() {
    assert_eq!(progress(None), "");
    assert_eq!(progress(Some(TerminalProgress::Remove)), "4;0;0");
    assert_eq!(progress(Some(TerminalProgress::Set(None))), "4;1;-1");
    assert_eq!(progress(Some(TerminalProgress::Indeterminate)), "4;3;0");
}
