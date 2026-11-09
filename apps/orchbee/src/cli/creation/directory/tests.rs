use super::*;
use orchbee_terminal_protocol::{
    AttachmentId, TerminalId, TerminalProcess, TerminalProcessConsole,
};

#[test]
fn root_directory_never_guesses_a_child_or_relative_path() {
    let row = |pid, value: &str| TerminalProcess {
        pid,
        parent_pid: None,
        creation_time: 1,
        console: TerminalProcessConsole::Attached,
        image: None,
        command_line: None,
        current_directory: Some(value.to_owned()),
    };
    let mut info = TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 20,
        processes: vec![row(30, r"C:\child"), row(20, r"C:\项目 with spaces")],
        launch_kind: None,
    };
    assert_eq!(
        root_directory(&info).unwrap().as_str(),
        r"C:\项目 with spaces"
    );
    for value in ["relative", r"C:relative", "", "bad\0path"] {
        info.processes[1].current_directory = Some(value.to_owned());
        assert!(root_directory(&info).is_none(), "{value:?}");
    }
    info.processes.pop();
    assert!(root_directory(&info).is_none());
}
