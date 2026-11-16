use super::*;
use crate::CommandFailure;

fn output(exit_code: i32, stdout: &[u8], stderr: &[u8]) -> CommandOutput {
    CommandOutput {
        exit_code: Some(exit_code),
        failure: (exit_code != 0).then_some(CommandFailure::ExitFailure),
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        stdout_truncated: false,
        stderr_truncated: false,
    }
}

// Only the current appearance can be observed on a real Mac without changing a system setting, so
// the light answer, the absent key, is fixed here from the message `defaults` prints for any
// absent global key.
#[test]
fn only_dark_or_the_absent_key_settle_an_appearance() {
    assert_eq!(classify(&output(0, b"Dark\n", b"")), Some(true));
    let absent = b"\n2026-11-16 defaults[1:2] \nThe domain/default pair of (kCFPreferencesAnyApplication, AppleInterfaceStyle) does not exist\n";
    assert_eq!(classify(&output(1, b"", absent)), Some(false));
    assert_eq!(classify(&output(0, b"Light\n", b"")), None);
    assert_eq!(
        classify(&output(1, b"", b"Could not read preferences")),
        None
    );
    assert_eq!(classify(&output(2, b"", absent)), None);
    let mut timed_out = output(0, b"Dark\n", b"");
    timed_out.failure = Some(CommandFailure::Timeout);
    assert_eq!(classify(&timed_out), None);
    let mut truncated = output(1, b"", absent);
    truncated.stderr_truncated = true;
    assert_eq!(classify(&truncated), None);
}
