use super::*;
use crate::{CapabilityNodeMessage as Message, decode_message, encode_message};
use std::num::NonZeroU64;

fn request() -> TerminalEditorCommandRequest {
    TerminalEditorCommandRequest {
        request_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MAX),
    }
}

fn editor_launch() -> crate::AuthorizedTerminalLaunch {
    use crate::*;
    AuthorizedTerminalLaunch::new(
        TerminalOperationAuthorization::new(
            TerminalId::from_bytes([1; 16]).unwrap(),
            OperationId::from_bytes([2; 16]).unwrap(),
            WorkerId::from_bytes([3; 16]).unwrap(),
            WorkerGeneration::new(NonZeroU64::MIN),
            TerminalFence::new(NonZeroU64::MIN),
            TerminalOperationLease::new(
                LeaseId::from_bytes([4; 16]).unwrap(),
                LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
            ),
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        ),
        None,
        TerminalLaunchMode::Deferred,
        Some(
            LaunchSelection::new(r"C:\editor.exe".into(), vec!["".into(), "--wait".into()])
                .unwrap(),
        ),
    )
}
fn editor_source() -> TerminalEditorSource {
    TerminalEditorSource {
        attachment_id: OperationId::from_bytes([9; 16]).unwrap(),
        host_epoch: crate::HostEpoch::from_bytes([10; 16]).unwrap(),
        terminal_id: crate::HostTerminalId::from_bytes([11; 16]).unwrap(),
        generation: NonZeroU64::MAX,
        revision: 0,
    }
}

#[test]
fn editor_source_kind_preserves_all_fences() {
    let base = editor_launch();
    for revision in [0, u64::MAX] {
        let source = TerminalEditorSource {
            revision,
            ..editor_source()
        };
        let launch = base.clone().with_editor_source(Some(source)).unwrap();
        let message = Message::AuthorizedTerminalLaunch(launch.clone());
        let frame = encode_message(&message).unwrap();
        assert_eq!(decode_message(&frame).unwrap(), message);
        assert_eq!(&frame[frame.len() - 8..], &revision.to_be_bytes());
    }
}

#[test]
fn editor_source_kind_refuses_invalid_tail_identity_geometry_and_ambiguous_launch() {
    use crate::*;
    let source = editor_source();
    let base = editor_launch();
    let valid = base.clone().with_editor_source(Some(source)).unwrap();
    let frame = encode_message(&Message::AuthorizedTerminalLaunch(valid.clone())).unwrap();
    let start = frame.len() - 64;
    for range in [
        start..start + 16,
        start + 16..start + 32,
        start + 32..start + 48,
        start + 48..start + 56,
    ] {
        let mut bad = frame.clone();
        bad[range].fill(0);
        assert!(decode_message(&bad).is_err());
    }
    for end in start..frame.len() {
        let mut bad = frame[..end].to_vec();
        bad[6..8].copy_from_slice(&((end - 8) as u16).to_be_bytes());
        assert!(decode_message(&bad).is_err());
    }
    let mut bad = frame.clone();
    bad.push(0);
    let len = (bad.len() - 8) as u16;
    bad[6..8].copy_from_slice(&len.to_be_bytes());
    assert!(decode_message(&bad).is_err());
    assert_eq!(frame[101], 2);
    let mut immediate = frame.clone();
    immediate[101] = 1;
    immediate.splice(102..102, [0, 80, 0, 24, 0, 8, 0, 16]);
    let len = (immediate.len() - 8) as u16;
    immediate[6..8].copy_from_slice(&len.to_be_bytes());
    assert!(decode_message(&immediate).is_err());
    let mut unknown = frame.clone();
    unknown[102] = 5;
    assert!(decode_message(&unknown).is_err());
    let no_program = AuthorizedTerminalLaunch::new(
        base.authorization(),
        None,
        TerminalLaunchMode::Deferred,
        None,
    );
    assert!(no_program.with_editor_source(Some(source)).is_err());
    let immediate = AuthorizedTerminalLaunch::new(
        base.authorization(),
        None,
        TerminalLaunchMode::Immediate(TerminalPixelGeometry::new(80, 24, 8, 16).unwrap()),
        base.launch().cloned(),
    );
    assert!(immediate.with_editor_source(Some(source)).is_err());
    let plugin = PluginTerminalEnvironment::new(
        PLUGIN_TERMINAL_ENVIRONMENT_NAMES[..7]
            .iter()
            .map(|name| ((*name).into(), "v".into()))
            .collect(),
    )
    .unwrap();
    assert!(
        base.clone()
            .with_plugin_environment(Some(plugin.clone()))
            .unwrap()
            .with_editor_source(Some(source))
            .is_err()
    );
    assert!(valid.clone().with_plugin_environment(Some(plugin)).is_err());
    assert!(
        valid
            .with_default_shell(Some(
                DefaultShellSelection::new("cmd.exe".into(), DefaultShellMode::Auto).unwrap()
            ))
            .is_err()
    );
}

