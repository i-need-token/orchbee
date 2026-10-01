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
