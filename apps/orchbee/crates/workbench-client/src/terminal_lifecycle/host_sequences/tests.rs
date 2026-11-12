use super::{clipboard, window_title};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Every byte between the OSC introducer and its single final terminator.
fn osc_body<'a>(bytes: &'a [u8], introducer: &[u8]) -> &'a [u8] {
    let start = bytes
        .windows(introducer.len())
        .position(|window| window == introducer)
        .expect("introducer")
        + introducer.len();
    assert!(bytes.ends_with(b"\x1b\\"));
    &bytes[start..bytes.len() - 2]
}

#[test]
fn a_title_cannot_end_its_sequence_or_start_another() {
    let hostile = "repo\x1b\\\x1b]52;c;aGk=\x07\u{9c}\u{9d}tab\r\n";
    let first = window_title(hostile, true);
    assert!(first.starts_with(b"\x1b[22;0t\x1b]0;"));
    let body = osc_body(&first, b"\x1b]0;");
    assert_eq!(body, b"repo\\]52;c;aGk=tab");
    let later = window_title("next", false);
    assert_eq!(later, b"\x1b]0;next\x1b\\");
}

#[test]
fn clipboard_text_reaches_the_terminal_unchanged() {
    let text = "line\r\n\x1b]0;x\x07\ttab 中文 e\u{301}";
    let bytes = clipboard(text);
    let body = osc_body(&bytes, b"\x1b]52;c;");
    assert!(body.iter().all(|byte| byte.is_ascii_graphic()));
    assert_eq!(STANDARD.decode(body).unwrap(), text.as_bytes());
}