#[test]
fn editor_command_round_trips_all_outcomes_and_rejects_every_strict_prefix_and_tail() {
    let mut messages = vec![Message::TerminalEditorCommandRequest(request())];
    for source in [
        TerminalEditorCommandSource::Visual,
        TerminalEditorCommandSource::Editor,
        TerminalEditorCommandSource::Default,
    ] {
        messages.push(Message::TerminalEditorCommandResult(
            TerminalEditorCommandResult {
                request: request(),
                outcome: TerminalEditorCommandOutcome::Resolved {
                    source,
                    selection: LaunchSelection::new(
                        r"C:\编辑 器\editor.exe".into(),
                        vec!["".into(), "literal & % ! $ 🙂".into()],
                    )
                    .unwrap(),
                },
            },
        ));
    }
    for reason in [
        TerminalEditorCommandRefusal::WrongWorker,
        TerminalEditorCommandRefusal::StaleWorkerGeneration,
        TerminalEditorCommandRefusal::UnsupportedPlatform,
        TerminalEditorCommandRefusal::InvalidCommand,
        TerminalEditorCommandRefusal::ProgramUnavailable,
    ] {
        messages.push(Message::TerminalEditorCommandResult(
            TerminalEditorCommandResult {
                request: request(),
                outcome: TerminalEditorCommandOutcome::Refused(reason),
            },
        ));
    }
    for message in messages {
        let frame = encode_message(&message).unwrap();
        assert_eq!(decode_message(&frame).unwrap(), message);
        assert_eq!(
            encode_message(&decode_message(&frame).unwrap()).unwrap(),
            frame
        );
        for end in 0..frame.len() {
            assert!(decode_message(&frame[..end]).is_err());
        }
        let mut tail = frame.clone();
        tail.push(0);
        assert!(decode_message(&tail).is_err());
        let size = u16::from_be_bytes([tail[6], tail[7]]) + 1;
        tail[6..8].copy_from_slice(&size.to_be_bytes());
        assert!(decode_message(&tail).is_err());
    }
}

#[test]
fn editor_command_refuses_zero_identity_generation_unknown_tags_and_invalid_selections() {
    let frame = encode_message(&Message::TerminalEditorCommandRequest(request())).unwrap();
    assert_eq!(frame[8], 24);
    for range in [9..25, 25..41, 41..49] {
        let mut bad = frame.clone();
        bad[range].fill(0);
        assert!(decode_message(&bad).is_err());
    }
    let frame = encode_message(&Message::TerminalEditorCommandResult(
        TerminalEditorCommandResult {
            request: request(),
            outcome: TerminalEditorCommandOutcome::Resolved {
                source: TerminalEditorCommandSource::Visual,
                selection: LaunchSelection::new(r"C:\editor.exe".into(), vec!["arg".into()])
                    .unwrap(),
            },
        },
    ))
    .unwrap();
    assert_eq!(frame[8], 25);
    for (position, value) in [
        (49, 0),
        (49, 3),
        (50, 0),
        (50, 4),
        (53, 0),
        (54, b'x'),
        (55, b'x'),
    ] {
        let mut bad = frame.clone();
        bad[position] = value;
        assert!(decode_message(&bad).is_err(), "{position}/{value}");
    }
    let mut bad = frame.clone();
    bad[51..53].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(decode_message(&bad).is_err());
    let refusal = encode_message(&Message::TerminalEditorCommandResult(
        TerminalEditorCommandResult {
            request: request(),
            outcome: TerminalEditorCommandOutcome::Refused(
                TerminalEditorCommandRefusal::InvalidCommand,
            ),
        },
    ))
    .unwrap();
    for tag in [0, 6, 255] {
        let mut bad = refusal.clone();
        bad[50] = tag;
        assert!(decode_message(&bad).is_err());
    }
}

#[test]
fn editor_command_preserves_maximum_literal_selection_without_a_document_argument() {
    let selection = LaunchSelection::new(
        r"C:\editor.exe".into(),
        vec!["🙂 ".repeat(32); crate::MAXIMUM_LAUNCH_SELECTION_ARGUMENTS],
    )
    .unwrap();
    let message = Message::TerminalEditorCommandResult(TerminalEditorCommandResult {
        request: request(),
        outcome: TerminalEditorCommandOutcome::Resolved {
            source: TerminalEditorCommandSource::Editor,
            selection,
        },
    });
    let frame = encode_message(&message).unwrap();
    assert_eq!(decode_message(&frame).unwrap(), message);
    assert!(LaunchSelection::new(r"C:\editor.exe".into(), vec![String::new(); 129]).is_err());
    assert!(LaunchSelection::new(r"C:\editor.exe".into(), vec!["\n".repeat(6000)]).is_err());
}
