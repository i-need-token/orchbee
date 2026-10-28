use super::*;
use orchbee_workbench_protocol::NotificationSound;
#[test]
fn notification_backend_selection_and_exact_osc_framing() {
    assert_eq!(backend(Some("WezTerm"), None, true), Some(Backend::Osc9));
    assert_eq!(
        backend(None, Some("xterm-kitty"), false),
        Some(Backend::Kitty)
    );
    assert_eq!(backend(None, Some("xterm-256color"), false), None);
    let message =
        Notification::normalize("Ready", Some("构建"), None, NotificationSound::None).unwrap();
    assert_eq!(
        encode(Backend::Osc9, &message, false),
        "\x1b]9;Ready: 构建\x1b\\".as_bytes()
    );
    assert_eq!(
        encode(Backend::Kitty, &message, false),
        "\x1b]99;i=1:d=0;Ready\x1b\\\x1b]99;i=1:p=body;构建\x1b\\".as_bytes()
    );
    let bytes = sequence(
        &message,
        &Host {
            program: Some("WezTerm".into()),
            term: None,
            kitty: true,
            tmux: true,
        },
    )
    .unwrap();
    assert_eq!(
        bytes,
        "\x1bPtmux;\x1b\x1b]9;Ready: 构建\x1b\x1b\\\x1b\\".as_bytes()
    );
}
