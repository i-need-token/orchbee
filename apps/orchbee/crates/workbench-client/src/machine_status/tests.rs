use super::*;
use crate::localization::Locale;
use crate::machine_add::Stage as Check;
use orchbee_connection_handshake::ProductVersion;
use orchbee_machine_protocol::{
    HostKey, KeyFingerprint, Token,
    hub::{ConnectFailure, LoginFailedCause, NeedsLoginCause},
};

fn key() -> HostKey {
    HostKey {
        algorithm: Token::new("ssh-ed25519").unwrap(),
        fingerprint: KeyFingerprint::new("SHA256:abc").unwrap(),
    }
}

fn remote(version: Option<&str>, platform: Option<&str>, architecture: Option<&str>) -> Remote {
    Remote::new(
        version.map(|v| ProductVersion::new(v).unwrap()),
        platform.map(|p| Token::new(p).unwrap()),
        architecture.map(|a| Token::new(a).unwrap()),
    )
    .unwrap()
}

struct Case {
    failure: ConnectFailure,
    state: MachineState,
    remote: Option<Remote>,
    check: Check,
    word: &'static str,
    lines: Vec<Line>,
    action: Option<Action>,
}

/// Each check of an add that fails ends in the state the hub reports, which reads as its word, its
/// detail and its action: the rows of the design's table of add checks.
#[test]
fn each_failed_check_reads_as_its_state_detail_and_action() {
    let target = "user@devbox.invalid";
    let unreachable = |why| Line::Unreachable {
        host: "devbox.invalid".to_owned(),
        why,
    };
    let offline = |cause| MachineState::Offline {
        cause: Some(cause),
        next_attempt: None,
    };
    let login = |state| vec![Line::Login(state)];
    let cases = [
        Case {
            failure: ConnectFailure::SshUnavailable,
            state: offline(OfflineCause::SshUnavailable),
            remote: None,
            check: Check::Ssh,
            word: "offline",
            lines: vec![Line::NoSsh, Line::InstallSsh, Line::LastAttempt(7)],
            action: None,
        },
        Case {
            failure: ConnectFailure::HostUnreachable,
            state: offline(OfflineCause::HostTimeout),
            remote: None,
            check: Check::Reach,
            word: "offline",
            lines: vec![unreachable(Unreachable::TimedOut), Line::LastAttempt(7)],
            action: Some(Action::Retry),
        },
        Case {
            failure: ConnectFailure::HostUnreachable,
            state: offline(OfflineCause::HostRefused),
            remote: None,
            check: Check::Reach,
            word: "offline",
            lines: vec![unreachable(Unreachable::Refused), Line::LastAttempt(7)],
            action: Some(Action::Retry),
        },
        Case {
            failure: ConnectFailure::HostUnreachable,
            state: offline(OfflineCause::HostUnresolved),
            remote: None,
            check: Check::Reach,
            word: "offline",
            lines: vec![unreachable(Unreachable::Unresolved), Line::LastAttempt(7)],
            action: Some(Action::Retry),
        },
        Case {
            failure: ConnectFailure::HostUntrusted,
            state: MachineState::Untrusted { host_key: key() },
            remote: None,
            check: Check::Trust,
            word: "untrusted",
            lines: login(LoginState::Untrusted(key())),
            action: Some(Action::Trust),
        },
        Case {
            failure: ConnectFailure::HostKeyChanged,
            state: MachineState::KeyChanged {
                host_key: Some(key()),
            },
            remote: None,
            check: Check::Trust,
            word: "key-changed",
            lines: login(LoginState::KeyChanged {
                host_key: Some(key()),
                host: "devbox.invalid".to_owned(),
            }),
            action: None,
        },
        Case {
            failure: ConnectFailure::AuthFailed,
            state: MachineState::LoginFailed {
                cause: Some(LoginFailedCause::KeyRejected),
            },
            remote: None,
            check: Check::Login,
            word: "login-failed",
            lines: login(LoginState::LoginFailed(Some(LoginFailedCause::KeyRejected))),
            action: Some(Action::LogInAgain),
        },
        Case {
            failure: ConnectFailure::AuthFailed,
            state: MachineState::LoginFailed {
                cause: Some(LoginFailedCause::PasswordRejected),
            },
            remote: None,
            check: Check::Login,
            word: "login-failed",
            lines: login(LoginState::LoginFailed(Some(
                LoginFailedCause::PasswordRejected,
            ))),
            action: Some(Action::LogInAgain),
        },
        Case {
            failure: ConnectFailure::AuthRequired,
            state: MachineState::NeedsLogin {
                cause: Some(NeedsLoginCause::AuthRequired),
            },
            remote: None,
            check: Check::Login,
            word: "needs-login",
            lines: login(LoginState::NeedsLogin(Some(NeedsLoginCause::AuthRequired))),
            action: Some(Action::LogIn),
        },
        Case {
            failure: ConnectFailure::RemoteUnsupported,
            state: MachineState::Unsupported {
                cause: Some(UnsupportedCause::RemotePlatform),
            },
            remote: Some(remote(None, Some("linux"), Some("armv7l"))),
            check: Check::System,
            word: "unsupported",
            lines: vec![
                Line::Unsupported {
                    platform: Some("linux".into()),
                    architecture: Some("armv7l".into()),
                },
                Line::Supported,
            ],
            action: None,
        },
        Case {
            failure: ConnectFailure::RemoteNotInstalled,
            state: MachineState::NotInstalled,
            remote: None,
            check: Check::Orchbee,
            word: "not-installed",
            lines: vec![Line::NotInstalled],
            action: None,
        },
        Case {
            failure: ConnectFailure::VersionIncompatible,
            state: MachineState::LocalNeedsUpdate,
            remote: None,
            check: Check::Compatibility,
            word: "local-needs-update",
            lines: vec![Line::LocalTooOld, Line::UpdateLocal],
            action: None,
        },
        Case {
            failure: ConnectFailure::VersionIncompatible,
            state: MachineState::NeedsUpdate {
                cause: Some(NeedsUpdateCause::RemoteTooOld),
            },
            remote: Some(remote(Some("1.2.0"), None, None)),
            check: Check::Compatibility,
            word: "needs-update",
            lines: vec![
                Line::RemoteTooOld {
                    version: Some("1.2.0".into()),
                },
                Line::UpdateRemote,
            ],
            action: None,
        },
        Case {
            failure: ConnectFailure::RemoteOtherEnvironment,
            state: MachineState::Incompatible,
            remote: None,
            check: Check::Compatibility,
            word: "incompatible",
            lines: vec![Line::OtherEnvironment],
            action: None,
        },
        Case {
            failure: ConnectFailure::RemoteNotReady,
            state: offline(OfflineCause::RemoteNotReady),
            remote: None,
            check: Check::Session,
            word: "offline",
            lines: vec![Line::NotStarted, Line::LastAttempt(7)],
            action: Some(Action::Retry),
        },
    ];
    for case in cases {
        let failure = case.failure;
        let facts = Facts {
            state: Some(case.state.clone()),
            last_attempt: Some(7),
            remote: case.remote,
        };
        assert_eq!(
            Check::failed(failure, Some(case.state.name())),
            Some(case.check),
            "{failure:?}"
        );
        let observation = Observation::reported(&case.state, target);
        assert_eq!(observation.key(), case.word, "{failure:?}");
        let explanation = Explanation::of(&observation, &facts, target);
        assert_eq!(explanation.lines, case.lines, "{failure:?}");
        assert_eq!(explanation.action, case.action, "{failure:?}");
        for locale in [Locale::EnUs, Locale::ZhCn] {
            assert!(
                locale
                    .machine_status_lines(&explanation, "devbox")
                    .iter()
                    .all(|line| !line.is_empty()),
                "{failure:?} {locale:?}"
            );
        }
    }
}
