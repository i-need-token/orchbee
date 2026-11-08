//! POSIX keeps `--confirm` and writes no destructive request log until ORC-328, although its
//! cohorts still carry release names.
use super::{Console, Typed};

pub(super) const fn release() -> bool {
    false
}

pub(super) const fn console() -> Console {
    Console {
        stdin: false,
        stdout: false,
    }
}

pub(super) const fn target_session_name() -> Option<String> {
    None
}

pub(super) fn prompt(_text: &str) -> Typed {
    Ok(String::new())
}

pub(super) const fn record(
    _command: &str,
    _target: Option<&str>,
    _console: Console,
    _outcome: &str,
) {
}
