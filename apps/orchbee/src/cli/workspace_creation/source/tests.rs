use super::*;
#[test]
fn an_absent_root_cwd_can_fall_back_but_a_missing_root_or_bad_path_cannot() {
    use orchbee_terminal_protocol::{
        AttachmentId, TerminalId, TerminalProcess, TerminalProcessInfo,
    };
    let mut sample = TerminalProcessInfo {
        attachment_id: AttachmentId::parse("00000000000000000000000000000001").unwrap(),
        terminal_id: TerminalId::parse("00000000000000000000000000000002").unwrap(),
        root_pid: 10,
        processes: vec![],
    };
    assert_eq!(
        root_directory(&sample).unwrap_err().kind,
        FailureKind::ServiceUnavailable
    );
    sample.processes.push(TerminalProcess {
        pid: 10,
        parent_pid: None,
        creation_time: 1,
        image: None,
        command_line: None,
        current_directory: None,
    });
    assert!(root_directory(&sample).unwrap().is_none());
    sample.processes[0].current_directory = Some("relative".to_owned());
    assert_eq!(
        root_directory(&sample).unwrap_err().kind,
        FailureKind::ProtocolInvalid
    );
    sample.processes[0].current_directory = Some("C:/live".to_owned());
    assert_eq!(
        root_directory(&sample).unwrap().unwrap().as_str(),
        "C:/live"
    );
}
#[test]
fn automatic_label_is_unicode_bounded_and_roots_are_localized() {
    let path = WorkbenchDirectoryPath::parse(&format!("C:/{}", "猫".repeat(600))).unwrap();
    assert_eq!(
        automatic_label(
            &path,
            &crate::machine_agents::test_support::fixture(),
            Locale::EnUs
        )
        .as_str()
        .chars()
        .count(),
        512
    );
    assert_eq!(
        automatic_label(
            &WorkbenchDirectoryPath::parse("C:/").unwrap(),
            &crate::machine_agents::test_support::fixture(),
            Locale::EnUs
        )
        .as_str(),
        "Workspace (2)"
    );
    assert_eq!(
        automatic_label(
            &WorkbenchDirectoryPath::parse("C:/").unwrap(),
            &crate::machine_agents::test_support::fixture(),
            Locale::ZhCn
        )
        .as_str(),
        "工作区"
    );
}
