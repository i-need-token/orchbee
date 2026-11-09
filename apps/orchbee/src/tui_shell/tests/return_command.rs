//! The detach hint's command must reopen the TUI being left when run as printed.
use std::ffi::OsString;

use crate::invocation::{Request, parse_selected_request};
use crate::tui_shell::return_command;

/// Parses the printed command as a shell would split it; no Session name needs quoting.
fn reparse(command: &str) -> crate::invocation::SelectedRequest {
    let mut words = command.split_whitespace();
    assert_eq!(words.next(), Some("orchbee"), "{command}");
    parse_selected_request(words.map(OsString::from)).unwrap_or_else(|failure| {
        panic!("{command} does not parse: {failure:?}");
    })
}

// A remote view lives inside the local TUI, so it takes the same command.
#[test]
fn return_command_reselects_the_local_session_it_left() {
    for (selected, expected) in [
        (None, None),
        (Some("default"), None),
        (Some("DEFAULT"), None),
        (Some("t2named"), Some("t2named")),
        (Some("Work.2_b"), Some("Work.2_b")),
        (Some("-x"), Some("-x")),
        (Some("--"), Some("--")),
    ] {
        let command = return_command(selected);
        let request = reparse(&command);
        assert!(
            matches!(request.request, Request::Tui { .. }),
            "{command} opens {:?}",
            request.request
        );
        assert_eq!(
            request.session.as_ref().map(|name| name.as_str()),
            expected,
            "{command}"
        );
    }
}
