use orchbee_capability_node_protocol::*;
use std::num::NonZeroU64;

fn launch() -> AuthorizedTerminalLaunch {
    let n = NonZeroU64::MIN;
    AuthorizedTerminalLaunch::new(
        TerminalOperationAuthorization::new(
            TerminalId::from_bytes([1; 16]).unwrap(),
            OperationId::from_bytes([2; 16]).unwrap(),
            WorkerId::from_bytes([3; 16]).unwrap(),
            WorkerGeneration::new(n),
            TerminalFence::new(n),
            TerminalOperationLease::new(
                LeaseId::from_bytes([4; 16]).unwrap(),
                LeaseExpiresAtUnixMilliseconds::new(n),
            ),
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        ),
        None,
        TerminalLaunchMode::Deferred,
        None,
    )
}

#[test]
fn shell_extension_has_exact_mode_path_environment_tail_and_legacy_bytes_stay_unchanged() {
    let legacy =
        encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(launch())).unwrap();
    for (mode, tag) in [
        (DefaultShellMode::Auto, 0),
        (DefaultShellMode::Login, 1),
        (DefaultShellMode::NonLogin, 2),
    ] {
        for executable in ["", "pwsh.exe", "C:/终端/shell.exe"] {
            for environment in [
                LaunchEnvironment::empty(),
                LaunchEnvironment::new(vec![("MARKER".into(), "literal".into())]).unwrap(),
            ] {
                let message = CapabilityNodeMessage::AuthorizedTerminalLaunch(
                    launch()
                        .with_default_shell(Some(
                            DefaultShellSelection::new(executable.into(), mode).unwrap(),
                        ))
                        .unwrap()
                        .with_environment(environment.clone()),
                );
                let encoded = encode_message(&message).unwrap();
                assert_eq!(&encoded[8..legacy.len()], &legacy[8..]);
                let mut tail = vec![0, 0, 2, tag];
                tail.extend_from_slice(&u16::try_from(executable.len()).unwrap().to_be_bytes());
                tail.extend_from_slice(executable.as_bytes());
                if environment.is_empty() {
                    tail.extend_from_slice(&[0, 0]);
                } else {
                    tail.extend_from_slice(b"\x00\x01\x00\x06MARKER\x00\x07literal");
                }
                assert_eq!(&encoded[legacy.len()..], &tail);
                assert_eq!(decode_message(&encoded).unwrap(), message);
                assert_eq!(
                    encode_message(&decode_message(&encoded).unwrap()).unwrap(),
                    encoded
                );
            }
        }
    }
    assert_eq!(
        legacy,
        encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
            launch().with_default_shell(None).unwrap()
        ))
        .unwrap()
    );
}

#[test]
fn shell_wire_refuses_truncation_unknown_tags_controls_utf8_overflow_and_explicit_selection_conflict()
 {
    let shell = DefaultShellSelection::new("pwsh.exe".into(), DefaultShellMode::Auto).unwrap();
    let start = encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(launch()))
        .unwrap()
        .len();
    let frame = encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
        launch().with_default_shell(Some(shell.clone())).unwrap(),
    ))
    .unwrap();
    let check = |mut frame: Vec<u8>| {
        let n = u16::try_from(frame.len() - 8).unwrap();
        frame[6..8].copy_from_slice(&n.to_be_bytes());
        assert!(decode_message(&frame).is_err());
    };
    for end in start + 1..frame.len() {
        check(frame[..end].to_vec());
    }
    for (index, value) in [
        (start + 2, 3),
        (start + 3, 3),
        (start + 6, 0),
        (start + 6, 0xff),
        (start + 4, 0xff),
    ] {
        let mut bad = frame.clone();
        bad[index] = value;
        check(bad);
    }
    let mut extra = frame;
    extra.push(0);
    check(extra);
    assert!(DefaultShellSelection::new("x".repeat(4096), DefaultShellMode::Auto).is_ok());
    for path in ["x".repeat(4097), "中".repeat(1366), "a\u{0085}b".into()] {
        assert!(DefaultShellSelection::new(path, DefaultShellMode::Auto).is_err());
    }
    let default = launch();
    let explicit = AuthorizedTerminalLaunch::new(
        default.authorization(),
        None,
        TerminalLaunchMode::Deferred,
        Some(LaunchSelection::new("C:/program.exe".into(), vec![]).unwrap()),
    );
    assert_eq!(
        explicit.with_default_shell(Some(shell)),
        Err(ProtocolError::InvalidPayload)
    );
}
